//! The terminal WebSocket: one browser pane attached to one tmux session.

#[path = "ssh_command.rs"]
mod ssh_command;
use ssh_command::build_ssh_command;

mod claude_voice;
mod dictation;
mod handlers;
mod history;
mod paste_file;
mod pty;
pub mod recording;
mod registry;
pub(crate) mod remote_access;
pub(crate) mod session;
mod session_settings;
mod settings;
pub(crate) mod sftp;
pub mod share;
pub mod vnc_console;
pub mod vnc_registry;
pub mod rdp_tunnel;
pub mod vnc_access;
pub mod console_share;
mod console_handlers;
pub mod console_drive;
pub mod console_creds;
pub mod share_store;
pub(crate) mod share_ticket;
pub mod telnet_client;

use actix_web::{get, web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::os::unix::io::FromRawFd;

pub use registry::open as open_registry;
pub use session::{restore_sessions, start_pty_cleanup, PtyStore};
pub use settings::{open as open_settings_registry, SettingsStore};

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

#[get("/api/terminal")]
async fn ws_terminal(
    req: HttpRequest,
    stream: web::Payload,
    store: web::Data<PtyStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    users: web::Data<crate::users::UserStore>,
    preferences: web::Data<SettingsStore>,
) -> Result<HttpResponse, actix_web::Error> {
    ws::handshake(&req)?;
    let query = req.query_string();
    // A dictation ticket is for dictation, not for a shell.
    if parse_query_param(query, "purpose").is_some() {
        return Ok(HttpResponse::BadRequest().finish());
    }
    let Some(username) = crate::auth::extract_username(&req) else {
        return Ok(HttpResponse::Unauthorized().finish());
    };

    let Some(grant) = crate::stream_access::ConnectionGrant::from_request(
        &req,
        sessions.get_ref().clone(),
        users.get_ref().clone(),
    )
    .filter(|g| g.valid()) else {
        return Ok(HttpResponse::Unauthorized().finish());
    };

    // Reconnect/join existing session via tmux attach
    let join_only = parse_query_param(query, "join")
        .map(|v| v == "1")
        .unwrap_or(false);
    if let Some(sid) = parse_query_param(query, "session_id") {
        let revoked = store
            .lock()
            .ok()
            .and_then(|m| m.get(&sid).cloned())
            .is_some_and(|s| !grant.can_access_session(&s) || !remote_access::allowed(&s));
        if revoked {
            return ws::start(
                NoticeSocket::new("Session ownership or server approval is no longer valid; open a new authorized session"),
                &req,
                stream,
            );
        }
        let can_join = {
            let map = store.lock().unwrap();
            map.get(&sid)
                .map(|s| grant.can_access_session(s) && remote_access::allowed(s))
                .unwrap_or(false)
        };
        if can_join {
            let tmux_name = {
                let map = store.lock().unwrap();
                map.get(&sid).map(|s| s.tmux_name.clone())
            };
            if let Some(name) = tmux_name {
                if session::tmux_session_state(&name).is_err() {
                    return ws::start(
                        NoticeSocket::new(
                            "Unable to confirm this session; verify the terminal service",
                        ),
                        &req,
                        stream,
                    );
                }
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
                            return ws::start(
                                WsBridge {
                                    session_id: sid,
                                    tmux_name: name,
                                    store: store.get_ref().clone(),
                                    master_fd: fd,
                                    master_file: Some(file),
                                    child_pid: pid,
                                    ended: false,
                                    read_only: false,
                                    grant: Some(grant.clone()),
                                    share_grant: None,
                                    share_lease: None,
                                    mirror_size: false,
                                    last_size: None,
                                    client_tty: pty::slave_tty(fd),
                                    history: (parse_query_param(query, "history").as_deref()
                                        == Some("1"))
                                    .then(|| None),
                                },
                                &req,
                                stream,
                            );
                        }
                        Err(e) => {
                            return Ok(HttpResponse::InternalServerError()
                                .json(crate::models::ApiResponse::<()>::error(&e)))
                        }
                    }
                }
            }
        }
    }

    // The browser asked for one specific session and it is no longer there.
    if join_only {
        return ws::start(
            NoticeSocket::new("이 세션은 종료되었습니다 / session no longer exists"),
            &req,
            stream,
        );
    }

    // --- New session ---
    let user_settings = match preferences.lock().unwrap().get(&username) {
        Ok(settings) => settings,
        Err(_) => return ws::start(NoticeSocket::new("Terminal preferences unavailable; contact your administrator before opening a new session"), &req, stream),
    };
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

    let member = users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == grant.user_id && crate::member_servers::is_member(u))
        .cloned();
    // A member's session keeps to the idle limit an administrator set.
    let (keepalive, timeout_secs) = member
        .as_ref()
        .map_or((keepalive, timeout_secs), |m| crate::member_servers::session_terms(m, keepalive, timeout_secs));
    let (shell_cmd, label, remote_binding) =
        match build_ssh_command(query, keepalive, user_settings.keepalive_interval_secs, member.as_ref()) {
            Ok(Some((cmd, label, binding))) => (Some(cmd), label, Some(binding)),
            Ok(None) => (None, "local".to_string(), None),
            // A target we cannot resolve (deleted server, bad parameters) is
            // reported on screen instead of failing the handshake, which the
            // browser could only read as "connection lost" and keep retrying.
            Err(e) => return ws::start(NoticeSocket::new(&e), &req, stream),
        };

    if !grant.valid() || remote_binding.as_ref().is_some_and(|b| !b.valid()) {
        return Ok(HttpResponse::Forbidden().finish());
    }
    let sid = generate_session_id();
    let safe_user: String = username
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .scan(0usize, |bytes, c| {
            *bytes += c.len_utf8();
            (*bytes <= 128).then_some(c)
        })
        .collect();
    let tmux_name = format!("nb_{}_{}", safe_user, sid);

    let mut map = store.lock().unwrap();
    if let Some(why) = member
        .as_ref()
        .and_then(|m| crate::member_servers::session_limit(m, map.values().filter(|s| s.owned_by(&m.id)).count()))
    {
        drop(map);
        return ws::start(NoticeSocket::new(&why), &req, stream);
    }
    let candidate = match registry::prepare_insert(
        &map,
        sid.clone(),
        TmuxSession {
            tmux_name: tmux_name.clone(),
            owner: username,
            owner_id: grant.user_id.clone(),
            label,
            ssh_cmd: shell_cmd.clone(),
            remote_binding,
            created_unix: now_unix(),
            last_active_unix: now_unix(),
            ws_count: 0,
            shared: false,
            timeout_secs,
            scrollback,
            keepalive,
            title: String::new(),
        },
    ) {
        Ok(candidate) => candidate,
        Err(_) => {
            drop(map);
            return ws::start(
                NoticeSocket::new("Terminal capacity or account metadata limit reached; review existing sessions or contact your administrator"),
                &req,
                stream,
            );
        }
    };
    // The mutex spans validation and creation/publication: another admission
    // cannot consume the reserved space in between.
    if !grant.valid() || !remote_access::allowed(&candidate[&sid]) {
        return Ok(HttpResponse::Forbidden().finish());
    }
    let size = parse_size(query);
    if let Err(e) = create_tmux_session(&tmux_name, shell_cmd.as_deref(), scrollback, size) {
        return Ok(HttpResponse::InternalServerError().json(
            crate::models::ApiResponse::<()>::error(&format!("tmux: {}", e)),
        ));
    }

    // Record the successfully created shell even if attaching subsequently
    // fails, so it remains visible and can be retried or explicitly closed.
    *map = candidate;
    persist(&map);
    if let Some(s) = map.get(&sid) {
        recording::start(&tmux_name, &sid, &s.owner, &s.owner_id, &s.label, size);
    }

    let (fd, pid) = match attach_tmux(&tmux_name, size) {
        Ok(r) => r,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError()
                .json(crate::models::ApiResponse::<()>::error(&e)))
        }
    };

    map.get_mut(&sid).unwrap().ws_count = 1;
    drop(map);

    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    ws::start(
        WsBridge {
            session_id: sid,
            tmux_name,
            store: store.get_ref().clone(),
            master_fd: fd,
            master_file: Some(file),
            child_pid: pid,
            ended: false,
            read_only: false,
            grant: Some(grant),
            share_grant: None,
            share_lease: None,
            mirror_size: false,
            last_size: None,
            client_tty: pty::slave_tty(fd),
            history: (parse_query_param(query, "history").as_deref() == Some("1")).then(|| None),
        },
        &req,
        stream,
    )
}

