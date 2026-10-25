//! One-use, target-bound admission for account terminal WebSockets (and
//! dictation's). The checks that keep a connection admitted afterwards are
//! NabiMan's, shared with every stream: `stream_access::ConnectionGrant`.
use crate::{auth, models::ApiResponse};
use actix_web::{dev::ServiceRequest, post, web, HttpRequest, HttpResponse};
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
            let Some((k, v)) = p.split_once('=') else {
                return false;
            };
            // Dictation's own ticket: it opens nothing else.
            if k == "purpose" {
                return v == "dictation" && keys.insert(k);
            }
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
                    | "telnet_host"
                    | "telnet_port"
                    | "telnet_user"
                    | "vm"
                    | "target"
                    | "serial"
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
/// The terminal's way in (registered in `remote.rs`): its WebSockets, and
/// dictation's, open with a ticket — never with an account token in the URL.
pub(crate) fn url_pass(req: &ServiceRequest) -> Option<Option<auth::Claims>> {
    let path = req.path();
    if path == "/api/remote/rdp" {
        // guacamole-common-js appends display params to the query, so the RDP
        // ticket is bound to the target alone, not the whole query.
        if req.query_string().split('&').any(|p| p.starts_with("ticket=")) {
            let Some(store) = req.app_data::<web::Data<TicketStore>>() else { return Some(None) };
            return Some(redeem_target(req.request(), store));
        }
        return Some(None);
    }
    if path != "/api/terminal"
        && path != "/api/terminal/dictation"
        && path != "/api/remote/console"
        && path != "/api/remote/serial"
    {
        return None;
    }
    let has = |key: &str| req.query_string().split('&').any(|p| p.starts_with(key));
    if has("ticket=") {
        let Some(store) = req.app_data::<web::Data<TicketStore>>() else { return Some(None) };
        return Some(redeem(req.request(), store));
    }
    has("token=").then_some(None)
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

/// Like `redeem`, but binds the ticket to the `target` parameter only — other
/// query parameters (the viewer's display size, etc.) do not affect admission.
pub(crate) fn redeem_target(req: &HttpRequest, store: &TicketStore) -> Option<auth::Claims> {
    let mut token = None;
    let mut target = None;
    for part in req.query_string().split('&') {
        if let Some(t) = part.strip_prefix("ticket=") {
            if token.replace(t).is_some() {
                return None;
            }
        } else if part.starts_with("target=") {
            if target.replace(part).is_some() {
                return None;
            }
        }
    }
    let query = target?;
    if !valid_query(query) {
        return None;
    }
    let origin = origin(req).ok()?;
    store.lock().unwrap().consume(token?, query, origin.as_deref(), Instant::now())
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(issue);
}

#[cfg(test)]
#[path = "terminal_ticket_tests.rs"]
mod tests;
