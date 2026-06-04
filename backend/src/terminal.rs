use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web::{get, web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::{Read, Write};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::auth::JwtSecret;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const PTY_READ_INTERVAL: Duration = Duration::from_millis(30);
const DEFAULT_TIMEOUT_SECS: u64 = 1800; // 30 min default
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

// --- tmux-backed session store ---

pub struct TmuxSession {
    tmux_name: String,
    owner: String,
    label: String,
    ssh_cmd: Option<String>,
    created_at: Instant,
    last_active: Instant,
    ws_count: u32,
    shared: bool,
    timeout_secs: u64, // 0 = permanent (no auto-kill)
}

pub type PtyStore = Arc<Mutex<HashMap<String, TmuxSession>>>;
pub fn new_pty_store() -> PtyStore { Arc::new(Mutex::new(HashMap::new())) }

pub fn start_pty_cleanup(store: PtyStore) {
    std::thread::spawn(move || loop {
        std::thread::sleep(CLEANUP_INTERVAL);
        let mut map = store.lock().unwrap();
        let expired: Vec<String> = map.iter()
            .filter(|(_, s)| {
                s.ws_count == 0 && s.timeout_secs > 0
                    && s.last_active.elapsed() > Duration::from_secs(s.timeout_secs)
            })
            .map(|(id, _)| id.clone()).collect();
        for id in expired {
            if let Some(s) = map.remove(&id) {
                let _ = std::process::Command::new("tmux").args(["kill-session", "-t", &s.tmux_name]).output();
            }
        }
    });
}

pub fn kill_user_sessions(store: &PtyStore, username: &str) {
    let mut map = store.lock().unwrap();
    let ids: Vec<String> = map.iter().filter(|(_, s)| s.owner == username).map(|(id, _)| id.clone()).collect();
    for id in ids {
        if let Some(s) = map.remove(&id) {
            let _ = std::process::Command::new("tmux").args(["kill-session", "-t", &s.tmux_name]).output();
        }
    }
}

fn generate_session_id() -> String {
    use rand::Rng;
    format!("pty_{:016x}", rand::thread_rng().gen::<u64>())
}

fn tmux_session_exists(name: &str) -> bool {
    std::process::Command::new("tmux").args(["has-session", "-t", name])
        .output().map(|o| o.status.success()).unwrap_or(false)
}

fn create_tmux_session(name: &str, ssh_cmd: Option<&str>) -> Result<(), String> {
    let mut args = vec!["new-session", "-d", "-s", name, "-x", "120", "-y", "40"];
    // If SSH, run ssh command inside tmux
    let ssh_full;
    if let Some(cmd) = ssh_cmd {
        ssh_full = format!("ssh -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 {}", cmd);
        args.push(&ssh_full);
    }
    let out = std::process::Command::new("tmux").args(&args).output()
        .map_err(|e| format!("tmux: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    // Enable mouse so the wheel scrolls tmux's own history (copy-mode) like a
    // native terminal — otherwise, in tmux's alternate screen, xterm.js turns
    // the wheel into arrow keys, which bash treats as command-history nav.
    let _ = std::process::Command::new("tmux")
        .args(["set-option", "-t", name, "mouse", "on"])
        .output();
    Ok(())
}

/// Spawn a PTY running `tmux attach -t <name>` and return (master_fd, child_pid)
fn attach_tmux(name: &str) -> Result<(i32, nix::unistd::Pid), String> {
    let cmd = ShellCommand { program: "tmux".into(), args: vec!["attach-session".into(), "-t".into(), name.into()] };
    spawn_pty(cmd)
}

// --- Shell / PTY ---
struct ShellCommand { program: String, args: Vec<String> }

fn spawn_pty(cmd: ShellCommand) -> Result<(i32, nix::unistd::Pid), String> {
    let pty = nix::pty::openpty(None, None).map_err(|e| format!("openpty: {}", e))?;
    let master_fd = pty.master.as_raw_fd();
    let slave_fd = pty.slave.as_raw_fd();
    match unsafe { nix::unistd::fork() } {
        Ok(nix::unistd::ForkResult::Child) => {
            drop(pty.master);
            let _ = nix::unistd::setsid();
            unsafe { libc::ioctl(slave_fd, libc::TIOCSCTTY, 0) };
            let _ = nix::unistd::dup2(slave_fd, 0);
            let _ = nix::unistd::dup2(slave_fd, 1);
            let _ = nix::unistd::dup2(slave_fd, 2);
            if slave_fd > 2 { let _ = nix::unistd::close(slave_fd); }
            std::env::set_var("TERM", "xterm-256color");
            std::env::set_var("COLORTERM", "truecolor");
            std::env::set_var("LANG", "en_US.UTF-8");
            let _ = std::process::Command::new(&cmd.program).args(&cmd.args)
                .stdin(unsafe { std::process::Stdio::from_raw_fd(0) })
                .stdout(unsafe { std::process::Stdio::from_raw_fd(1) })
                .stderr(unsafe { std::process::Stdio::from_raw_fd(2) }).status();
            std::process::exit(0);
        }
        Ok(nix::unistd::ForkResult::Parent { child }) => {
            drop(pty.slave);
            let flags = nix::fcntl::fcntl(master_fd, nix::fcntl::FcntlArg::F_GETFL)
                .map_err(|e| format!("fcntl: {}", e))?;
            let mut oflags = nix::fcntl::OFlag::from_bits_truncate(flags);
            oflags.insert(nix::fcntl::OFlag::O_NONBLOCK);
            nix::fcntl::fcntl(master_fd, nix::fcntl::FcntlArg::F_SETFL(oflags))
                .map_err(|e| format!("fcntl: {}", e))?;
            std::mem::forget(pty.master);
            Ok((master_fd, child))
        }
        Err(e) => Err(format!("fork: {}", e)),
    }
}

// --- WebSocket Bridge ---
struct WsBridge {
    session_id: String,
    store: PtyStore,
    master_fd: i32,
    master_file: Option<std::fs::File>,
    child_pid: nix::unistd::Pid,
}

impl Actor for WsBridge {
    type Context = ws::WebsocketContext<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.text(format!("\x02SESSION:{}", self.session_id));
        ctx.run_interval(PTY_READ_INTERVAL, |act, ctx| {
            let file = match act.master_file.as_mut() { Some(f) => f, None => { ctx.stop(); return; } };
            let mut buf = [0u8; 8192];
            loop {
                match file.read(&mut buf) {
                    Ok(0) => { ctx.stop(); break; }
                    Ok(n) => ctx.binary(buf[..n].to_vec()),
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => { ctx.stop(); break; }
                }
            }
        });
        ctx.run_interval(HEARTBEAT_INTERVAL, |_, ctx| { ctx.ping(b""); });
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        // Close PTY fd properly — this kills the tmux attach process
        drop(self.master_file.take());
        let _ = nix::sys::wait::waitpid(self.child_pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG));
        // Decrement ws_count
        if let Ok(mut map) = self.store.lock() {
            if let Some(s) = map.get_mut(&self.session_id) {
                s.ws_count = s.ws_count.saturating_sub(1);
                s.last_active = Instant::now();
            }
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsBridge {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                if text.starts_with("\x01RESIZE:") {
                    let parts: Vec<&str> = text[8..].split(':').collect();
                    if parts.len() == 2 {
                        if let (Ok(cols), Ok(rows)) = (parts[0].parse::<u16>(), parts[1].parse::<u16>()) {
                            let ws = nix::pty::Winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
                            unsafe { libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &ws) };
                            let _ = nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGWINCH);
                        }
                    }
                    return;
                }
                if let Some(f) = self.master_file.as_mut() { let _ = f.write_all(text.as_bytes()); }
            }
            Ok(ws::Message::Binary(data)) => { if let Some(f) = self.master_file.as_mut() { let _ = f.write_all(&data); } }
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Close(_)) => ctx.stop(),
            _ => {}
        }
    }
}