// --- Helpers ---
pub(crate) fn parse_query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|p| {
        let mut kv = p.splitn(2, '=');
        if kv.next() == Some(key) {
            kv.next().map(urlencoding_decode)
        } else {
            None
        }
    })
}
fn urlencoding_decode(s: &str) -> String {
    let mut r = String::with_capacity(s.len());
    let mut c = s.bytes();
    while let Some(b) = c.next() {
        if b == b'%' {
            let h = c.next().unwrap_or(b'0');
            let l = c.next().unwrap_or(b'0');
            r.push((hv(h) * 16 + hv(l)) as char);
        } else if b == b'+' {
            r.push(' ');
        } else {
            r.push(b as char);
        }
    }
    r
}
fn hv(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}
fn validate_ssh_input(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 256
        && s.chars()
            .all(|c| c.is_alphanumeric() || ".-_@:".contains(c))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_terminal);
    handlers::configure(cfg);
    paste_file::config(cfg);
    session_settings::config(cfg);
    sftp::config(cfg);
    dictation::config(cfg);
    claude_voice::config(cfg);
    share::config(cfg);
    recording::config(cfg);
    vnc_console::config(cfg);
    rdp_tunnel::config(cfg);
    console_share::config(cfg);
    console_drive::config(cfg);
    console_creds::config(cfg);
}
