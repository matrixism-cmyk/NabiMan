//! Download links, for dragging a file out of the panel onto the desktop,
//! and stream links, for playing a video or a song in the preview.
//!
//! The browser saves a file dropped outside the window by fetching a URL on
//! its own, without the page's Authorization header. So the page asks for a
//! link first: a random ticket bound to the signed-in session, one server and
//! one exact path, good for a few minutes until its first fetch and for a
//! minute after it: a browser or a download manager taking the download over
//! fetches the URL again, and a spent link handed it an error saved as
//! `download.json` instead of the file. Redeeming it only
//! stands in for the header — the session must still be live, and every
//! permission check that follows runs as usual.
//!
//! A media element fetches its file many times, a range at a time, so a
//! stream link is good for every fetch of that one file, for a couple of
//! hours, and only on the stream route. Each kind opens only its own route.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::auth::Claims;

const TTL: Duration = Duration::from_secs(5 * 60);
const STREAM_TTL: Duration = Duration::from_secs(2 * 3600);
const CAPACITY: usize = 4096;
/// How long a download link stays good after its first fetch.
const REFETCH: Duration = Duration::from_secs(60);

/// Whose link it is: a signed-in session, or a share link's visitor.
pub(crate) enum Owner {
    Session(Claims),
    Share(String),
}

struct Link {
    owner: Owner,
    scope: String,
    path: String,
    expires: Instant,
    /// A stream link: kept after use, until it expires.
    reusable: bool,
    /// A download link fetched once: it expires `REFETCH` after that.
    used: bool,
}

impl Link {
    /// A download link's first fetch starts its short second life.
    fn spend(&mut self) {
        if !self.reusable && !self.used {
            self.used = true;
            self.expires = self.expires.min(Instant::now() + REFETCH);
        }
    }
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

/// A new short-lived download ticket for `scope` (a server id, `local`, or a share
/// token) and `path`.
pub(crate) fn issue(owner: Owner, scope: &str, path: &str) -> Result<String, String> {
    issue_kind(owner, scope, path, false)
}

/// A ticket for playing `path`: every range of it, for a while.
pub(crate) fn issue_stream(owner: Owner, scope: &str, path: &str) -> Result<String, String> {
    issue_kind(owner, scope, path, true)
}

pub(crate) fn stream_ttl_secs() -> u64 {
    STREAM_TTL.as_secs()
}

fn issue_kind(owner: Owner, scope: &str, path: &str, reusable: bool) -> Result<String, String> {
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
        Link { owner, scope: scope.into(), path: path.into(), expires: now + if reusable { STREAM_TTL } else { TTL }, reusable, used: false },
    );
    Ok(ticket)
}

/// Downloads' and streams' way in (registered in `remote.rs`).
pub(crate) fn url_pass(req: &actix_web::dev::ServiceRequest) -> Option<Option<Claims>> {
    redeem(req.method().as_str(), req.path(), req.query_string())
}

/// For the auth middleware. `None`: not a ticketed download, carry on as
/// usual. `Some(None)`: a ticket that does not admit this request.
/// `Some(Some(claims))`: admitted, as the session that asked for the link.
pub(crate) fn redeem(method: &str, path: &str, query: &str) -> Option<Option<Claims>> {
    let rest = path.strip_prefix("/api/terminal/sftp/")?;
    let (scope, stream) = match (rest.strip_suffix("/stream"), rest.strip_suffix("/download"), rest.strip_suffix("/download-folder")) {
        (Some(s), _, _) => (s, true),
        (_, Some(s), _) | (_, _, Some(s)) => (s, false),
        _ => return None,
    };
    if scope.is_empty() || scope.contains('/') {
        return None;
    }
    let params = web_params(query);
    let ticket = params.get("ticket")?;
    if method != "GET" {
        return Some(None);
    }
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    let key = digest(ticket);
    // Each kind of link opens only its own route; a download link tried on
    // the stream route is gone. A stream link stays for the next range.
    let Some(link) = map.get_mut(&key) else { return Some(None) };
    if link.reusable != stream {
        if !link.reusable {
            map.remove(&key);
        }
        return Some(None);
    }
    link.spend();
    Some(admit(link, scope, &params))
}

fn admit(link: &Link, scope: &str, params: &HashMap<String, String>) -> Option<Claims> {
    let admitted = link.expires > Instant::now()
        && link.scope == scope
        && params.get("path").is_some_and(|p| *p == link.path);
    match &link.owner {
        Owner::Session(claims) if admitted => Some(claims.clone()),
        _ => None,
    }
}

