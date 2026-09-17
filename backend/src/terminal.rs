use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web::{get, web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::{Read, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::auth::JwtSecret;
use crate::ssh_auth;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const PTY_READ_INTERVAL: Duration = Duration::from_millis(30);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
const MAX_TIMEOUT_SECS: u64 = 30 * 24 * 3600; // 30 days
const MAX_SCROLLBACK: u32 = 200_000;
const PW_FILE_TTL_SECS: u64 = 30; // long enough for ssh to read it inside tmux
/// How long a pane whose ssh failed stays open so the reason can be read.
const FAILED_PANE_LINGER_SECS: u64 = 900;

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn data_dir() -> String {
    std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".to_string())
}

fn default_scrollback() -> u32 { 5000 }
fn default_true() -> bool { true }
fn default_ka_interval() -> u64 { 30 }
fn default_font_size() -> u32 { 14 }

// --- Per-user terminal settings -------------------------------------------

/// Preferences that outlive a browser: how much history to keep, whether the
/// session is kept alive server-side, and how big the text is.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TerminalSettings {
    #[serde(default = "default_scrollback")]
    pub scrollback_lines: u32,
    #[serde(default = "default_true")]
    pub keepalive: bool,
    #[serde(default = "default_ka_interval")]
    pub keepalive_interval_secs: u64,
    /// Seconds a detached session survives; 0 = never auto-closed.
    #[serde(default)]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    /// Replay the server-side history when a window reopens with an empty buffer.
    #[serde(default = "default_true")]
    pub restore_scrollback: bool,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            scrollback_lines: default_scrollback(),
            keepalive: true,
            keepalive_interval_secs: default_ka_interval(),
            idle_timeout_secs: 0,
            font_size: default_font_size(),
            restore_scrollback: true,
        }
    }
}

fn settings_file() -> String { format!("{}/terminal_settings.json", data_dir()) }

fn load_all_settings() -> HashMap<String, TerminalSettings> {
    std::fs::read_to_string(settings_file())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub fn settings_for(user: &str) -> TerminalSettings {
    load_all_settings().remove(user).unwrap_or_default()
}

fn save_settings_for(user: &str, settings: &TerminalSettings) -> Result<(), String> {
    let mut all = load_all_settings();
    all.insert(user.to_string(), settings.clone());
    let json = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    std::fs::write(settings_file(), json).map_err(|e| e.to_string())
}

// --- tmux-backed session store --------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TmuxSession {
    tmux_name: String,
    owner: String,
    label: String,
    #[serde(default)]
    #[allow(dead_code)]
    ssh_cmd: Option<String>,
    #[serde(default = "now_unix")]
    created_unix: u64,
    #[serde(default = "now_unix")]
    last_active_unix: u64,
    /// Live attachments. Not persisted — a restart detaches every browser.
    #[serde(skip)]
    ws_count: u32,
    #[serde(default)]
    shared: bool,
    /// 0 = permanent (keep-alive): the session is never reaped while detached.
    #[serde(default)]
    timeout_secs: u64,
    #[serde(default = "default_scrollback")]
    scrollback: u32,
    #[serde(default)]
    keepalive: bool,
}

pub type PtyStore = Arc<Mutex<HashMap<String, TmuxSession>>>;
pub fn new_pty_store() -> PtyStore { Arc::new(Mutex::new(HashMap::new())) }

fn sessions_file() -> String { format!("{}/terminal_sessions.json", data_dir()) }

/// Write the session registry to disk so a server restart does not orphan the
/// tmux sessions the browser still knows about.
fn persist(map: &HashMap<String, TmuxSession>) {
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = std::fs::create_dir_all(data_dir());
        let _ = std::fs::write(sessions_file(), json);
    }
}

/// Re-adopt sessions that survived a restart; drop the ones whose tmux session
/// is gone. Called once at startup.
pub fn restore_sessions(store: &PtyStore) {
    let saved: HashMap<String, TmuxSession> = std::fs::read_to_string(sessions_file())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default();
    let mut map = store.lock().unwrap();
    for (id, mut session) in saved {
        if tmux_session_exists(&session.tmux_name) {
            session.ws_count = 0;
            map.insert(id, session);
        }
    }
    if !map.is_empty() {
        println!("Terminal: restored {} live tmux session(s)", map.len());
    }
    persist(&map);
    ssh_auth::cleanup_password_files();
}

