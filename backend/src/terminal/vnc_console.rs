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

use crate::models::UserRole;

/// name, label, local websockify port. Mirrors the nginx console locations;
/// only these fixed targets are reachable — never an arbitrary port.
pub(super) const CONSOLES: &[(&str, &str, u16)] = &[
    ("winsrv2022", "Windows Server 2022", 6081),
    ("win11", "Windows 11", 6080),
    ("win11-2", "Windows 11 (2)", 6082),
    ("k8s-rocky8", "Rocky Linux 8", 6083),
];

const BIGGEST_FRAME: usize = 1 << 20; // One RFB frame; 1 MiB is ample.
const LONGEST: Duration = Duration::from_secs(12 * 60 * 60);
const CHECK_EVERY: Duration = Duration::from_secs(30);

pub(super) fn port_for(vm: &str) -> Option<u16> {
    CONSOLES.iter().find(|(n, _, _)| *n == vm).map(|(_, _, p)| *p)
}

/// Where a console's VNC lives: an internal bridge on loopback (reached as a
/// WebSocket) or an admin-registered external host (reached over raw TCP).
#[derive(Clone)]
enum Target {
    LocalWs(u16),
    RawTcp(String, u16),
}

/// Resolve a console key to its target, only to a known console — never to an
/// address taken from the request, so no arbitrary host or port is reachable.
fn resolve(vm: &str) -> Option<Target> {
    // A VNC console with a host — a registered one, or an overlay on a built-in
    // — reaches that external address over raw TCP. (RDP overlays are opened via
    // the RDP path, not here.) A built-in with no such overlay uses its local
    // bridge.
    if let Some(c) = super::vnc_registry::find(vm) {
        if c.protocol != "rdp" && !c.host.trim().is_empty() {
            return Some(Target::RawTcp(c.host, c.port));
        }
    }
    port_for(vm).map(Target::LocalWs)
}

/// Whether a console key names a real console (built-in bridge or registered).
pub(super) fn console_exists(key: &str) -> bool {
    port_for(key).is_some() || super::vnc_registry::find(key).is_some()
}

/// A console's display label and protocol ("vnc"|"rdp"), for share pages.
pub(super) fn console_label_protocol(key: &str) -> Option<(String, String)> {
    if let Some((_, label, _)) = CONSOLES.iter().find(|(n, _, _)| *n == key) {
        return Some(((*label).to_string(), "vnc".into()));
    }
    super::vnc_registry::find(key).map(|c| (c.label, c.protocol))
}

/// Opens a console for a share link (no account): VNC here, RDP via guacd.
pub(super) fn open_shared_console(
    req: &HttpRequest,
    stream: actix_web::web::Payload,
    console: &str,
    view_only: bool,
    token: String,
) -> Result<HttpResponse, actix_web::Error> {
    let protocol = console_label_protocol(console).map(|(_, p)| p).unwrap_or_else(|| "vnc".into());
    if protocol == "rdp" {
        return super::rdp_tunnel::open_shared_rdp(req, stream, console, view_only, token);
    }
    let Some(target) = resolve(console) else {
        return Ok(HttpResponse::NotFound().finish());
    };
    ws::WsResponseBuilder::new(
        VncConsole {
            target,
            started: Instant::now(),
            to_upstream: None,
            life: super::console_share::Lifeline::Share(token),
            view_only,
        },
        req,
        stream,
    )
    .protocols(&["binary"])
    .start()
}

pub(super) fn is_admin(req: &HttpRequest) -> bool {
    matches!(crate::auth::verified_role(req), Some(UserRole::Admin))
}

/// VM consoles are sensitive internal infrastructure. An administrator may open
/// any; anyone else only a console delegated to their account (vnc_access).
pub(super) fn may_open(req: &HttpRequest, console: &str) -> bool {
    is_admin(req)
        || crate::auth::verified_user_id(req).is_some_and(|uid| {
            super::vnc_access::allowed(&uid, console)
                || super::vnc_registry::owner_of(console).as_deref() == Some(uid.as_str())
        })
}

/// Who may register a new console: an admin or a remote-only member (for their
/// own). Operators and viewers do not.
pub(super) fn may_create_console(req: &HttpRequest) -> bool {
    matches!(crate::auth::verified_role(req), Some(UserRole::Admin) | Some(UserRole::Remote))
}

/// Who may edit or delete a specific registered console: an admin or its owner.
/// Built-in consoles have no owner, so only an admin configures them.
pub(super) fn may_manage(req: &HttpRequest, console: &str) -> bool {
    if is_admin(req) {
        return true;
    }
    match (crate::auth::verified_user_id(req), super::vnc_registry::owner_of(console)) {
        (Some(uid), Some(owner)) => !owner.is_empty() && owner == uid,
        _ => false,
    }
}

struct VncConsole {
    target: Target,
    started: Instant,
    to_upstream: Option<mpsc::UnboundedSender<WsMessage>>,
    life: super::console_share::Lifeline,
    view_only: bool,
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
        let target = self.target.clone();
        actix::spawn(async move { connect_and_pump(target, rx, addr).await });
        // The account can be signed out or lose access while connected.
        ctx.run_interval(CHECK_EVERY, |act, ctx| {
            if !act.life.valid() || act.started.elapsed() > LONGEST {
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
                // A view-only share never sends input to the VM.
                if !self.view_only && bytes.len() <= BIGGEST_FRAME {
                    self.send(WsMessage::binary(bytes));
                }
            }
            // noVNC speaks binary; a text frame is harmless to pass through.
            Ok(ws::Message::Text(text)) => { if !self.view_only { self.send(WsMessage::text(text.to_string())); } }
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
    target: Target,
    rx: mpsc::UnboundedReceiver<WsMessage>,
    addr: actix::Addr<VncConsole>,
) {
    match target {
        Target::LocalWs(port) => pump_local_ws(port, rx, addr).await,
        Target::RawTcp(host, port) => pump_raw_tcp(&host, port, rx, addr).await,
    }
}
/// An internal bridge already speaks WebSocket; relay WS frames both ways.
async fn pump_local_ws(
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
/// An external VNC server is a raw RFB TCP endpoint; relay bytes both ways.
async fn pump_raw_tcp(
    host: &str,
    port: u16,
    mut rx: mpsc::UnboundedReceiver<WsMessage>,
    addr: actix::Addr<VncConsole>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let tcp = match tokio::net::TcpStream::connect((host, port)).await {
        Ok(tcp) => tcp,
        Err(_) => {
            addr.do_send(FromVm::Closed);
            return;
        }
    };
    let (mut rd, mut wr) = tcp.into_split();
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(WsMessage::Binary(b)) => if wr.write_all(&b).await.is_err() { break },
                Some(WsMessage::Close(_)) | None => break,
                _ => {}
            },
            read = rd.read(&mut buf) => match read {
                Ok(0) | Err(_) => break,
                Ok(n) => addr.do_send(FromVm::Binary(buf[..n].to_vec())),
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
    if !may_open(&req, &vm) {
        return Ok(HttpResponse::Forbidden().finish());
    }
    let Some(target) = resolve(&vm) else {
        return Ok(HttpResponse::NotFound().finish());
    };
    // Echo the "binary" subprotocol so noVNC's client is satisfied.
    ws::WsResponseBuilder::new(
        VncConsole {
            target,
            started: Instant::now(),
            to_upstream: None,
            life: super::console_share::Lifeline::Account(grant),
            view_only: false,
        },
        &req,
        stream,
    )
    .protocols(&["binary"])
    .start()
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_console);
    super::console_handlers::config(cfg);
}