/// For a share visitor's download or stream: whether `ticket` was issued to
/// the share `token` for exactly `path`, for this route. A download link
/// lasts a minute after its first fetch; a stream link stays for the next range.
pub(crate) fn redeem_share(token: &str, path: &str, ticket: &str, stream: bool) -> bool {
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    let key = digest(ticket);
    let fits = |l: &Link| {
        l.expires > Instant::now() && l.scope == token && l.path == path && matches!(&l.owner, Owner::Share(t) if t == token)
    };
    let Some(link) = map.get_mut(&key) else { return false };
    if link.reusable {
        return stream && fits(link);
    }
    if stream {
        map.remove(&key);
        return false;
    }
    link.spend();
    fits(link)
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

    /// What a minute's wait does to a link already fetched.
    fn age(ticket: &str) {
        let mut map = store().lock().unwrap();
        let link = map.get_mut(&digest(ticket)).unwrap();
        assert!(link.used && link.expires <= Instant::now() + REFETCH);
        link.expires = Instant::now();
    }

    #[test]
    fn a_link_opens_one_file_for_a_minute_after_its_first_fetch() {
        let ticket = issue(Owner::Session(claims()), "srv_1", "/home/me/a b.txt").unwrap();
        let path = "/api/terminal/sftp/srv_1/download";
        let query = format!("path=%2Fhome%2Fme%2Fa%20b.txt&ticket={ticket}");
        assert!(matches!(redeem("GET", path, &query), Some(Some(c)) if c.sid == "s1"));
        // A download manager taking over fetches it again.
        assert!(matches!(redeem("GET", path, &query), Some(Some(_))));
        age(&ticket);
        assert!(matches!(redeem("GET", path, &query), Some(None)));
    }

    #[test]
    fn a_link_is_bound_to_its_file_and_server() {
        let ticket = issue(Owner::Session(claims()), "srv_1", "/etc/hosts").unwrap();
        let other_file = format!("path=%2Fetc%2Fshadow&ticket={ticket}");
        assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_1/download", &other_file), Some(None)));
        let ticket = issue(Owner::Session(claims()), "srv_1", "/etc/hosts").unwrap();
        let query = format!("path=%2Fetc%2Fhosts&ticket={ticket}");
        assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_2/download", &query), Some(None)));
    }

    #[test]
    fn share_links_never_stand_in_for_a_session() {
        let token = "t".repeat(64);
        let ticket = issue(Owner::Share(token.clone()), &token, "/etc/hosts").unwrap();
        // Replayed against the account route, it admits nothing.
        let query = format!("path=%2Fetc%2Fhosts&ticket={ticket}");
        assert!(matches!(redeem("GET", &format!("/api/terminal/sftp/{token}/download"), &query), Some(None)));
        let ticket = issue(Owner::Share(token.clone()), &token, "/etc/hosts").unwrap();
        assert!(!redeem_share(&token, "/etc/passwd", &ticket, false));
        let ticket = issue(Owner::Share(token.clone()), &token, "/etc/hosts").unwrap();
        assert!(redeem_share(&token, "/etc/hosts", &ticket, false));
        assert!(redeem_share(&token, "/etc/hosts", &ticket, false), "fetched again");
        age(&ticket);
        assert!(!redeem_share(&token, "/etc/hosts", &ticket, false), "spent a minute later");
        let stream = issue_stream(Owner::Share(token.clone()), &token, "/m.mp4").unwrap();
        assert!(redeem_share(&token, "/m.mp4", &stream, true));
        assert!(redeem_share(&token, "/m.mp4", &stream, true), "every range");
        assert!(!redeem_share(&token, "/m.mp4", &stream, false), "not a download");
    }

    #[test]
    fn a_folder_link_opens_that_folder_as_an_archive() {
        let ticket = issue(Owner::Session(claims()), "srv_1", "/var/log").unwrap();
        let query = format!("path=%2Fvar%2Flog&ticket={ticket}");
        assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_1/download-folder", &query), Some(Some(_))));
    }

    #[test]
    fn a_stream_link_plays_one_file_many_times_and_only_plays() {
        let ticket = issue_stream(Owner::Session(claims()), "srv_1", "/v/a.mp4").unwrap();
        let query = format!("path=%2Fv%2Fa.mp4&ticket={ticket}");
        for _ in 0..3 {
            assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_1/stream", &query), Some(Some(_))));
        }
        assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_1/download", &query), Some(None)));
        let other = format!("path=%2Fetc%2Fshadow&ticket={ticket}");
        assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_1/stream", &other), Some(None)));
        // A download link does not stream, and is spent trying.
        let once = issue(Owner::Session(claims()), "srv_1", "/v/a.mp4").unwrap();
        let q = format!("path=%2Fv%2Fa.mp4&ticket={once}");
        assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_1/stream", &q), Some(None)));
        assert!(matches!(redeem("GET", "/api/terminal/sftp/srv_1/download", &q), Some(None)));
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
