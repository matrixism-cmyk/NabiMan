//! One-use, target-bound admission for account terminal WebSockets.
use crate::{auth, jwt_sessions::SessionStore, models::ApiResponse, users::UserStore};
use actix_web::{post, web, HttpRequest, HttpResponse};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const TTL: Duration = Duration::from_secs(30);
const CAPACITY: usize = 4096;
const PER_SESSION: usize = 8;
const MAX_QUERY: usize = 4096;
#[derive(Default)]
pub struct Tickets {
    records: HashMap<String, Ticket>,
}
#[derive(Debug, PartialEq)]
enum IssueError {
    Capacity,
    Entropy,
}
struct Ticket {
    claims: auth::Claims,
    query: String,
    origin: Option<String>,
    expires: Instant,
}
pub type TicketStore = Arc<Mutex<Tickets>>;
pub fn new_store() -> TicketStore {
    Arc::new(Mutex::new(Tickets::default()))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IssueRequest {
    query: String,
}
#[derive(Serialize)]
struct Issued {
    ticket: String,
    expires_in_seconds: u64,
}

fn valid_query(query: &str) -> bool {
    if query.len() > MAX_QUERY || query.bytes().any(|b| !b.is_ascii() || b.is_ascii_control()) {
        return false;
    }
    let mut keys = HashSet::new();
    query.is_empty()
        || query.split('&').all(|p| {
            let Some((k, _)) = p.split_once('=') else {
                return false;
            };
            matches!(
                k,
                "scrollback"
                    | "cols"
                    | "rows"
                    | "keepalive"
                    | "timeout"
                    | "session_id"
                    | "join"
                    | "server_id"
                    | "no_saved_password"
                    | "ssh_host"
                    | "ssh_port"
                    | "ssh_user"
                    | "history"
            ) && keys.insert(k)
        })
}
fn digest(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
pub(crate) fn origin(req: &HttpRequest) -> Result<Option<String>, ()> {
    let Some(value) = req.headers().get("Origin") else {
        return Ok(None);
    };
    if req.headers().get_all("Origin").count() != 1 {
        return Err(());
    }
    let s = value.to_str().map_err(|_| ())?;
    if s.len() > 256 {
        return Err(());
    }
    let uri: actix_web::http::Uri = s.parse().map_err(|_| ())?;
    if !matches!(uri.scheme_str(), Some("http" | "https"))
        || uri.authority().is_none()
        || uri.authority().is_some_and(|a| a.as_str().contains('@'))
        || uri.path_and_query().is_some_and(|p| p.as_str() != "/")
    {
        return Err(());
    }
    Ok(Some(s.into()))
}
impl Tickets {
    fn issue(
        &mut self,
        claims: auth::Claims,
        query: String,
        origin: Option<String>,
        now: Instant,
        lifetime: Duration,
    ) -> Result<String, IssueError> {
        self.records.retain(|_, t| t.expires > now);
        if self.records.len() >= CAPACITY
            || self
                .records
                .values()
                .filter(|t| t.claims.sid == claims.sid)
                .count()
                >= PER_SESSION
        {
            return Err(IssueError::Capacity);
        }
        let mut bytes = [0u8; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| IssueError::Entropy)?;
        let token = hex::encode(bytes);
        self.records.insert(
            digest(&token),
            Ticket {
                claims,
                query,
                origin,
                expires: now + lifetime,
            },
        );
        Ok(token)
    }
    fn consume(
        &mut self,
        token: &str,
        query: &str,
        origin: Option<&str>,
        now: Instant,
    ) -> Option<auth::Claims> {
        if token.len() != 64
            || !token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        let ticket = self.records.remove(&digest(token))?;
        (ticket.expires > now && ticket.query == query && ticket.origin.as_deref() == origin)
            .then_some(ticket.claims)
    }
}
fn admission_lifetime(exp: usize, now: chrono::DateTime<chrono::Utc>) -> Option<Duration> {
    let expires = chrono::DateTime::<chrono::Utc>::from_timestamp(i64::try_from(exp).ok()?, 0)?;
    let remaining = expires.signed_duration_since(now).to_std().ok()?;
    (remaining >= Duration::from_secs(1)).then_some(TTL.min(remaining))
}

#[post("/api/terminal/tickets")]
async fn issue(
    req: HttpRequest,
    body: web::Json<IssueRequest>,
    store: web::Data<TicketStore>,
) -> HttpResponse {
    let Some(claims) = auth::verified_claims(&req) else {
        return HttpResponse::Unauthorized().finish();
    };
    if !valid_query(&body.query) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Invalid terminal connection parameters",
        ));
    }
    let origin = match origin(&req) {
        Ok(o) => o,
        Err(()) => {
            return HttpResponse::BadRequest()
                .json(ApiResponse::<()>::error("Invalid terminal origin"))
        }
    };
    let Some(lifetime) = admission_lifetime(claims.exp, chrono::Utc::now()) else {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Access token expired"));
    };
    let ticket = match store.lock().unwrap().issue(
        claims,
        body.query.clone(),
        origin,
        Instant::now(),
        lifetime,
    ) {
        Ok(t) => t,
        Err(IssueError::Capacity) => {
            return HttpResponse::TooManyRequests().json(ApiResponse::<()>::error(
                "Terminal connection ticket limit reached",
            ))
        }
        Err(IssueError::Entropy) => {
            return HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
                "Terminal connection entropy unavailable",
            ))
        }
    };
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(Issued {
            ticket,
            expires_in_seconds: lifetime.as_secs(),
        }))
}
/// Remove exactly one ticket field. All other bytes stay bound to the issuance.
pub(crate) fn redeem(req: &HttpRequest, store: &TicketStore) -> Option<auth::Claims> {
    let mut token = None;
    let mut parameters = Vec::new();
    for part in req.query_string().split('&') {
        if part.is_empty() {
            return None;
        }
        if let Some(t) = part.strip_prefix("ticket=") {
            if token.replace(t).is_some() {
                return None;
            }
        } else {
            parameters.push(part);
        }
    }
    let query = parameters.join("&");
    if !valid_query(&query) {
        return None;
    }
    let origin = origin(req).ok()?;
    store
        .lock()
        .unwrap()
        .consume(token?, &query, origin.as_deref(), Instant::now())
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(issue);
}