// --- API Handlers ---

#[derive(serde::Serialize)]
struct SessionInfo { id: String, label: String, owner: String, ws_count: u32, shared: bool, age_secs: u64, timeout_secs: u64 }

async fn list_sessions(store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let username = extract_username_header(&req);
    let map = store.lock().unwrap();
    let list: Vec<SessionInfo> = map.iter()
        .filter(|(_, s)| s.owner == username || s.shared)
        .map(|(id, s)| SessionInfo {
            id: id.clone(), label: s.label.clone(), owner: s.owner.clone(),
            ws_count: s.ws_count, shared: s.shared, age_secs: s.created_at.elapsed().as_secs(),
            timeout_secs: s.timeout_secs,
        }).collect();
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(list))
}

#[derive(serde::Deserialize)]
struct ShareRequest { shared: bool }

#[derive(serde::Deserialize)]
struct TimeoutRequest { timeout_secs: u64 }

async fn toggle_share(path: web::Path<String>, body: web::Json<ShareRequest>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let mut map = store.lock().unwrap();
    if let Some(s) = map.get_mut(&id) {
        if s.owner != username { return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner")); }
        s.shared = body.shared;
        return HttpResponse::Ok().json(crate::models::ApiResponse::ok(if body.shared { "Session shared" } else { "Session unshared" }));
    }
    HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"))
}

async fn set_timeout(path: web::Path<String>, body: web::Json<TimeoutRequest>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let allowed = [0, 1800, 3600, 86400]; // permanent, 30m, 1h, 24h
    if !allowed.contains(&body.timeout_secs) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Allowed: 0 (permanent), 1800 (30m), 3600 (1h), 86400 (24h)"));
    }
    let mut map = store.lock().unwrap();
    if let Some(s) = map.get_mut(&id) {
        if s.owner != username { return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner")); }
        s.timeout_secs = body.timeout_secs;
        let label = if body.timeout_secs == 0 { "permanent" } else { &format!("{}s", body.timeout_secs) };
        return HttpResponse::Ok().json(crate::models::ApiResponse::ok(format!("Timeout set to {}", label)));
    }
    HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"))
}

fn extract_username_header(req: &HttpRequest) -> String {
    req.headers().get("Authorization")
        .and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|t| crate::auth::decode_claims_unverified(t).ok())
        .map(|d| d.claims.sub).unwrap_or_default()
}

