//! The PTY side: spawning `tmux attach` on a pseudo-terminal and bridging it
//! to a WebSocket.

use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web_actors::ws;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::time::Duration;

use super::parse_query_param;
use super::session::{client_size, now_unix, persist, tmux_session_exists, PtyStore};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const PTY_READ_INTERVAL: Duration = Duration::from_millis(30);
/// How often a mirroring client is told the pane size.
const SIZE_POLL_INTERVAL: Duration = Duration::from_millis(1500);

struct ShellCommand { program: String, args: Vec<String> }

/// The path tmux will report for a client running on this pseudo-terminal.
pub(crate) fn slave_tty(master_fd: i32) -> Option<String> {
    let name = unsafe { libc::ptsname(master_fd) };
    if name.is_null() { return None; }
    unsafe { std::ffi::CStr::from_ptr(name) }.to_str().ok().map(str::to_string)
}

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
pub(crate) struct WsBridge {
    pub(crate) session_id: String,
    pub(crate) tmux_name: String,
    pub(crate) store: PtyStore,
    pub(crate) master_fd: i32,
    pub(crate) master_file: Option<std::fs::File>,
    pub(crate) child_pid: nix::unistd::Pid,
    /// Set once the shell really exited, so the browser stops reconnecting.
    pub(crate) ended: bool,
    /// A share link may watch without typing; input is dropped on the way in.
    pub(crate) read_only: bool,
    /// The tty tmux knows this connection by, used to force a repaint.
    pub(crate) client_tty: Option<String>,
    /// A share viewer mirrors the pane instead of resizing it: its own window
    /// must not reshape the terminal the owner is working in.
    pub(crate) mirror_size: bool,
    /// Last size a mirroring viewer was moved to.
    pub(crate) last_size: Option<(u16, u16)>,
}

/// After a mirrored client changes size, tmux may leave the old frame on
/// screen — padded out with its "client too big" filler. Asking for a refresh
/// of that one client paints the real thing.
fn refresh_tmux_client(tty: &str) {
    let _ = std::process::Command::new("tmux").args(["refresh-client", "-t", tty]).output();
}

/// Tells a mirroring client how big the pane is, so it can match without
/// sending a resize back.
pub(crate) const SIZE_MSG: &str = "\x02SIZE:";

/// Sent to the browser when the session is gone for good (the shell exited, or
/// somebody killed it) as opposed to a network drop, which the browser retries.
pub(crate) const ENDED_MSG: &str = "\x02ENDED";

/// A WebSocket that explains why it cannot serve this pane and closes — used
/// when a browser asks for a session that is gone or a target that no longer
/// resolves. The human-readable line lands on the screen; ENDED stops the
/// browser from retrying.
pub(crate) struct NoticeSocket(String);

impl NoticeSocket {
    pub(crate) fn new(reason: &str) -> Self {
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
    /// Resize the pseudo-terminal and let the process inside know.
    fn set_winsize(&self, (cols, rows): (u16, u16)) {
        let ws = nix::pty::Winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
        unsafe { libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &ws) };
        let _ = nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGWINCH);
        if let Some(tty) = self.client_tty.as_deref() {
            refresh_tmux_client(tty);
        }
    }

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
        if self.mirror_size {
            ctx.run_interval(SIZE_POLL_INTERVAL, |act, ctx| {
                let size = match client_size(&act.tmux_name) {
                    Some(s) => s,
                    None => return,
                };
                if act.last_size == Some(size) { return; }
                act.last_size = Some(size);
                // Follow the owner on this end too. Telling only the browser
                // leaves tmux drawing for a client of the old size, which is
                // what made a shared view go stale the moment anyone zoomed.
                act.set_winsize(size);
                ctx.text(format!("{}{}:{}", SIZE_MSG, size.0, size.1));
            });
        }
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
                if self.read_only { return; }
                if text.starts_with("\x01RESIZE:") {
                    // A viewer's window size is its own business.
                    if self.mirror_size { return; }
                    let parts: Vec<&str> = text[8..].split(':').collect();
                    if parts.len() == 2 {
                        if let (Ok(cols), Ok(rows)) = (parts[0].parse::<u16>(), parts[1].parse::<u16>()) {
                            self.set_winsize((cols, rows));
                        }
                    }
                    return;
                }
                if let Some(f) = self.master_file.as_mut() { let _ = f.write_all(text.as_bytes()); }
            }
            Ok(ws::Message::Binary(data)) => {
                if self.read_only { return; }
                if let Some(f) = self.master_file.as_mut() { let _ = f.write_all(&data); }
            }
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Close(_)) => ctx.stop(),
            _ => {}
        }
    }
}

/// Spawn a PTY running `tmux attach -t <name>` and return (master_fd, child_pid)
pub(crate) fn attach_tmux(name: &str, size: (u16, u16)) -> Result<(i32, nix::unistd::Pid), String> {
    let cmd = ShellCommand { program: "tmux".into(), args: vec!["attach-session".into(), "-t".into(), name.into()] };
    spawn_pty(cmd, size)
}

/// The same attach, with tmux refusing input from this client.
pub(crate) fn attach_tmux_readonly(name: &str, size: (u16, u16)) -> Result<(i32, nix::unistd::Pid), String> {
    let cmd = ShellCommand {
        program: "tmux".into(),
        args: vec!["attach-session".into(), "-r".into(), "-t".into(), name.into()],
    };
    spawn_pty(cmd, size)
}

/// Terminal size requested by the browser, clamped to something tmux accepts.
pub(crate) fn parse_size(query: &str) -> (u16, u16) {
    let cols = parse_query_param(query, "cols").and_then(|v| v.parse::<u16>().ok()).unwrap_or(120);
    let rows = parse_query_param(query, "rows").and_then(|v| v.parse::<u16>().ok()).unwrap_or(40);
    (cols.clamp(20, 500), rows.clamp(5, 200))
}
