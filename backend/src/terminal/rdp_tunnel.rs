//! RDP consoles in the browser, via guacd (Apache Guacamole daemon on
//! 127.0.0.1:4822). NabiMan performs the Guacamole handshake with guacd,
//! injecting the RDP login server-side (the password never reaches the
//! browser), then relays the protocol to the browser's guacamole-common-js.
//! RDP carries clipboard, files and Korean input natively.

use std::time::{Duration, Instant};

use actix::{Actor, ActorContext, AsyncContext, Handler, Message, StreamHandler};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

const GUACD: &str = "127.0.0.1:4822";
const LONGEST: Duration = Duration::from_secs(12 * 60 * 60);
const CHECK_EVERY: Duration = Duration::from_secs(30);

#[derive(Clone)]
struct RdpParams {
    host: String,
    port: u16,
    user: String,
    password: String,
    width: u32,
    height: u32,
    dpi: u32,
}

/// One Guacamole instruction: `len.value,len.value,…;` where the length is the
/// number of Unicode characters, not bytes.
fn inst(elems: &[&str]) -> Vec<u8> {
    let mut s = String::new();
    for (i, e) in elems.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&e.chars().count().to_string());
        s.push('.');
        s.push_str(e);
    }
    s.push(';');
    s.into_bytes()
}

/// Reads one complete instruction's elements from guacd (ASCII during the
/// handshake), returning the opcode-plus-args and any trailing bytes read.
async fn read_instruction(rd: &mut tokio::net::tcp::OwnedReadHalf) -> Option<Vec<String>> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(end) = buf.iter().position(|&b| b == b';') {
            let body = String::from_utf8_lossy(&buf[..end]).to_string();
            let mut elems = Vec::new();
            let mut rest = body.as_str();
            while !rest.is_empty() {
                let dot = rest.find('.')?;
                let n: usize = rest[..dot].parse().ok()?;
                let val: String = rest[dot + 1..].chars().take(n).collect();
                let consumed = dot + 1 + val.chars().map(|c| c.len_utf8()).sum::<usize>();
                elems.push(val);
                rest = rest.get(consumed..).unwrap_or("");
                rest = rest.strip_prefix(',').unwrap_or(rest);
            }
            return Some(elems);
        }
        let n = rd.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

struct RdpConsole {
    params: RdpParams,
    started: Instant,
    to_guacd: Option<mpsc::UnboundedSender<Vec<u8>>>,
    life: super::console_share::Lifeline,
    view_only: bool,
}

/// The opcode of a Guacamole instruction (`len.opcode,…;`), if parseable.
fn guac_opcode(text: &str) -> Option<&str> {
    let dot = text.find('.')?;
    let n: usize = text.get(..dot)?.parse().ok()?;
    text.get(dot + 1..dot + 1 + n)
}

