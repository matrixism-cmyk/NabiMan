use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web::{get, web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::{Read, Write};
use std::time::Duration;
use crate::auth::TokenStore;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const PTY_READ_INTERVAL: Duration = Duration::from_millis(30);

pub struct TerminalSession {
    master_fd: i32,
    master_file: std::fs::File,
    child_pid: nix::unistd::Pid,
}

struct ShellCommand {
    program: String,
    args: Vec<String>,
}

impl ShellCommand {
    fn local_shell() -> Self {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
        Self { program: shell, args: vec!["--login".into()] }
    }

    fn ssh(user: &str, host: &str, port: &str) -> Self {
        Self {
            program: "ssh".into(),
            args: vec![
                "-o".into(), "StrictHostKeyChecking=accept-new".into(),
                "-o".into(), "ServerAliveInterval=30".into(),
                "-p".into(), port.into(),
                format!("{}@{}", user, host),
            ],
        }
    }
}

impl TerminalSession {
    fn new(cmd: ShellCommand) -> Result<Self, String> {
        let pty = nix::pty::openpty(None, None)
            .map_err(|e| format!("openpty failed: {}", e))?;

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
                if slave_fd > 2 {
                    let _ = nix::unistd::close(slave_fd);
                }

                std::env::set_var("TERM", "xterm-256color");
                std::env::set_var("COLORTERM", "truecolor");
                std::env::set_var("LANG", "en_US.UTF-8");

                let _ = std::process::Command::new(&cmd.program)
                    .args(&cmd.args)
                    .stdin(unsafe { std::process::Stdio::from_raw_fd(0) })
                    .stdout(unsafe { std::process::Stdio::from_raw_fd(1) })
                    .stderr(unsafe { std::process::Stdio::from_raw_fd(2) })
                    .status();

                std::process::exit(0);
            }
            Ok(nix::unistd::ForkResult::Parent { child }) => {
                drop(pty.slave);

                let flags = nix::fcntl::fcntl(master_fd, nix::fcntl::FcntlArg::F_GETFL)
                    .map_err(|e| format!("fcntl get: {}", e))?;
                let mut oflags = nix::fcntl::OFlag::from_bits_truncate(flags);
                oflags.insert(nix::fcntl::OFlag::O_NONBLOCK);
                nix::fcntl::fcntl(master_fd, nix::fcntl::FcntlArg::F_SETFL(oflags))
                    .map_err(|e| format!("fcntl set: {}", e))?;

                let master_file = unsafe { std::fs::File::from_raw_fd(master_fd) };
                std::mem::forget(pty.master);

                Ok(Self { master_fd, master_file, child_pid: child })
            }
            Err(e) => Err(format!("fork failed: {}", e)),
        }
    }
}

impl Actor for TerminalSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Read PTY output → WebSocket
        ctx.run_interval(PTY_READ_INTERVAL, |act, ctx| {
            let mut buf = [0u8; 8192];
            loop {
                match act.master_file.read(&mut buf) {
                    Ok(0) => { ctx.stop(); break; }
                    Ok(n) => { ctx.binary(buf[..n].to_vec()); }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => { ctx.stop(); break; }
                }
            }
        });

        ctx.run_interval(HEARTBEAT_INTERVAL, |_act, ctx| {
            ctx.ping(b"");
        });
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        let _ = nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGHUP);
        let _ = nix::sys::wait::waitpid(self.child_pid, None);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for TerminalSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                // Resize: \x01RESIZE:cols:rows
                if text.starts_with("\x01RESIZE:") {
                    let parts: Vec<&str> = text[8..].split(':').collect();
                    if parts.len() == 2 {
                        if let (Ok(cols), Ok(rows)) = (
                            parts[0].parse::<u16>(),
                            parts[1].parse::<u16>(),
                        ) {
                            let winsize = nix::pty::Winsize {
                                ws_row: rows,
                                ws_col: cols,
                                ws_xpixel: 0,
                                ws_ypixel: 0,
                            };
                            unsafe { libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &winsize) };
                            // Notify the shell of resize
                            let _ = nix::sys::signal::kill(
                                self.child_pid,
                                nix::sys::signal::Signal::SIGWINCH,
                            );
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

fn parse_query_param<'a>(query: &'a str, key: &str) -> Option<String> {
    query.split('&').find_map(|p| {
        let mut kv = p.splitn(2, '=');
        if kv.next() == Some(key) {
            kv.next().map(|v| urlencoding_decode(v))
        } else {
            None
        }
    })
}

fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h = chars.next().unwrap_or(b'0');
            let l = chars.next().unwrap_or(b'0');
            let val = hex_val(h) * 16 + hex_val(l);
            result.push(val as char);
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

fn hex_val(b: u8) -> u8 {
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
        && s.chars().all(|c| c.is_alphanumeric() || ".-_@:".contains(c))
}

#[get("/api/terminal")]
async fn ws_terminal(
    req: HttpRequest,
    stream: web::Payload,
    store: web::Data<TokenStore>,
) -> Result<HttpResponse, actix_web::Error> {
    let query = req.query_string();

    // Auth via token query param
    let authenticated = parse_query_param(query, "token")
        .map(|token| store.lock().unwrap().contains(&token))
        .unwrap_or(false);

    if !authenticated {
        return Ok(HttpResponse::Unauthorized().json(
            crate::models::ApiResponse::<()>::error("Unauthorized"),
        ));
    }

    // Determine mode: local shell or SSH
    let cmd = if let Some(host) = parse_query_param(query, "ssh_host") {
        let port = parse_query_param(query, "ssh_port").unwrap_or_else(|| "22".into());
        let user = parse_query_param(query, "ssh_user").unwrap_or_else(|| "root".into());

        if !validate_ssh_input(&host) || !validate_ssh_input(&port) || !validate_ssh_input(&user) {
            return Ok(HttpResponse::BadRequest().json(
                crate::models::ApiResponse::<()>::error("Invalid SSH parameters"),
            ));
        }

        ShellCommand::ssh(&user, &host, &port)
    } else {
        ShellCommand::local_shell()
    };

    match TerminalSession::new(cmd) {
        Ok(session) => ws::start(session, &req, stream),
        Err(e) => Ok(HttpResponse::InternalServerError().json(
            crate::models::ApiResponse::<()>::error(&e),
        )),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_terminal);
}
