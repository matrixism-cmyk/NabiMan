use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web::{get, web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::{Read, Write};
use std::process::Command;
use std::time::Duration;
use crate::auth::TokenStore;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const PTY_READ_INTERVAL: Duration = Duration::from_millis(50);

/// WebSocket actor that bridges browser <-> PTY
pub struct TerminalSession {
    master_fd: i32,
    master_file: std::fs::File,
    child_pid: nix::unistd::Pid,
}

impl TerminalSession {
    fn new() -> Result<Self, String> {
        // Open a PTY pair
        let pty = nix::pty::openpty(None, None)
            .map_err(|e| format!("openpty failed: {}", e))?;

        let master_fd = pty.master.as_raw_fd();
        let slave_fd = pty.slave.as_raw_fd();

        // Fork a child process
        match unsafe { nix::unistd::fork() } {
            Ok(nix::unistd::ForkResult::Child) => {
                // Child: set up new session, attach to slave PTY, exec bash
                drop(pty.master);
                let _ = nix::unistd::setsid();

                // Set controlling terminal
                unsafe { libc::ioctl(slave_fd, libc::TIOCSCTTY, 0) };

                // Redirect stdin/stdout/stderr to slave
                let _ = nix::unistd::dup2(slave_fd, 0);
                let _ = nix::unistd::dup2(slave_fd, 1);
                let _ = nix::unistd::dup2(slave_fd, 2);

                if slave_fd > 2 {
                    let _ = nix::unistd::close(slave_fd);
                }

                // Set TERM env
                std::env::set_var("TERM", "xterm-256color");

                // Exec shell
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
                let _ = Command::new(&shell)
                    .arg("--login")
                    .stdin(unsafe { std::process::Stdio::from_raw_fd(0) })
                    .stdout(unsafe { std::process::Stdio::from_raw_fd(1) })
                    .stderr(unsafe { std::process::Stdio::from_raw_fd(2) })
                    .status();

                std::process::exit(0);
            }
            Ok(nix::unistd::ForkResult::Parent { child }) => {
                // Parent: close slave, use master
                drop(pty.slave);

                // Set master to non-blocking
                let flags = nix::fcntl::fcntl(master_fd, nix::fcntl::FcntlArg::F_GETFL)
                    .map_err(|e| format!("fcntl get: {}", e))?;
                let mut oflags = nix::fcntl::OFlag::from_bits_truncate(flags);
                oflags.insert(nix::fcntl::OFlag::O_NONBLOCK);
                nix::fcntl::fcntl(master_fd, nix::fcntl::FcntlArg::F_SETFL(oflags))
                    .map_err(|e| format!("fcntl set: {}", e))?;

                let master_file = unsafe { std::fs::File::from_raw_fd(master_fd) };
                // Forget pty.master so it doesn't close master_fd
                std::mem::forget(pty.master);

                Ok(Self {
                    master_fd,
                    master_file,
                    child_pid: child,
                })
            }
            Err(e) => Err(format!("fork failed: {}", e)),
        }
    }
}

impl Actor for TerminalSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Periodically read PTY output and send to WebSocket
        ctx.run_interval(PTY_READ_INTERVAL, |act, ctx| {
            let mut buf = [0u8; 4096];
            loop {
                match act.master_file.read(&mut buf) {
                    Ok(0) => {
                        ctx.stop();
                        break;
                    }
                    Ok(n) => {
                        ctx.binary(buf[..n].to_vec());
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        ctx.stop();
                        break;
                    }
                }
            }
        });

        // Heartbeat
        ctx.run_interval(HEARTBEAT_INTERVAL, |_act, ctx| {
            ctx.ping(b"");
        });
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        // Kill the child process
        let _ = nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGHUP);
        let _ = nix::sys::wait::waitpid(self.child_pid, None);
    }
}

/// Handle incoming WebSocket messages
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for TerminalSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                // Check for resize command: \x01RESIZE:cols:rows
                if text.starts_with("\x01RESIZE:") {
                    let parts: Vec<&str> = text[8..].split(':').collect();
                    if parts.len() == 2 {
                        if let (Ok(cols), Ok(rows)) = (
                            parts[0].parse::<u16>(),
                            parts[1].parse::<u16>(),
                        ) {
                            let ws = nix::pty::Winsize {
                                ws_row: rows,
                                ws_col: cols,
                                ws_xpixel: 0,
                                ws_ypixel: 0,
                            };
                            unsafe {
                                libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &ws);
                            }
                        }
                    }
                    return;
                }
                let _ = self.master_file.write_all(text.as_bytes());
            }
            Ok(ws::Message::Binary(data)) => {
                let _ = self.master_file.write_all(&data);
            }
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Close(_)) => ctx.stop(),
            _ => {}
        }
    }
}

#[get("/api/terminal")]
async fn ws_terminal(
    req: HttpRequest,
    stream: web::Payload,
    store: web::Data<TokenStore>,
) -> Result<HttpResponse, actix_web::Error> {
    // Auth check via query param: ?token=xxx
    let authenticated = req
        .query_string()
        .split('&')
        .find_map(|p| {
            let mut kv = p.splitn(2, '=');
            if kv.next() == Some("token") {
                kv.next().map(String::from)
            } else {
                None
            }
        })
        .map(|token| store.lock().unwrap().contains(&token))
        .unwrap_or(false);

    if !authenticated {
        return Ok(HttpResponse::Unauthorized().json(
            crate::models::ApiResponse::<()>::error("Unauthorized"),
        ));
    }

    match TerminalSession::new() {
        Ok(session) => ws::start(session, &req, stream),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            crate::models::ApiResponse::<()>::error(&e),
        )),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_terminal);
}
