//! The terminal WebSocket: one browser pane attached to one tmux session.

mod handlers;
mod pty;
pub mod share;
mod share_ticket;
mod session;
mod settings;

use actix_web::{get, web, HttpRequest, HttpResponse};
use std::os::unix::io::FromRawFd;
use actix_web_actors::ws;

pub use session::{new_pty_store, restore_sessions, start_pty_cleanup, PtyStore};
pub use settings::settings_for;

use crate::auth::JwtSecret;
use crate::ssh_auth;
use pty::{attach_tmux, parse_size, NoticeSocket, WsBridge};
use session::{
    create_tmux_session, generate_session_id, now_unix, persist, tmux_session_exists, TmuxSession,
    MAX_SCROLLBACK,
};

pub(crate) const MAX_TIMEOUT_SECS: u64 = 30 * 24 * 3600; // 30 days
const PW_FILE_TTL_SECS: u64 = 30; // long enough for ssh to read it inside tmux
/// How long a pane whose ssh failed stays open so the reason can be read.
const FAILED_PANE_LINGER_SECS: u64 = 900;

// --- WebSocket endpoint ---

/// Resolve the SSH target of a new session: either a saved server (`server_id`,
/// which may carry a stored password) or explicit host/port/user parameters.
/// Returns the shell command tmux should run, plus a label for the UI.
fn build_ssh_command(query: &str, keepalive: bool, ka_interval: u64) -> Result<Option<(String, String)>, String> {
    let (host, port, user, password) = if let Some(server_id) = parse_query_param(query, "server_id") {
        let srv = crate::remote_servers::find_server(&server_id)
            .ok_or_else(|| "Saved server not found".to_string())?;
        // `no_saved_password=1` connects as if nothing were stored, so the
        // operator can type the password (or a one-time code) in the pane.
        let skip = parse_query_param(query, "no_saved_password").map(|v| v == "1").unwrap_or(false);
        let pw = if skip { None } else { crate::remote_servers::server_password(&srv) };
        (srv.host, srv.port.to_string(), srv.user, pw)
    } else if let Some(host) = parse_query_param(query, "ssh_host") {
        let port = parse_query_param(query, "ssh_port").unwrap_or_else(|| "22".into());
        let user = parse_query_param(query, "ssh_user").unwrap_or_else(|| "root".into());
        (host, port, user, None)
    } else {
        return Ok(None);
    };

    if !validate_ssh_input(&host) || !validate_ssh_input(&port) || !validate_ssh_input(&user) {
        return Err("Invalid SSH params".into());
    }

    let mut cmd = String::new();
    let using_saved_password = password.is_some();
    if let Some(pw) = password {
        // sshpass reads the password from a 0600 file that is deleted shortly
        // after: it never appears in `ps` output or in the environment.
        let pw_file = ssh_auth::write_password_file(&pw)?;
        let path = pw_file.delete_after(PW_FILE_TTL_SECS);
        cmd.push_str(&format!("sshpass -f {} ", path));
    }
    cmd.push_str("ssh -o StrictHostKeyChecking=accept-new ");
    if using_saved_password {
        // Go straight to password auth. Offering keys first can exhaust the
        // server's MaxAuthTries before the password is ever tried, which the
        // server then reports as a plain "Permission denied".
        cmd.push_str(&ssh_auth::password_auth_opts().join(" "));
        cmd.push(' ');
    }
    if keepalive {
        cmd.push_str(&ssh_auth::keepalive_opts(ka_interval).join(" "));
        cmd.push(' ');
    } else {
        cmd.push_str("-o ServerAliveInterval=30 ");
    }
    cmd.push_str(&format!("-p {} {}@{}", port, user, host));

    // When ssh fails (refused, wrong password, unreachable) it exits in well
    // under a second. Without this guard tmux would tear the pane down before
    // the browser ever rendered the reason, and the client — seeing only a
    // closed socket — would reconnect forever. Holding the pane open on a
    // non-zero exit keeps the message on screen; a clean logout still ends the
    // session, which the browser is told about explicitly.
    let hint = if using_saved_password {
        "저장된 비밀번호가 거부되면(exit 5) 서버 편집에서 비밀번호를 다시 저장하거나,          \"직접 입력\" 버튼으로 접속해 보세요."
    } else {
        "위의 메시지에서 원인을 확인하세요."
    };
    let guarded = format!(
        concat!(
            r#"{}; __code=$?; if [ "$__code" -ne 0 ]; then "#,
            r#"printf '
[1;31m[SSH 연결 종료 · exit %s][0m "#,
            r#"{} Enter 키를 누르면 세션이 닫힙니다.
' "$__code"; "#,
            // Bounded wait: the operator can close it with Enter, and an
            // unattended failed session does not linger for ever.
            r#"if command -v timeout >/dev/null 2>&1; then "#,
            r#"timeout --foreground {} sh -c 'read -r __nabiman_wait'; else read -r __nabiman_wait; fi; fi"#,
        ),
        cmd, hint, FAILED_PANE_LINGER_SECS,
    );

    Ok(Some((guarded, format!("ssh:{}@{}:{}", user, host, port))))
}

#[get("/api/terminal")]
async fn ws_terminal(
    req: HttpRequest, stream: web::Payload,
    secret: web::Data<JwtSecret>, store: web::Data<PtyStore>,
) -> Result<HttpResponse, actix_web::Error> {
    let query = req.query_string();
    if !parse_query_param(query, "token").map(|t| crate::auth::check_auth_token(&secret, &t)).unwrap_or(false) {
        return Ok(HttpResponse::Unauthorized().json(crate::models::ApiResponse::<()>::error("Unauthorized")));
    }
    let username = parse_query_param(query, "token")
        .and_then(|t| crate::auth::decode_claims_unverified(&t).ok())
        .map(|d| d.claims.sub).unwrap_or_default();

    // Reconnect/join existing session via tmux attach
    let join_only = parse_query_param(query, "join").map(|v| v == "1").unwrap_or(false);
    if let Some(sid) = parse_query_param(query, "session_id") {
        let can_join = {
            let map = store.lock().unwrap();
            map.get(&sid).map(|s| s.owner == username || s.shared).unwrap_or(false)
        };
        if can_join {
            let tmux_name = {
                let map = store.lock().unwrap();
                map.get(&sid).map(|s| s.tmux_name.clone())
            };
            if let Some(name) = tmux_name {
                if tmux_session_exists(&name) {
                    match attach_tmux(&name, parse_size(query)) {
                        Ok((fd, pid)) => {
                            {
                                let mut map = store.lock().unwrap();
                                if let Some(s) = map.get_mut(&sid) {
                                    s.ws_count += 1;
                                    s.last_active_unix = now_unix();
                                }
                                persist(&map);
                            }
                            let file = unsafe { std::fs::File::from_raw_fd(fd) };
                            return ws::start(WsBridge {
                                session_id: sid, tmux_name: name, store: store.get_ref().clone(),
                                master_fd: fd, master_file: Some(file), child_pid: pid, ended: false,
                                read_only: false, mirror_size: false, last_size: None, client_tty: None,
                            }, &req, stream);
                        }
                        Err(e) => return Ok(HttpResponse::InternalServerError().json(crate::models::ApiResponse::<()>::error(&e))),
                    }
                }
            }
        }
    }

    // The browser asked for one specific session and it is no longer there.
    if join_only {
        return ws::start(
            NoticeSocket::new("이 세션은 종료되었습니다 / session no longer exists"),
            &req, stream,
        );
    }

    // --- New session ---
    let user_settings = settings_for(&username);
    let scrollback = parse_query_param(query, "scrollback")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(user_settings.scrollback_lines)
        .clamp(100, MAX_SCROLLBACK);
    let keepalive = parse_query_param(query, "keepalive")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(user_settings.keepalive);
    let timeout_secs = if keepalive {
        0
    } else {
        parse_query_param(query, "timeout")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(user_settings.idle_timeout_secs)
            .min(MAX_TIMEOUT_SECS)
    };

    let (shell_cmd, label) = match build_ssh_command(query, keepalive, user_settings.keepalive_interval_secs) {
        Ok(Some((cmd, label))) => (Some(cmd), label),
        Ok(None) => (None, "local".to_string()),
        // A target we cannot resolve (deleted server, bad parameters) is
        // reported on screen instead of failing the handshake, which the
        // browser could only read as "connection lost" and keep retrying.
        Err(e) => return ws::start(NoticeSocket::new(&e), &req, stream),
    };

    let sid = generate_session_id();
    let safe_user: String = username.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
    let tmux_name = format!("nb_{}_{}", safe_user, sid);

    let size = parse_size(query);
    if let Err(e) = create_tmux_session(&tmux_name, shell_cmd.as_deref(), scrollback, size) {
        return Ok(HttpResponse::InternalServerError().json(crate::models::ApiResponse::<()>::error(&format!("tmux: {}", e))));
    }

    let (fd, pid) = match attach_tmux(&tmux_name, size) {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::InternalServerError().json(crate::models::ApiResponse::<()>::error(&e))),
    };

    {
        let mut map = store.lock().unwrap();
        map.insert(sid.clone(), TmuxSession {
            tmux_name: tmux_name.clone(), owner: username, label, ssh_cmd: shell_cmd,
            created_unix: now_unix(), last_active_unix: now_unix(), ws_count: 1, shared: false,
            timeout_secs, scrollback, keepalive,
        });
        persist(&map);
    }

    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    ws::start(WsBridge {
        session_id: sid, tmux_name, store: store.get_ref().clone(),
        master_fd: fd, master_file: Some(file), child_pid: pid, ended: false,
        read_only: false, mirror_size: false, last_size: None, client_tty: None,
    }, &req, stream)
}

// --- Helpers ---
pub(crate) fn parse_query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|p| {
        let mut kv = p.splitn(2, '=');
        if kv.next() == Some(key) { kv.next().map(urlencoding_decode) } else { None }
    })
}
fn urlencoding_decode(s: &str) -> String {
    let mut r = String::with_capacity(s.len());
    let mut c = s.bytes();
    while let Some(b) = c.next() {
        if b == b'%' { let h = c.next().unwrap_or(b'0'); let l = c.next().unwrap_or(b'0');
            r.push((hv(h)*16+hv(l)) as char); }
        else if b == b'+' { r.push(' '); } else { r.push(b as char); }
    }
    r
}
fn hv(b: u8) -> u8 { match b { b'0'..=b'9'=>b-b'0', b'a'..=b'f'=>b-b'a'+10, b'A'..=b'F'=>b-b'A'+10, _=>0 } }
fn validate_ssh_input(s: &str) -> bool {
    !s.is_empty() && s.len() < 256 && s.chars().all(|c| c.is_alphanumeric() || ".-_@:".contains(c))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_terminal);
    handlers::configure(cfg);
    share::config(cfg);
}
