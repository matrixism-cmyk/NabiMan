//! The browser's side of dictation: a WebSocket that takes the opening
//! message and 16 kHz PCM from the page and passes them to the speech
//! service, and passes its JSON (partial, final, error) back.

use std::time::{Duration, Instant};

use actix::{Actor, ActorContext, AsyncContext, Handler, Message, StreamHandler};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::WebSocketStream;

use super::voice::{self, Live};
use super::{load, Settings, LANGUAGES};
use crate::terminal::share::{live_link, ShareLink};
use crate::terminal::share_store::ShareStore;
use crate::terminal_tickets::ConnectionGrant;
use crate::users::UserStore;

/// The speech service ends a live dictation after an hour; so does this.
const LONGEST: Duration = Duration::from_secs(61 * 60);
/// 16 kHz × 16 bit for an hour, and a little more.
const MOST_AUDIO: u64 = 16_000 * 2 * 3_660;
/// One audio frame from the page; it sends 0.1 s (3200 bytes) at a time.
const BIGGEST_FRAME: usize = 64 * 1024;
const CHECK_EVERY: Duration = Duration::from_secs(30);

#[derive(Message)]
#[rtype(result = "()")]
enum FromVoice {
    Text(String),
    /// The service closed the dictation; with a reason when it failed.
    Closed(Option<String>),
}

/// Who is dictating, checked again while they do.
enum Admission {
    Account(ConnectionGrant),
    /// A share link's visitor: the link must stay live, as it was, and writable.
    Share { link: Box<ShareLink>, shares: ShareStore, users: UserStore },
}

impl Admission {
    fn valid(&self) -> bool {
        match self {
            Admission::Account(grant) => grant.valid(),
            Admission::Share { link, shares, users } => live_link(shares, users, &link.token)
                .is_some_and(|now| now.same_authorization(link) && !now.read_only),
        }
    }
}

struct Dictation {
    grant: Admission,
    settings: Settings,
    key: String,
    to_voice: Option<mpsc::UnboundedSender<WsMessage>>,
    opened: bool,
    audio: u64,
    since: Instant,
}

impl Dictation {
    fn send(&self, message: WsMessage) {
        if let Some(tx) = &self.to_voice {
            let _ = tx.send(message);
        }
    }
    fn fail(&mut self, ctx: &mut ws::WebsocketContext<Self>, message: &str) {
        ctx.text(serde_json::json!({ "type": "error", "message": message }).to_string());
        ctx.close(None);
        ctx.stop();
    }
}

impl Actor for Dictation {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.to_voice = Some(tx);
        let addr = ctx.address();
        let (settings, key) = (self.settings.clone(), self.key.clone());
        actix::spawn(async move {
            match voice::connect(&settings, &key).await {
                Ok(Live::Plain(ws)) => pump(ws, rx, addr).await,
                Ok(Live::Tls(ws)) => pump(*ws, rx, addr).await,
                Err(e) => addr.do_send(FromVoice::Closed(Some(e))),
            }
        });
        // The account can be signed out or lose the terminal meanwhile.
        ctx.run_interval(CHECK_EVERY, |act, ctx| {
            if !act.grant.valid() || act.since.elapsed() > LONGEST {
                act.fail(ctx, "Dictation ended: sign-in or time limit");
            }
        });
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        // Dropping the sender closes the service's side too.
        self.to_voice = None;
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for Dictation {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { return };
                if value.get("type").and_then(|t| t.as_str()) == Some("stop") {
                    self.send(WsMessage::text(r#"{"type":"stop"}"#));
                } else if !self.opened {
                    // The opening message is NabiMan's own: the language the
                    // page asks for (if known), and never kept by the service.
                    let asked = value.get("language").and_then(|l| l.as_str()).filter(|l| LANGUAGES.contains(l));
                    let language = asked.unwrap_or(self.settings.language());
                    let open = serde_json::json!({ "language": language, "title": "NabiMan terminal", "save": false });
                    self.opened = true;
                    self.send(WsMessage::text(open.to_string()));
                }
            }
            Ok(ws::Message::Binary(bytes)) => {
                if !self.opened || bytes.len() > BIGGEST_FRAME {
                    return;
                }
                self.audio += bytes.len() as u64;
                if self.audio > MOST_AUDIO {
                    self.fail(ctx, "Dictation is limited to an hour at a time");
                    return;
                }
                self.send(WsMessage::binary(bytes));
            }
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

impl Handler<FromVoice> for Dictation {
    type Result = ();
    fn handle(&mut self, msg: FromVoice, ctx: &mut Self::Context) {
        match msg {
            FromVoice::Text(text) => ctx.text(text),
            FromVoice::Closed(Some(reason)) => self.fail(ctx, &reason),
            FromVoice::Closed(None) => {
                ctx.close(None);
                ctx.stop();
            }
        }
    }
}

/// Carries the page's messages to the service and the service's to the page
/// until either side closes.
async fn pump<S: AsyncRead + AsyncWrite + Unpin>(
    ws: WebSocketStream<S>,
    mut rx: mpsc::UnboundedReceiver<WsMessage>,
    addr: actix::Addr<Dictation>,
) {
    let (mut sink, mut stream) = ws.split();
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(message) => if sink.send(message).await.is_err() { break },
                None => { let _ = sink.send(WsMessage::Close(None)).await; break }
            },
            incoming = stream.next() => match incoming {
                Some(Ok(WsMessage::Text(text))) => addr.do_send(FromVoice::Text(text.to_string())),
                Some(Ok(WsMessage::Close(_))) | None => break,
                Some(Err(e)) => { addr.do_send(FromVoice::Closed(Some(format!("Dictation connection lost: {e}")))); return }
                _ => {}
            },
        }
    }
    addr.do_send(FromVoice::Closed(None));
}

