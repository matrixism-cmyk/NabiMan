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
    if let Some(port) = port_for(vm) {
        return Some(Target::LocalWs(port));
    }
    super::vnc_registry::find(vm).map(|c| Target::RawTcp(c.host, c.port))
}

/// Registering or changing a console is an administrator action.
fn may_manage_console(req: &HttpRequest) -> bool {
    matches!(crate::auth::verified_role(req), Some(UserRole::Admin))
}

fn is_admin(req: &HttpRequest) -> bool {
    matches!(crate::auth::verified_role(req), Some(UserRole::Admin))
}

/// VM consoles are sensitive internal infrastructure. An administrator may open
/// any; anyone else only a console delegated to their account (vnc_access).
fn may_open(req: &HttpRequest, console: &str) -> bool {
    is_admin(req)
        || crate::auth::verified_user_id(req)
            .is_some_and(|uid| super::vnc_access::allowed(&uid, console))
}

struct VncConsole {
    target: Target,
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
        let target = self.target.clone();
        actix::spawn(async move { connect_and_pump(target, rx, addr).await });
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
            grant,
        },
        &req,
        stream,
    )
    .protocols(&["binary"])
    .start()
}

/// `GET /api/remote/consoles` — the VM consoles the caller may open: the
/// built-in internal bridges (read-only) and the admin-registered ones.
#[actix_web::get("/api/remote/consoles")]
async fn list_consoles(req: HttpRequest) -> HttpResponse {
    let admin = is_admin(&req);
    let uid = crate::auth::verified_user_id(&req).unwrap_or_default();
    let visible = |key: &str| admin || super::vnc_access::allowed(&uid, key);
    let mut list: Vec<serde_json::Value> = CONSOLES
        .iter()
        .filter(|(name, _, _)| visible(name))
        .map(|(name, label, _)| serde_json::json!({ "name": name, "label": label, "builtin": true }))
        .collect();
    for c in super::vnc_registry::load() {
        if visible(&c.id) {
            list.push(serde_json::json!({
                "name": c.id, "label": c.label, "host": c.host, "port": c.port, "builtin": false
            }));
        }
    }
    HttpResponse::Ok().json(ApiResponse::ok(list))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsoleBody {
    label: String,
    host: String,
    port: u16,
}

fn as_json(c: super::vnc_registry::Console) -> serde_json::Value {
    serde_json::json!({ "name": c.id, "label": c.label, "host": c.host, "port": c.port, "builtin": false })
}

/// `POST /api/remote/consoles` — register an external VNC console (admin).
#[actix_web::post("/api/remote/consoles")]
async fn create_console(req: HttpRequest, body: web::Json<ConsoleBody>) -> HttpResponse {
    if !may_manage_console(&req) {
        return HttpResponse::Forbidden().finish();
    }
    match super::vnc_registry::create(&body.label, &body.host, body.port) {
        Ok(c) => HttpResponse::Ok().json(ApiResponse::ok(as_json(c))),
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    }
}

/// `PUT /api/remote/consoles/{id}` — edit a registered console (admin).
#[actix_web::put("/api/remote/consoles/{id}")]
async fn update_console(req: HttpRequest, id: web::Path<String>, body: web::Json<ConsoleBody>) -> HttpResponse {
    if !may_manage_console(&req) {
        return HttpResponse::Forbidden().finish();
    }
    match super::vnc_registry::update(&id, &body.label, &body.host, body.port) {
        Ok(c) => HttpResponse::Ok().json(ApiResponse::ok(as_json(c))),
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    }
}

/// `DELETE /api/remote/consoles/{id}` — remove a registered console (admin).
#[actix_web::delete("/api/remote/consoles/{id}")]
async fn delete_console(req: HttpRequest, id: web::Path<String>) -> HttpResponse {
    if !may_manage_console(&req) {
        return HttpResponse::Forbidden().finish();
    }
    match super::vnc_registry::delete(&id) {
        Ok(()) => {
            super::vnc_access::forget(&id);
            HttpResponse::Ok().json(ApiResponse::ok(()))
        }
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    }
}
/// `GET /api/remote/consoles/{key}/access` — who may open this console (admin).
#[actix_web::get("/api/remote/consoles/{key}/access")]
async fn get_access(req: HttpRequest, key: web::Path<String>) -> HttpResponse {
    if !is_admin(&req) {
        return HttpResponse::Forbidden().finish();
    }
    HttpResponse::Ok().json(ApiResponse::ok(super::vnc_access::allowed_users(&key)))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AccessBody {
    users: Vec<String>,
}
/// `PUT /api/remote/consoles/{key}/access` — set who may open this console (admin).
#[actix_web::put("/api/remote/consoles/{key}/access")]
async fn set_access(req: HttpRequest, key: web::Path<String>, body: web::Json<AccessBody>) -> HttpResponse {
    if !is_admin(&req) {
        return HttpResponse::Forbidden().finish();
    }
    // Only a real console key may carry a grant.
    if port_for(&key).is_none() && super::vnc_registry::find(&key).is_none() {
        return HttpResponse::NotFound().finish();
    }
    match super::vnc_access::set(&key, body.users.clone()) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::ok(())),
        Err(e) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e)),
    }
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_console)
        .service(list_consoles)
        .service(create_console)
        .service(update_console)
        .service(delete_console)
        .service(get_access)
        .service(set_access);
}