/// Checked before each account input and periodically while attached.
#[derive(Clone)]
pub(crate) struct ConnectionGrant {
    pub(crate) user_id: String,
    username: String,
    sid: String,
    sessions: SessionStore,
    users: UserStore,
    path: String,
}
impl ConnectionGrant {
    pub(crate) fn from_request(
        req: &HttpRequest,
        sessions: SessionStore,
        users: UserStore,
    ) -> Option<Self> {
        let claims = auth::verified_claims(req)?;
        Some(Self {
            user_id: auth::verified_user_id(req).filter(|id| !id.is_empty())?,
            username: claims.sub,
            sid: claims.sid,
            sessions,
            users,
            path: req.path().into(),
        })
    }
    pub(crate) fn valid(&self) -> bool {
        self.valid_for_method("GET")
    }
    pub(crate) fn valid_for_method(&self, method: &str) -> bool {
        if self.user_id.is_empty() {
            return false;
        }
        let session = self.sessions.lock().unwrap().get(&self.sid).cloned();
        let Some(session) =
            session.filter(|s| s.username == self.username && s.user_id == self.user_id)
        else {
            return false;
        };
        let Some(created) =
            chrono::NaiveDateTime::parse_from_str(&session.created_at, "%Y-%m-%d %H:%M:%S").ok()
        else {
            return false;
        };
        let age = chrono::Utc::now().signed_duration_since(created.and_utc());
        if age < chrono::Duration::zero() || age >= chrono::Duration::days(7) {
            return false;
        }
        crate::users::find_user_by_name(&self.users, &self.username).is_some_and(|u| {
            u.id == self.user_id
                && crate::mfa_policy::permitted(&u, session.mfa_verified, method, &self.path)
                && crate::scope_access::permission(&u, method, &self.path)
        })
    }
    pub(crate) fn can_access_session(
        &self,
        session: &crate::terminal::session::TmuxSession,
    ) -> bool {
        crate::terminal::session::visible_to(session, &self.user_id, &self.users)
    }
}
#[cfg(test)]
#[path = "terminal_ticket_tests.rs"]
mod tests;