/// `GET /api/terminal/dictation?purpose=dictation&ticket=…`: admitted like a
/// terminal, by an account that may use one.
pub(super) async fn start(
    req: HttpRequest,
    stream: web::Payload,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    users: web::Data<crate::users::UserStore>,
) -> Result<HttpResponse, actix_web::Error> {
    if !dictation_query(&req) {
        return Ok(HttpResponse::BadRequest().finish());
    }
    let Some(grant) = ConnectionGrant::from_request(&req, sessions.get_ref().clone(), users.get_ref().clone())
        .filter(|g| g.valid())
    else {
        return Ok(HttpResponse::Unauthorized().finish());
    };
    begin(Admission::Account(grant), &req, stream)
}

fn begin(grant: Admission, req: &HttpRequest, stream: web::Payload) -> Result<HttpResponse, actix_web::Error> {
    let settings = load();
    let Some(key) = settings.key() else {
        return Ok(HttpResponse::ServiceUnavailable().finish());
    };
    let actor = Dictation { grant, settings, key, to_voice: None, opened: false, audio: 0, since: Instant::now() };
    ws::start(actor, req, stream)
}

fn dictation_query(req: &HttpRequest) -> bool {
    req.query_string() == "purpose=dictation" || req.query_string().starts_with("purpose=dictation&")
}

/// `GET /api/share/{token}/dictation?purpose=dictation&ticket=…`: a share
/// link's visitor, admitted by a ticket the link's own admission issued
/// (password and limits included), into a session that is still running,
/// through a link that lets them type.
pub(super) async fn start_shared(
    req: HttpRequest,
    path: web::Path<String>,
    stream: web::Payload,
    tickets: web::Data<crate::terminal::share_ticket::ShareTickets>,
    shares: web::Data<ShareStore>,
    users: web::Data<UserStore>,
    sessions: web::Data<crate::terminal::session::PtyStore>,
) -> Result<HttpResponse, actix_web::Error> {
    let token = path.into_inner();
    if !dictation_query(&req) {
        return Ok(HttpResponse::BadRequest().finish());
    }
    let Some(snapshot) = crate::terminal::share_ticket::redeem(&req, &token, &tickets) else {
        return Ok(HttpResponse::Unauthorized().finish());
    };
    let Some(link) = live_link(&shares, &users, &token).filter(|l| l.same_authorization(&snapshot)) else {
        return Ok(HttpResponse::Forbidden().finish());
    };
    if link.read_only || link.files_only() {
        return Ok(HttpResponse::Forbidden().finish());
    }
    let running = sessions.lock().ok()
        .and_then(|m| m.get(&link.session_id).filter(|s| s.owned_by(&link.owner_id)).map(|s| s.tmux_name.clone()))
        .is_some_and(|name| crate::terminal::session::tmux_session_exists(&name));
    if !running {
        return Ok(HttpResponse::Gone().finish());
    }
    let grant = Admission::Share { link: Box::new(link), shares: shares.get_ref().clone(), users: users.get_ref().clone() };
    begin(grant, &req, stream)
}
