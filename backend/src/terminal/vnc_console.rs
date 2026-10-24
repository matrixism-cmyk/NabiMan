//! Browser VNC consoles for the internal KubeVirt VMs, inside NabiMan.
//!
//! The VMs already have a local websockify bridge each (127.0.0.1:<port>,
//! `vnc-bridge.py`). NabiMan reaches those bridges **locally**, so the public
//! hairpin never applies, and relays the VNC WebSocket to the browser's own
//! noVNC under NabiMan's auth — not the console's separate login. Admins and
//! operators only; the VMs are powerful internal infrastructure.

use std::time::{Duration, Instant};

use actix::{Actor, ActorContext, AsyncContext, Handler, Message, StreamHandler};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::models::{ApiResponse, UserRole};

/// name, label, local websockify port. Mirrors the nginx console locations;
/// only these fixed targets are reachable — never an arbitrary port.
const CONSOLES: &[(&str, &str, u16)] = &[
    ("winsrv2022", "Windows Server 2022", 6081),
    ("win11", "Windows 11", 6080),
    ("win11-2", "Windows 11 (2)", 6082),
    ("k8s-rocky8", "Rocky Linux 8", 6083),
];

const BIGGEST_FRAME: usize = 1 << 20; // One RFB frame; 1 MiB is ample.
const LONGEST: Duration = Duration::from_secs(12 * 60 * 60);
const CHECK_EVERY: Duration = Duration::from_secs(30);

fn port_for(vm: &str) -> Option<u16> {
    CONSOLES.iter().find(|(n, _, _)| *n == vm).map(|(_, _, p)| *p)
}

/// Internal VM consoles are powerful: admins and operators only.
fn may_use_console(req: &HttpRequest) -> bool {
    matches!(
        crate::auth::verified_role(req),
        Some(UserRole::Admin) | Some(UserRole::Operator)
    )
}

struct VncConsole {
    port: u16,
    started: Instant,
    to_upstream: Option<mpsc::UnboundedSender<WsMessage>>,
    grant: crate::stream_access::ConnectionGrant,
}

#[derive(Message)]
#[rtype(result = "()")]
enum FromVm {
    Binary(Vec<u8>),
    Closed,
}

impl VncConsole {
    fn send(&self, m: WsMessage) {
        if let Some(tx) = &self.to_upstream {
            let _ = tx.send(m);
        }
    }
}

impl Actor for VncConsole {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.to_upstream = Some(tx);
        let addr = ctx.address();
        let port = self.port;
        actix::spawn(async move { connect_and_pump(port, rx, addr).await });
        // The account can be signed out or lose access while connected.
        ctx.run_interval(CHECK_EVERY, |act, ctx| {
            if !act.grant.valid() || act.started.elapsed() > LONGEST {
                ctx.close(None);
                ctx.stop();
            }
        });
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        // Dropping the sender closes the upstream side too.
        self.to_upstream = None;
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for VncConsole {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Binary(bytes)) => {
                if bytes.len() <= BIGGEST_FRAME {
                    self.send(WsMessage::binary(bytes));
                }
            }
            // noVNC speaks binary; a text frame is harmless to pass through.
            Ok(ws::Message::Text(text)) => self.send(WsMessage::text(text.to_string())),
            Ok(ws::Message::Ping(p)) => ctx.pong(&p),
            Ok(ws::Message::Close(_)) | Err(_) => {
                self.send(WsMessage::Close(None));
                ctx.close(None);
                ctx.stop();
            }
            _ => {}
        }
    }
}

impl Handler<FromVm> for VncConsole {
    type Result = ();
    fn handle(&mut self, msg: FromVm, ctx: &mut Self::Context) {
        match msg {
            FromVm::Binary(b) => ctx.binary(b),
            FromVm::Closed => {
                ctx.close(None);
                ctx.stop();
            }
        }
    }
}

/// Relays the VNC WebSocket between the local websockify bridge and the actor
/// until either side closes.
async fn connect_and_pump(
    port: u16,
    mut rx: mpsc::UnboundedReceiver<WsMessage>,
    addr: actix::Addr<VncConsole>,
) {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::http::HeaderValue;
    let url = format!("ws://127.0.0.1:{port}/websockify");
    let Ok(mut request) = url.into_client_request() else {
        addr.do_send(FromVm::Closed);
        return;
    };
    // noVNC negotiates the "binary" subprotocol; offer the same upstream.
    request
        .headers_mut()
        .insert("Sec-WebSocket-Protocol", HeaderValue::from_static("binary"));
    // The bridge is on loopback; open the socket and do the WS handshake over it
    // (this build of tokio-tungstenite has handshake only, no connector).
    let tcp = match tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
        Ok(tcp) => tcp,
        Err(_) => {
            addr.do_send(FromVm::Closed);
            return;
        }
    };
    let ws = match tokio_tungstenite::client_async(request, tcp).await {
        Ok((ws, _)) => ws,
        Err(_) => {
            addr.do_send(FromVm::Closed);
            return;
        }
    };
    let (mut sink, mut stream) = ws.split();
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(message) => if sink.send(message).await.is_err() { break },
                None => { let _ = sink.send(WsMessage::Close(None)).await; break }
            },
            incoming = stream.next() => match incoming {
                Some(Ok(WsMessage::Binary(b))) => addr.do_send(FromVm::Binary(b.to_vec())),
                Some(Ok(WsMessage::Ping(_))) | Some(Ok(WsMessage::Pong(_))) => {}
                Some(Ok(WsMessage::Close(_))) | None => break,
                Some(Err(_)) => break,
                _ => {}
            },
        }
    }
    addr.do_send(FromVm::Closed);
}

/// `GET /api/remote/console?vm=<name>&ticket=…` — the VNC WebSocket, admitted
/// by the same one-use ticket a terminal uses.
#[actix_web::get("/api/remote/console")]
async fn ws_console(
    req: HttpRequest,
    stream: web::Payload,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    users: web::Data<crate::users::UserStore>,
) -> Result<HttpResponse, actix_web::Error> {
    ws::handshake(&req)?;
    if !may_use_console(&req) {
        return Ok(HttpResponse::Forbidden().finish());
    }
    let Some(grant) = crate::stream_access::ConnectionGrant::from_request(
        &req,
        sessions.get_ref().clone(),
        users.get_ref().clone(),
    )
    .filter(|g| g.valid()) else {
        return Ok(HttpResponse::Unauthorized().finish());
    };
    let Some(vm) = super::parse_query_param(req.query_string(), "vm") else {
        return Ok(HttpResponse::BadRequest().finish());
    };
    let Some(port) = port_for(&vm) else {
        return Ok(HttpResponse::NotFound().finish());
    };
    // Echo the "binary" subprotocol so noVNC's client is satisfied.
    ws::WsResponseBuilder::new(
        VncConsole {
            port,
            started: Instant::now(),
            to_upstream: None,
            grant,
        },
        &req,
        stream,
    )
    .protocols(&["binary"])
    .start()
}

/// `GET /api/remote/consoles` — the VM consoles the caller may open.
#[actix_web::get("/api/remote/consoles")]
async fn list_consoles(req: HttpRequest) -> HttpResponse {
    if !may_use_console(&req) {
        return HttpResponse::Forbidden().finish();
    }
    let list: Vec<_> = CONSOLES
        .iter()
        .map(|(name, label, _)| serde_json::json!({ "name": name, "label": label }))
        .collect();
    HttpResponse::Ok().json(ApiResponse::ok(list))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_console).service(list_consoles);
}
