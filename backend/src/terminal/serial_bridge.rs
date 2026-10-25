//! A serial device bridged from the operator's browser. The browser opens a
//! Web Serial port (e.g. an ESP32 on USB) and relays its bytes over this
//! WebSocket; NabiMan hands them to a server-side PTY via `socat`, so tools run
//! on the server — arduino-cli, esptool, a plain monitor — reach a board on the
//! operator's own machine as if it were local. Admin only. (Phase 1: raw bytes;
//! the browser sets baud and toggles DTR/RTS itself.)

use std::process::Stdio;
use std::time::{Duration, Instant};

use actix::{Actor, ActorContext, AsyncContext, Handler, Message, StreamHandler};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::mpsc;

const LONGEST: Duration = Duration::from_secs(12 * 60 * 60);
const CHECK_EVERY: Duration = Duration::from_secs(30);

/// The directory holding the per-device PTY links that server tools open.
fn serial_dir() -> std::path::PathBuf {
    let root = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/workspace/var/lib/nabiman".into());
    std::path::Path::new(&root).join("serial")
}

struct SerialBridge {
    link: String,
    started: Instant,
    to_socat: Option<mpsc::UnboundedSender<Vec<u8>>>,
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
        ctx.run_interval(CHECK_EVERY, |act, ctx| {
            if act.started.elapsed() > LONGEST {
                ctx.close(None);
                ctx.stop();
            }
        });
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        self.to_socat = None;
        let _ = std::fs::remove_file(&self.link);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for SerialBridge {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Binary(b)) => {
                if let Some(tx) = &self.to_socat {
                    let _ = tx.send(b.to_vec());
                }
            }
            Ok(ws::Message::Ping(p)) => ctx.pong(&p),
            Ok(ws::Message::Close(_)) | Err(_) => {
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
        Err(_) => {
            addr.do_send(FromSocat::Closed);
            return;
        }
    };
    let (Some(mut stdin), Some(mut stdout)) = (child.stdin.take(), child.stdout.take()) else {
        addr.do_send(FromSocat::Closed);
        return;
    };
    let mut buf = vec![0u8; 8192];
    loop {
        tokio::select! {
            m = rx.recv() => match m {
                Some(b) => if stdin.write_all(&b).await.is_err() { break },
                None => break,
            },
            r = stdout.read(&mut buf) => match r {
                Ok(0) | Err(_) => break,
                Ok(n) => addr.do_send(FromSocat::Data(buf[..n].to_vec())),
            },
        }
    }
    let _ = child.kill().await;
    addr.do_send(FromSocat::Closed);
}

/// `GET /api/remote/serial?serial=<name>&ticket=…` — the serial bridge socket.
#[actix_web::get("/api/remote/serial")]
async fn ws_serial(
    req: HttpRequest,
    stream: web::Payload,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    users: web::Data<crate::users::UserStore>,
) -> Result<HttpResponse, actix_web::Error> {
    ws::handshake(&req)?;
    let Some(grant) = crate::stream_access::ConnectionGrant::from_request(
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
    let safe: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(40).collect();
    if safe.is_empty() {
        return Ok(HttpResponse::BadRequest().finish());
    }
    let link = serial_dir().join(&safe).to_string_lossy().into_owned();
    ws::WsResponseBuilder::new(
        SerialBridge { link, started: Instant::now(), to_socat: None },
        &req,
        stream,
    )
    .start()
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_serial);
}