pub fn start_pty_cleanup(store: PtyStore) {
    std::thread::spawn(move || loop {
        std::thread::sleep(CLEANUP_INTERVAL);
        let mut map = store.lock().unwrap();
        let now = now_unix();
        let expired: Vec<String> = map.iter()
            .filter(|(_, s)| {
                s.ws_count == 0 && s.timeout_secs > 0
                    && now.saturating_sub(s.last_active_unix) > s.timeout_secs
            })
            .map(|(id, _)| id.clone()).collect();
        // A session whose tmux process died (host rebooted, user typed exit)
        // is dropped too, so the session list never shows ghosts.
        let dead: Vec<String> = map.iter()
            .filter(|(_, s)| s.ws_count == 0 && !tmux_session_exists(&s.tmux_name))
            .map(|(id, _)| id.clone()).collect();
        let mut changed = false;
        for id in expired {
            if let Some(s) = map.remove(&id) {
                let _ = std::process::Command::new("tmux").args(["kill-session", "-t", &s.tmux_name]).output();
                changed = true;
            }
        }
        for id in dead {
            map.remove(&id);
            changed = true;
        }
        if changed { persist(&map); }
    });
}

#[allow(dead_code)]
pub fn kill_user_sessions(store: &PtyStore, username: &str) {
    let mut map = store.lock().unwrap();
    let ids: Vec<String> = map.iter().filter(|(_, s)| s.owner == username).map(|(id, _)| id.clone()).collect();
    for id in ids {
        if let Some(s) = map.remove(&id) {
            let _ = std::process::Command::new("tmux").args(["kill-session", "-t", &s.tmux_name]).output();
        }
    }
    persist(&map);
}

fn generate_session_id() -> String {
    use rand::Rng;
    format!("pty_{:016x}", rand::thread_rng().gen::<u64>())
}

fn tmux_session_exists(name: &str) -> bool {
    std::process::Command::new("tmux").args(["has-session", "-t", name])
        .output().map(|o| o.status.success()).unwrap_or(false)
}

/// Seed options for a tmux server this process may be about to start.
fn write_tmux_conf(scrollback: u32) -> Option<String> {
    let path = format!("{}/tmux.conf", data_dir());
    let body = format!("set -g history-limit {}\nset -g mouse on\n", scrollback);
    std::fs::create_dir_all(data_dir()).ok()?;
    std::fs::write(&path, body).ok()?;
    Some(path)
}