/// Bytes in one UTF-8 character, from its lead byte.
fn utf8_len(b: u8) -> usize {
    match b {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// How many leading bytes of `s` form whole Guacamole instructions. The
/// browser's WebSocketTunnel parses each WebSocket message as complete
/// instructions and does not buffer a partial one across messages, so NabiMan
/// must never split an instruction (e.g. a large base64 image) across frames:
/// it relays up to the last `;` and keeps the rest until more arrives.
///
/// Each element is `length.value`, length counted in Unicode characters, with
/// elements joined by `,` and the instruction ended by `;`. A value may itself
/// contain `,` or `;`, so boundaries are found by length, not by scanning.
fn complete_instructions_end(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut complete = 0; // byte offset just past the last ';'
    let mut i = 0;
    loop {
        // Element length: ASCII digits up to the '.'.
        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j == i || j >= bytes.len() || bytes[j] != b'.' {
            break; // no digits, or the '.' hasn't arrived yet
        }
        let Ok(len) = s[i..j].parse::<usize>() else { break };
        // Skip the '.' then `len` characters of value.
        let mut k = j + 1;
        let mut chars = 0;
        while k < bytes.len() && chars < len {
            k += utf8_len(bytes[k]);
            chars += 1;
        }
        if chars < len || k >= bytes.len() {
            break; // value, or its trailing separator, not fully here yet
        }
        let sep = bytes[k];
        i = k + 1;
        if sep == b';' {
            complete = i; // a whole instruction ends here
        } else if sep != b',' {
            break; // malformed; stop at the last good boundary
        }
    }
    complete
}

#[derive(Message)]
#[rtype(result = "()")]
enum FromGuacd {
    Text(String),
    Closed,
}

impl Actor for RdpConsole {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.to_guacd = Some(tx);
        let addr = ctx.address();
        let params = self.params.clone();
        actix::spawn(async move { pump(params, rx, addr).await });
        ctx.run_interval(CHECK_EVERY, |act, ctx| {
            if !act.life.valid() || act.started.elapsed() > LONGEST {
                ctx.close(None);
                ctx.stop();
            }
        });
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        self.to_guacd = None;
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for RdpConsole {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                // A view-only share may keep the display in sync but cannot drive
                // the VM: mouse, keyboard and clipboard instructions are dropped.
                if self.view_only && matches!(guac_opcode(&text), Some("mouse" | "key" | "clipboard")) {
                    return;
                }
                if let Some(tx) = &self.to_guacd {
                    let _ = tx.send(text.into_bytes().to_vec());
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

impl Handler<FromGuacd> for RdpConsole {
    type Result = ();
    fn handle(&mut self, msg: FromGuacd, ctx: &mut Self::Context) {
        match msg {
            FromGuacd::Text(t) => ctx.text(t),
            FromGuacd::Closed => {
                ctx.close(None);
                ctx.stop();
            }
        }
    }
}

/// Handshakes guacd for an RDP connection, then relays the protocol both ways.
async fn pump(
    p: RdpParams,
    mut rx: mpsc::UnboundedReceiver<Vec<u8>>,
    addr: actix::Addr<RdpConsole>,
) {
    let tcp = match TcpStream::connect(GUACD).await {
        Ok(t) => t,
        Err(_) => {
            addr.do_send(FromGuacd::Closed);
            return;
        }
    };
    let (mut rd, mut wr) = tcp.into_split();
    // select → args
    if wr.write_all(&inst(&["select", "rdp"])).await.is_err() {
        addr.do_send(FromGuacd::Closed);
        return;
    }
    let Some(args) = read_instruction(&mut rd).await else {
        addr.do_send(FromGuacd::Closed);
        return;
    };
    // Client display capabilities, then the connection parameters.
    let (w, h, dpi) = (p.width.to_string(), p.height.to_string(), p.dpi.to_string());
    let hs = [
        inst(&["size", &w, &h, &dpi]),
        inst(&["audio"]),
        inst(&["video"]),
        inst(&["image", "image/png", "image/jpeg"]),
    ];
    for m in hs {
        if wr.write_all(&m).await.is_err() {
            addr.do_send(FromGuacd::Closed);
            return;
        }
    }
    let port = p.port.to_string();
    // Values keyed by the parameter names guacd just listed (args[0] is VERSION).
    let value_for = |name: &str| -> String {
        match name {
            "hostname" => p.host.clone(),
            "port" => port.clone(),
            "username" => p.user.clone(),
            "password" => p.password.clone(),
            "width" => w.clone(),
            "height" => h.clone(),
            "dpi" => dpi.clone(),
            "security" => "any".into(),
            "ignore-cert" => "true".into(),
            "resize-method" => "display-update".into(),
            "enable-wallpaper" => "true".into(),
            _ => String::new(),
        }
    };
    // args = ["args", VERSION, <param names…>]; the connect values line up with
    // VERSION + the param names (echo VERSION, then a value per parameter).
    let fields = args.get(1..).unwrap_or(&[]);
    let mut connect: Vec<String> = Vec::with_capacity(fields.len() + 1);
    connect.push("connect".into());
    for (i, name) in fields.iter().enumerate() {
        connect.push(if i == 0 { name.clone() } else { value_for(name) });
    }
    let refs: Vec<&str> = connect.iter().map(|s| s.as_str()).collect();
    if wr.write_all(&inst(&refs)).await.is_err() {
        addr.do_send(FromGuacd::Closed);
        return;
    }
    // Relay. guacd speaks UTF-8 Guacamole; forward only whole instructions so
    // the browser never sees one split across WebSocket messages.
    let mut pending: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(bytes) => if wr.write_all(&bytes).await.is_err() { break },
                None => break,
            },
            read = rd.read(&mut chunk) => match read {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    pending.extend_from_slice(&chunk[..n]);
                    // Parse only the valid-UTF-8 prefix; a multi-byte character
                    // straddling the read boundary waits for its rest.
                    let valid = match std::str::from_utf8(&pending) {
                        Ok(_) => pending.len(),
                        Err(e) => e.valid_up_to(),
                    };
                    let end = complete_instructions_end(
                        std::str::from_utf8(&pending[..valid]).unwrap_or(""),
                    );
                    if end > 0 {
                        let text = String::from_utf8_lossy(&pending[..end]).to_string();
                        pending.drain(..end);
                        addr.do_send(FromGuacd::Text(text));
                    }
                }
            },
        }
    }
    addr.do_send(FromGuacd::Closed);
}

fn q(query: &str, key: &str) -> Option<String> {
    super::parse_query_param(query, key)
}

/// `GET /api/remote/rdp?target=<id>&ticket=…` (+ guacamole display params) —
/// the RDP console WebSocket.
#[actix_web::get("/api/remote/rdp")]
async fn ws_rdp(
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
    let query = req.query_string();
    let Some(target) = q(query, "target") else {
        return Ok(HttpResponse::BadRequest().finish());
    };
    if !super::vnc_console::may_open(&req, &target) {
        return Ok(HttpResponse::Forbidden().finish());
    }
    let Some((host, port, user, password)) = super::vnc_registry::rdp_login(&target) else {
        return Ok(HttpResponse::NotFound().finish());
    };
    let dims = |k: &str, d: u32| q(query, k).and_then(|v| v.parse().ok()).filter(|n| *n > 0 && *n <= 8000).unwrap_or(d);
    let params = RdpParams {
        host,
        port,
        user,
        password,
        width: dims("GUAC_WIDTH", 1280),
        height: dims("GUAC_HEIGHT", 800),
        dpi: dims("GUAC_DPI", 96),
    };
    ws::WsResponseBuilder::new(
        RdpConsole { params, started: Instant::now(), to_guacd: None, life: super::console_share::Lifeline::Account(grant), view_only: false },
        &req,
        stream,
    )
    .protocols(&["guacamole"])
    .start()
}

/// Opens an RDP console for a share link (no account), view-only or control.
pub(super) fn open_shared_rdp(
    req: &HttpRequest,
    stream: web::Payload,
    console: &str,
    view_only: bool,
    token: String,
) -> Result<HttpResponse, actix_web::Error> {
    let Some((host, port, user, password)) = super::vnc_registry::rdp_login(console) else {
        return Ok(HttpResponse::NotFound().finish());
    };
    let params = RdpParams { host, port, user, password, width: 1280, height: 800, dpi: 96 };
    ws::WsResponseBuilder::new(
        RdpConsole { params, started: Instant::now(), to_guacd: None, life: super::console_share::Lifeline::Share(token), view_only },
        req,
        stream,
    )
    .protocols(&["guacamole"])
    .start()
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(ws_rdp);
}
