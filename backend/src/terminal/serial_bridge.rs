//! Serial devices bridged from a browser. The browser opens a Web Serial port
//! (e.g. an ESP32 on USB) and relays its bytes over a WebSocket; NabiMan hands
//! them to a server-side PTY via `socat`, so tools on the server — arduino-cli,
//! esptool, a plain monitor — reach a board on the operator's own machine as if
//! it were local.
//!
//! A device is attached to one terminal session (`/api/terminal/serial`, see
//! `serial_sessions`); the older `/api/remote/serial` keeps one global,
//! admin-only namespace.
//!
//! A PTY carries no modem lines, so DTR/RTS cannot pass through it: instead a
//! server tool writes `reset` or `bootloader` to `<link>.cmd`, the bridge
//! forwards it, and the browser drives the lines and answers; the answer lands
//! in `<link>.ack`.

use std::process::Stdio;
use std::time::{Duration, Instant};

use actix::{Actor, ActorContext, AsyncContext, Handler, Message, StreamHandler};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::mpsc;

use super::serial_sessions::{release, session_admits, SessionScope};
use crate::stream_access::ConnectionGrant;

const LONGEST: Duration = Duration::from_secs(12 * 60 * 60);
const CHECK_EVERY: Duration = Duration::from_secs(30);
/// How soon a session-attached device notices that its session or access is gone.
const SESSION_CHECK_EVERY: Duration = Duration::from_secs(10);
const COMMAND_EVERY: Duration = Duration::from_millis(200);
/// Line sequences the browser knows how to drive.
const SIGNALS: [&str; 2] = ["reset", "bootloader"];
/// A browser that has silently vanished (network drop) is pinged; no answer in this long ends it,
/// so its device name frees up for the reconnect instead of being held by a dead socket.
const PING_EVERY: Duration = Duration::from_secs(10);
const SILENT_LIMIT: Duration = Duration::from_secs(35);

/// The directory holding the per-device PTY links that server tools open.
pub(super) fn serial_dir() -> std::path::PathBuf {
    std::path::Path::new(&super::session::data_dir()).join("serial")
}

/// `[A-Za-z0-9_-]`, at most `max` characters; anything else is dropped.
pub(super) fn safe_name(raw: &str, max: usize) -> String {
    raw.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(max).collect()
}

pub(super) struct SerialBridge {
    pub(super) link: String,
    pub(super) started: Instant,
    pub(super) to_socat: Option<mpsc::UnboundedSender<Vec<u8>>>,
    pub(super) session: Option<SessionScope>,
    /// When the browser was last heard from (any frame, including a pong).
    pub(super) heard: Instant,
}