// --- WebSocket endpoint ---

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
                    match attach_tmux(&name) {
                        Ok((fd, pid)) => {
                            store.lock().unwrap().get_mut(&sid).map(|s| { s.ws_count += 1; s.last_active = Instant::now(); });
                            let file = unsafe { std::fs::File::from_raw_fd(fd) };
                            return ws::start(WsBridge { session_id: sid, store: store.get_ref().clone(), master_fd: fd, master_file: Some(file), child_pid: pid }, &req, stream);
                        }
                        Err(e) => return Ok(HttpResponse::InternalServerError().json(crate::models::ApiResponse::<()>::error(&e))),
                    }
                }
            }
        }
    }

    // Create new tmux session
    let sid = generate_session_id();
    let tmux_name = format!("nb_{}_{}", &username, &sid[4..12]);

    let (ssh_cmd, label) = if let Some(host) = parse_query_param(query, "ssh_host") {
        let port = parse_query_param(query, "ssh_port").unwrap_or_else(|| "22".into());
        let user = parse_query_param(query, "ssh_user").unwrap_or_else(|| "root".into());
        if !validate_ssh_input(&host) || !validate_ssh_input(&port) || !validate_ssh_input(&user) {
            return Ok(HttpResponse::BadRequest().json(crate::models::ApiResponse::<()>::error("Invalid SSH params")));
        }
        let cmd = format!("{}@{} -p {}", user, host, port);
        (Some(cmd.clone()), format!("ssh:{}@{}:{}", user, host, port))
    } else {
        (None, "local".to_string())
    };

    if let Err(e) = create_tmux_session(&tmux_name, ssh_cmd.as_deref()) {
        return Ok(HttpResponse::InternalServerError().json(crate::models::ApiResponse::<()>::error(&format!("tmux: {}", e))));
    }

    let (fd, pid) = match attach_tmux(&tmux_name) {
        Ok(r) => r,
        Err(e) => return Ok(HttpResponse::InternalServerError().json(crate::models::ApiResponse::<()>::error(&e))),
    };

    let timeout_secs = parse_query_param(query, "timeout")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    store.lock().unwrap().insert(sid.clone(), TmuxSession {
        tmux_name, owner: username, label, ssh_cmd,
        created_at: Instant::now(), last_active: Instant::now(), ws_count: 1, shared: false,
        timeout_secs,
    });

    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    ws::start(WsBridge { session_id: sid, store: store.get_ref().clone(), master_fd: fd, master_file: Some(file), child_pid: pid }, &req, stream)
}

// --- Helpers ---
fn parse_query_param(query: &str, key: &str) -> Option<String> {
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
    cfg.service(ws_terminal)
       .route("/api/terminal/sessions", web::get().to(list_sessions))
       .route("/api/terminal/sessions/{id}/share", web::post().to(toggle_share))
       .route("/api/terminal/sessions/{id}/timeout", web::post().to(set_timeout));
}