fn create_tmux_session(name: &str, command: Option<&str>, scrollback: u32, size: (u16, u16)) -> Result<(), String> {
    // history-limit only applies to panes created when they are made, so the
    // value has to be in place *before* the session is spawned. Two paths cover
    // both cases: -f seeds the options when this command starts the tmux server
    // (the first session after a reboot), and set-option -g covers every later
    // session on an already running server.
    let conf = write_tmux_conf(scrollback);
    let _ = std::process::Command::new("tmux")
        .args(["set-option", "-g", "history-limit", &scrollback.to_string()])
        .output();

    // Create the pane at the size the browser already has: attaching a smaller
    // client would otherwise shrink the pane and push the first lines (an ssh
    // error, for instance) straight out of view.
    let cols = size.0.to_string();
    let rows = size.1.to_string();
    let mut args: Vec<&str> = Vec::new();
    if let Some(ref path) = conf {
        args.push("-f");
        args.push(path);
    }
    args.extend(["new-session", "-d", "-s", name, "-x", &cols, "-y", &rows]);
    if let Some(cmd) = command {
        args.push(cmd);
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
    let _ = std::process::Command::new("tmux")
        .args(["set-option", "-t", name, "history-limit", &scrollback.to_string()])
        .output();
    // Detaching a browser must never tear the session down.
    let _ = std::process::Command::new("tmux")
        .args(["set-option", "-t", name, "destroy-unattached", "off"])
        .output();
    Ok(())
}

/// Read back the pane history (ANSI colours included) so a reopened window can
/// show what scrolled past while it was closed.
fn capture_scrollback(name: &str, lines: u32) -> Result<String, String> {
    let start = format!("-{}", lines.min(MAX_SCROLLBACK));
    let out = std::process::Command::new("tmux")
        .args(["capture-pane", "-p", "-e", "-J", "-S", &start, "-t", name])
        .output().map_err(|e| format!("tmux: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Spawn a PTY running `tmux attach -t <name>` and return (master_fd, child_pid)
fn attach_tmux(name: &str, size: (u16, u16)) -> Result<(i32, nix::unistd::Pid), String> {
    let cmd = ShellCommand { program: "tmux".into(), args: vec!["attach-session".into(), "-t".into(), name.into()] };
    spawn_pty(cmd, size)
}

/// Terminal size requested by the browser, clamped to something tmux accepts.
fn parse_size(query: &str) -> (u16, u16) {
    let cols = parse_query_param(query, "cols").and_then(|v| v.parse::<u16>().ok()).unwrap_or(120);
    let rows = parse_query_param(query, "rows").and_then(|v| v.parse::<u16>().ok()).unwrap_or(40);
    (cols.clamp(20, 500), rows.clamp(5, 200))
}

// --- Shell / PTY ---
struct ShellCommand { program: String, args: Vec<String> }

fn spawn_pty(cmd: ShellCommand, size: (u16, u16)) -> Result<(i32, nix::unistd::Pid), String> {
    let winsize = nix::pty::Winsize { ws_row: size.1, ws_col: size.0, ws_xpixel: 0, ws_ypixel: 0 };
    let pty = nix::pty::openpty(Some(&winsize), None).map_err(|e| format!("openpty: {}", e))?;
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
    tmux_name: String,
    store: PtyStore,
    master_fd: i32,
    master_file: Option<std::fs::File>,
    child_pid: nix::unistd::Pid,
    /// Set once the shell really exited, so the browser stops reconnecting.
    ended: bool,
}

/// Sent to the browser when the session is gone for good (the shell exited, or
/// somebody killed it) as opposed to a network drop, which the browser retries.
const ENDED_MSG: &str = "\x02ENDED";

/// A WebSocket that explains why it cannot serve this pane and closes — used
/// when a browser asks for a session that is gone or a target that no longer
/// resolves. The human-readable line lands on the screen; ENDED stops the
/// browser from retrying.
struct NoticeSocket(String);

impl NoticeSocket {
    fn new(reason: &str) -> Self {
        Self(format!("\r\n\x1b[33m[{}]\x1b[0m\r\n", reason))
    }
}

impl Actor for NoticeSocket {
    type Context = ws::WebsocketContext<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.text(self.0.clone());
        ctx.text(ENDED_MSG);
        ctx.run_later(Duration::from_millis(80), |_, ctx| ctx.stop());
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for NoticeSocket {
    fn handle(&mut self, _: Result<ws::Message, ws::ProtocolError>, _: &mut Self::Context) {}
}

impl WsBridge {
    /// The PTY hit EOF: either the tmux session ended (shell exited / killed)
    /// or the attach died. Tell the browser which one before closing, so a
    /// deliberate exit is not answered with an automatic new session.
    fn finish(&mut self, ctx: &mut ws::WebsocketContext<Self>) {
        if self.ended { return; }
        self.ended = true;
        if !tmux_session_exists(&self.tmux_name) {
            if let Ok(mut map) = self.store.lock() {
                map.remove(&self.session_id);
                persist(&map);
            }
            ctx.text(ENDED_MSG);
            // Give the frame a moment to flush before the socket closes.
            ctx.run_later(Duration::from_millis(80), |_, ctx| ctx.stop());
            return;
        }
        ctx.stop();
    }
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
                    Ok(0) => { act.finish(ctx); break; }
                    Ok(n) => ctx.binary(buf[..n].to_vec()),
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => { act.finish(ctx); break; }
                }
            }
        });
        ctx.run_interval(HEARTBEAT_INTERVAL, |_, ctx| { ctx.ping(b""); });
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        // Close PTY fd properly — this kills the tmux attach process, while the
        // tmux session itself keeps running with the shell inside it.
        drop(self.master_file.take());
        let _ = nix::sys::wait::waitpid(self.child_pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG));
        if let Ok(mut map) = self.store.lock() {
            if let Some(s) = map.get_mut(&self.session_id) {
                s.ws_count = s.ws_count.saturating_sub(1);
                s.last_active_unix = now_unix();
            }
            persist(&map);
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
struct SessionInfo {
    id: String, label: String, owner: String, ws_count: u32, shared: bool,
    age_secs: u64, timeout_secs: u64, scrollback: u32, keepalive: bool, alive: bool,
}

async fn list_sessions(store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let username = extract_username_header(&req);
    let map = store.lock().unwrap();
    let now = now_unix();
    let list: Vec<SessionInfo> = map.iter()
        .filter(|(_, s)| s.owner == username || s.shared)
        .map(|(id, s)| SessionInfo {
            id: id.clone(), label: s.label.clone(), owner: s.owner.clone(),
            ws_count: s.ws_count, shared: s.shared,
            age_secs: now.saturating_sub(s.created_unix),
            timeout_secs: s.timeout_secs, scrollback: s.scrollback,
            keepalive: s.keepalive, alive: tmux_session_exists(&s.tmux_name),
        }).collect();
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(list))
}

#[derive(serde::Deserialize)]
struct ShareRequest { shared: bool }

#[derive(serde::Deserialize)]
struct TimeoutRequest { timeout_secs: u64 }

#[derive(serde::Deserialize)]
struct KeepaliveRequest { keepalive: bool }

#[derive(serde::Deserialize)]
struct ScrollbackQuery { lines: Option<u32> }

fn may_touch(map: &HashMap<String, TmuxSession>, id: &str, user: &str) -> bool {
    map.get(id).map(|s| s.owner == user).unwrap_or(false)
}

async fn toggle_share(path: web::Path<String>, body: web::Json<ShareRequest>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let mut map = store.lock().unwrap();
    if !map.contains_key(&id) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if !may_touch(&map, &id, &username) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner"));
    }
    if let Some(s) = map.get_mut(&id) { s.shared = body.shared; }
    persist(&map);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(
        if body.shared { "Session shared" } else { "Session unshared" }))
}

