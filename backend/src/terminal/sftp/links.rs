//! Single-use download links, for dragging a file out of the panel onto the
//! desktop.
//!
//! The browser saves a file dropped outside the window by fetching a URL on
//! its own, without the page's Authorization header. So the page asks for a
//! link first: a random ticket bound to the signed-in session, one server and
//! one exact path, good for one fetch within a few minutes. Redeeming it only
//! stands in for the header — the session must still be live, and every
//! permission check that follows runs as usual.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::auth::Claims;

const TTL: Duration = Duration::from_secs(5 * 60);
const CAPACITY: usize = 4096;

struct Link {
    claims: Claims,
    scope: String,
    path: String,
    expires: Instant,
}

fn store() -> &'static Mutex<HashMap<String, Link>> {
    static STORE: OnceLock<Mutex<HashMap<String, Link>>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

/// Only a digest of the ticket is kept, so a memory dump holds no usable link.
fn digest(ticket: &str) -> String {
    hex::encode(Sha256::digest(ticket.as_bytes()))
}

pub(crate) fn ttl_secs() -> u64 {
    TTL.as_secs()
}

/// A new ticket for `scope` (a server id or `local`) and `path`.
pub(crate) fn issue(claims: Claims, scope: &str, path: &str) -> Result<String, String> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| "No randomness available".to_string())?;
    let ticket = hex::encode(bytes);
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    map.retain(|_, l| l.expires > now);
    if map.len() >= CAPACITY {
        return Err("Too many download links are open; try again shortly".into());
    }
    map.insert(
        digest(&ticket),
        Link { claims, scope: scope.into(), path: path.into(), expires: now + TTL },
    );
    Ok(ticket)
}

/// For the auth middleware. `None`: not a ticketed download, carry on as
/// usual. `Some(None)`: a ticket that does not admit this request.
/// `Some(Some(claims))`: admitted, as the session that asked for the link.
pub(crate) fn redeem(method: &str, path: &str, query: &str) -> Option<Option<Claims>> {
    let scope = path
        .strip_prefix("/api/terminal/sftp/")?
        .strip_suffix("/download")
        .filter(|s| !s.is_empty() && !s.contains('/'))?;
    let params = web_params(query);
    let ticket = params.get("ticket")?;
    if method != "GET" {
        return Some(None);
    }
    let Some(link) = store()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&digest(ticket))
    else {
        return Some(None);
    };
    let admitted = link.expires > Instant::now()
        && link.scope == scope
        && params.get("path").is_some_and(|p| *p == link.path);
    Some(admitted.then_some(link.claims))
}

fn web_params(query: &str) -> HashMap<String, String> {
    actix_web::web::Query::<HashMap<String, String>>::from_query(query)
        .map(|q| q.into_inner())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims() -> Claims {
        Claims { sub: "admin".into(), role: "admin".into(), exp: 0, iat: 0, sid: "s1".into() }
    }

    #[test]
    fn a_link_opens_one_file_once() {
        let ticket = issue(claims(), "srv_1", "/home/me/a b.txt").unwrap();
        let path = "/api/terminal/sftp/srv_1/download";
        let query = format!("path=%2Fhome%2Fme%2Fa%20b.txt&ticket={ticket}");
        assert!(matches!(redeem("GET", path, &query), Some(Some(c)) if c.sid == "s1"));
        // Spent.
        assert!(matches!(redeem("GET", path, &query), Some(None)));
    }

    #[test]
    fn a_link_is_bound_to_its_file_and_server() {
        let ticket = issue(claims(), "srv_1", "/etc/hosts").unwrap();
        let other_file = format!("path=%2Fetc%2Fshadow&ticket={ticket}");
        assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_1/download", &other_file), Some(None)));
        let ticket = issue(claims(), "srv_1", "/etc/hosts").unwrap();
        let query = format!("path=%2Fetc%2Fhosts&ticket={ticket}");
        assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_2/download", &query), Some(None)));
    }

    #[test]
    fn other_requests_are_left_alone() {
        assert!(redeem("GET", "/api/terminal/sftp/srv_1/download", "path=%2Fx").is_none());
        assert!(redeem("GET", "/api/terminal/sftp/srv_1/list", "ticket=abc").is_none());
        assert!(matches!(redeem("POST", "/api/terminal/sftp/srv_1/download", "ticket=abc"), Some(None)));
        assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_1/download", "ticket=unknown"), Some(None))
            || redeem("GET", "/api/terminal/sftp/srv_1/download", "ticket=unknown").is_none());
    }
}