impl SerialBridge {
    pub(super) fn new(link: String, session: Option<SessionScope>) -> Self {
        let now = Instant::now();
        SerialBridge { link, started: now, to_socat: None, session, heard: now }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
enum FromSocat {
    Data(Vec<u8>),
    Closed,
}

impl Actor for SerialBridge {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.to_socat = Some(tx);
        let addr = ctx.address();
        let link = self.link.clone();
        actix::spawn(async move { pump(link, rx, addr).await });
        // The browser shows this so the operator knows what the server tools open.
        ctx.text(serde_json::json!({ "path": self.link }).to_string());
        ctx.run_interval(PING_EVERY, |act, ctx| {
            if act.heard.elapsed() > SILENT_LIMIT {
                eprintln!("[serial] {}: browser silent for {}s, closing", act.link, act.heard.elapsed().as_secs());
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
        ctx.run_interval(CHECK_EVERY, |act, ctx| {
            if act.started.elapsed() > LONGEST {
                ctx.close(None);
                ctx.stop();
            }
        });
        if self.session.is_some() {
            ctx.run_interval(SESSION_CHECK_EVERY, |act, ctx| {
                let Some(scope) = &act.session else { return };
                if !scope.grant.valid() || !session_admits(&scope.store, &scope.sid, &scope.grant) {
                    eprintln!("[serial] {}: session {} or its access is gone", act.link, scope.sid);
                    ctx.close(Some(ws::CloseReason {
                        code: ws::CloseCode::Policy,
                        description: Some("session closed or access revoked".into()),
                    }));
                    ctx.stop();
                }
            });
        }
        // A server tool asks for a reset / bootloader entry by writing `<link>.cmd`.
        ctx.run_interval(COMMAND_EVERY, |act, ctx| {
            let path = format!("{}.cmd", act.link);
            let Ok(cmd) = std::fs::read_to_string(&path) else { return };
            let _ = std::fs::remove_file(&path);
            let cmd = cmd.trim();
            if SIGNALS.contains(&cmd) {
                let _ = std::fs::remove_file(format!("{}.ack", act.link));
                ctx.text(serde_json::json!({ "signal": cmd }).to_string());
            }
        });
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        self.to_socat = None;
        eprintln!("[serial] {}: bridge closed after {}s", self.link, self.started.elapsed().as_secs());
        for path in [self.link.clone(), format!("{}.cmd", self.link), format!("{}.ack", self.link)] {
            let _ = std::fs::remove_file(path);
        }
        if let Some(scope) = &self.session {
            release(&scope.sid, &scope.name);
            // The session's directory goes once its last device does.
            if let Some(dir) = std::path::Path::new(&self.link).parent() {
                let _ = std::fs::remove_dir(dir);
            }
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for SerialBridge {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        self.heard = Instant::now();
        match msg {
            Ok(ws::Message::Binary(b)) => {
                if let Some(tx) = &self.to_socat {
                    let _ = tx.send(b.to_vec());
                }
            }
            // The browser's answer to a forwarded signal: {"ack":"bootloader"} or {"ack":"bootloader","error":"…"}.
            Ok(ws::Message::Text(t)) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                    if let Some(ack) = v.get("ack").and_then(|a| a.as_str()).filter(|a| SIGNALS.contains(a)) {
                        let line = match v.get("error").and_then(|e| e.as_str()) {
                            Some(e) => format!("{ack} error {}\n", e.chars().take(200).collect::<String>()),
                            None => format!("{ack} ok\n"),
                        };
                        let _ = std::fs::write(format!("{}.ack", self.link), line);
                    }
                }
            }
            Ok(ws::Message::Ping(p)) => ctx.pong(&p),
            Ok(ws::Message::Close(reason)) => {
                eprintln!("[serial] {}: browser closed the socket ({:?})", self.link, reason);
                ctx.close(None);
                ctx.stop();
            }
            Err(e) => {
                eprintln!("[serial] {}: socket error: {e}", self.link);
                ctx.close(None);
                ctx.stop();
            }
            _ => {}
        }
    }
}

impl Handler<FromSocat> for SerialBridge {
    type Result = ();
    fn handle(&mut self, msg: FromSocat, ctx: &mut Self::Context) {
        match msg {
            FromSocat::Data(b) => ctx.binary(b),
            FromSocat::Closed => {
                ctx.close(None);
                ctx.stop();
            }
        }
    }
}

/// Runs `socat` to make a PTY at `link`, then relays bytes both ways: the server
/// tool opens the PTY, the browser holds the real port.
async fn pump(link: String, mut rx: mpsc::UnboundedReceiver<Vec<u8>>, addr: actix::Addr<SerialBridge>) {
    if let Some(dir) = std::path::Path::new(&link).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::remove_file(&link);
    let mut child = match Command::new("socat")
        .arg(format!("PTY,link={link},raw,echo=0"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[serial] {link}: cannot start socat: {e}");
            addr.do_send(FromSocat::Closed);
            return;
        }
    };
    let (Some(mut stdin), Some(mut stdout)) = (child.stdin.take(), child.stdout.take()) else {
        addr.do_send(FromSocat::Closed);
        return;
    };
    let mut buf = vec![0u8; 8192];
    let why = loop {
        tokio::select! {
            m = rx.recv() => match m {
                Some(b) => if let Err(e) = stdin.write_all(&b).await { break format!("write to socat: {e}") },
                None => break "socket gone".to_string(),
            },
            r = stdout.read(&mut buf) => match r {
                Ok(0) => break "socat closed its output".to_string(),
                Err(e) => break format!("read from socat: {e}"),
                Ok(n) => addr.do_send(FromSocat::Data(buf[..n].to_vec())),
            },
        }
    };
    if why != "socket gone" {
        eprintln!("[serial] {link}: relay ended: {why}");
    }
    let _ = child.kill().await;
    addr.do_send(FromSocat::Closed);
}

/// `GET /api/remote/serial?serial=<name>&ticket=…` — the global, admin-only bridge.
#[actix_web::get("/api/remote/serial")]
async fn ws_serial(
    req: HttpRequest,
    stream: web::Payload,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    users: web::Data<crate::users::UserStore>,
) -> Result<HttpResponse, actix_web::Error> {
    ws::handshake(&req)?;
    let Some(grant) = ConnectionGrant::from_request(
        &req,
        sessions.get_ref().clone(),
        users.get_ref().clone(),
    )
    .filter(|g| g.valid()) else {
        return Ok(HttpResponse::Unauthorized().finish());
    };
    let _ = grant;
    if !super::vnc_console::is_admin(&req) {
        return Ok(HttpResponse::Forbidden().finish());
    }
    let name = super::parse_query_param(req.query_string(), "serial").unwrap_or_default();
    let safe = safe_name(&name, 40);
    if safe.is_empty() {
        return Ok(HttpResponse::BadRequest().finish());
    }
    let link = serial_dir().join(&safe).to_string_lossy().into_owned();
    ws::WsResponseBuilder::new(SerialBridge::new(link, None), &req, stream).start()
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_serial);
    super::serial_sessions::config(cfg);
}