async fn set_timeout(path: web::Path<String>, body: web::Json<TimeoutRequest>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    if body.timeout_secs > MAX_TIMEOUT_SECS {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Timeout too large (max 30 days, 0 = permanent)"));
    }
    let mut map = store.lock().unwrap();
    if !map.contains_key(&id) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if !may_touch(&map, &id, &username) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner"));
    }
    if let Some(s) = map.get_mut(&id) {
        s.timeout_secs = body.timeout_secs;
        if body.timeout_secs != 0 { s.keepalive = false; }
    }
    persist(&map);
    let label = if body.timeout_secs == 0 { "permanent".to_string() } else { format!("{}s", body.timeout_secs) };
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(format!("Timeout set to {}", label)))
}

/// Turn "keep this session alive" on or off for a running session.
async fn set_keepalive(path: web::Path<String>, body: web::Json<KeepaliveRequest>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let mut map = store.lock().unwrap();
    if !map.contains_key(&id) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if !may_touch(&map, &id, &username) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner"));
    }
    let user_settings = settings_for(&username);
    if let Some(s) = map.get_mut(&id) {
        s.keepalive = body.keepalive;
        s.timeout_secs = if body.keepalive { 0 } else { user_settings.idle_timeout_secs };
    }
    persist(&map);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(
        if body.keepalive { "Keep-alive enabled" } else { "Keep-alive disabled" }))
}

async fn kill_session(path: web::Path<String>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let mut map = store.lock().unwrap();
    if !may_touch(&map, &id, &username) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if let Some(s) = map.remove(&id) {
        let _ = std::process::Command::new("tmux").args(["kill-session", "-t", &s.tmux_name]).output();
    }
    persist(&map);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok("Session closed"))
}

async fn get_scrollback(path: web::Path<String>, query: web::Query<ScrollbackQuery>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let (tmux_name, lines) = {
        let map = store.lock().unwrap();
        match map.get(&id) {
            Some(s) if s.owner == username || s.shared => {
                (s.tmux_name.clone(), query.lines.unwrap_or(s.scrollback))
            }
            _ => return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found")),
        }
    };
    match capture_scrollback(&tmux_name, lines.min(MAX_SCROLLBACK)) {
        Ok(text) => HttpResponse::Ok().json(crate::models::ApiResponse::ok(text)),
        Err(e) => HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(&e)),
    }
}

async fn get_settings(req: HttpRequest) -> HttpResponse {
    let username = extract_username_header(&req);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(settings_for(&username)))
}

async fn put_settings(body: web::Json<TerminalSettings>, req: HttpRequest) -> HttpResponse {
    let username = extract_username_header(&req);
    let mut settings = body.into_inner();
    if settings.scrollback_lines < 100 || settings.scrollback_lines > MAX_SCROLLBACK {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "scrollback_lines must be between 100 and 200000"));
    }
    if settings.idle_timeout_secs > MAX_TIMEOUT_SECS {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "idle_timeout_secs must be 0 (permanent) or at most 30 days"));
    }
    settings.keepalive_interval_secs = settings.keepalive_interval_secs.clamp(5, 3600);
    settings.font_size = settings.font_size.clamp(8, 28);
    if settings.keepalive { settings.idle_timeout_secs = 0; }
    match save_settings_for(&username, &settings) {
        Ok(()) => HttpResponse::Ok().json(crate::models::ApiResponse::ok(settings)),
        Err(e) => HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(&e)),
    }
}

fn extract_username_header(req: &HttpRequest) -> String {
    req.headers().get("Authorization")
        .and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|t| crate::auth::decode_claims_unverified(t).ok())
        .map(|d| d.claims.sub).unwrap_or_default()
}

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
    }, &req, stream)
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
       .route("/api/terminal/settings", web::get().to(get_settings))
       .route("/api/terminal/settings", web::put().to(put_settings))
       .route("/api/terminal/sessions", web::get().to(list_sessions))
       .route("/api/terminal/sessions/{id}", web::delete().to(kill_session))
       .route("/api/terminal/sessions/{id}/share", web::post().to(toggle_share))
       .route("/api/terminal/sessions/{id}/timeout", web::post().to(set_timeout))
       .route("/api/terminal/sessions/{id}/keepalive", web::post().to(set_keepalive))
       .route("/api/terminal/sessions/{id}/scrollback", web::get().to(get_scrollback));
}
