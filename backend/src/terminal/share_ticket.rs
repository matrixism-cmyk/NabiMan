//! Short, one-use share admission. No reusable signed password proof or JWT.
use super::share::ShareLink;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(crate) const TTL: Duration = Duration::from_secs(30);
const CAPACITY: usize = 4096;
const PER_LINK: usize = 8;
const WINDOW: Duration = Duration::from_secs(60);
const RATE_CAPACITY: usize = 8192;
const LINK_RATE: u32 = 30;
const PEER_RATE: u32 = 60;
const GLOBAL_RATE: u32 = 240;
const PER_LINK_CONNECTIONS: usize = 8;
const GLOBAL_CONNECTIONS: usize = 64;
struct Ticket {
    link: ShareLink,
    query: String,
    origin: Option<String>,
    expires: Instant,
}
#[derive(Default)]
pub(super) struct Tickets {
    records: HashMap<String, Ticket>,
}
#[derive(Debug, PartialEq)]
pub(super) enum IssueError {
    Capacity,
    Entropy,
}
struct Bucket {
    start: Instant,
    count: u32,
}
#[derive(Default)]
pub(super) struct Attempts {
    buckets: HashMap<(u8, String), Bucket>,
}
pub struct Admission {
    pub(super) tickets: Mutex<Tickets>,
    pub(super) attempts: Mutex<Attempts>,
    pub(super) verification: Arc<tokio::sync::Semaphore>,
    connections: Mutex<HashMap<String, usize>>,
}
pub type ShareTickets = Arc<Admission>;
pub fn new_store() -> ShareTickets {
    Arc::new(Admission {
        tickets: Mutex::new(Tickets::default()),
        attempts: Mutex::new(Attempts::default()),
        verification: Arc::new(tokio::sync::Semaphore::new(4)),
        connections: Mutex::new(HashMap::new()),
    })
}

impl Admission {
    pub(super) fn has_room(&self, link: &str) -> bool {
        self.connections.lock().is_ok_and(|m| {
            m.values().sum::<usize>() < GLOBAL_CONNECTIONS
                && m.get(&digest(link)).copied().unwrap_or(0) < PER_LINK_CONNECTIONS
        })
    }
}
/// Held across attach and actor lifetime; failures and stopped actors release it.
pub(crate) struct ConnectionLease {
    state: std::sync::Weak<Admission>,
    key: String,
}
pub(super) fn reserve(store: &ShareTickets, link: &str) -> Option<ConnectionLease> {
    let key = digest(link);
    let mut connections = store.connections.lock().ok()?;
    if connections.values().sum::<usize>() >= GLOBAL_CONNECTIONS
        || connections.get(&key).copied().unwrap_or(0) >= PER_LINK_CONNECTIONS
    {
        return None;
    }
    *connections.entry(key.clone()).or_default() += 1;
    Some(ConnectionLease {
        state: Arc::downgrade(store),
        key,
    })
}
impl Drop for ConnectionLease {
    fn drop(&mut self) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        if let Ok(mut map) = state.connections.lock() {
            if let Some(count) = map.get_mut(&self.key) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    map.remove(&self.key);
                }
            }
        };
    }
}
fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
pub(super) fn valid_query(query: &str) -> bool {
    if query.len() > 128 || !query.is_ascii() || query.bytes().any(|b| b.is_ascii_control()) {
        return false;
    }
    if query.is_empty() {
        return true;
    }
    let mut keys = HashSet::new();
    query.split('&').all(|p| {
        let Some((key, value)) = p.split_once('=') else {
            return false;
        };
        matches!(key, "cols" | "rows")
            && keys.insert(key)
            && !value.is_empty()
            && value.bytes().all(|b| b.is_ascii_digit())
            && value.parse::<u16>().is_ok_and(|n| n > 0 && n <= 512)
    })
}
impl Tickets {
    #[cfg(test)]
    pub(super) fn pending_count(&self) -> usize {
        self.records.len()
    }
    pub(super) fn issue(
        &mut self,
        link: ShareLink,
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
                .filter(|t| t.link.token == link.token)
                .count()
                >= PER_LINK
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
                link,
                query,
                origin,
                expires: now + lifetime.min(TTL),
            },
        );
        Ok(token)
    }
    pub(super) fn consume(
        &mut self,
        ticket: &str,
        link: &str,
        query: &str,
        origin: Option<&str>,
        now: Instant,
    ) -> Option<ShareLink> {
        if ticket.len() != 64
            || !ticket
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        let record = self.records.remove(&digest(ticket))?;
        (record.expires > now
            && record.link.token == link
            && record.query == query
            && record.origin.as_deref() == origin)
            .then_some(record.link)
    }
}
impl Attempts {
    /// Independent link, peer and process buckets. Rejections never evict live buckets.
    pub(super) fn allow(&mut self, link: &str, peer: &str, now: Instant) -> bool {
        self.buckets
            .retain(|_, b| now.saturating_duration_since(b.start) < WINDOW);
        let checks = [
            ((0, digest(link)), LINK_RATE),
            ((1, peer.to_owned()), PEER_RATE),
            ((2, "process".into()), GLOBAL_RATE),
        ];
        let new_keys = checks
            .iter()
            .filter(|(key, _)| !self.buckets.contains_key(key))
            .count();
        if self.buckets.len() + new_keys > RATE_CAPACITY
            || checks
                .iter()
                .any(|(key, max)| self.buckets.get(key).is_some_and(|b| b.count >= *max))
        {
            return false;
        }
        for (key, _) in checks {
            self.buckets
                .entry(key)
                .or_insert(Bucket {
                    start: now,
                    count: 0,
                })
                .count += 1;
        }
        true
    }
}
/// Exactly one field; mismatched query/link/Origin consumes the opaque proof.
pub(super) fn redeem(
    req: &actix_web::HttpRequest,
    link: &str,
    store: &ShareTickets,
) -> Option<ShareLink> {
    let mut ticket = None;
    let mut parameters = Vec::new();
    for part in req.query_string().split('&') {
        if part.is_empty() {
            return None;
        }
        if let Some(value) = part.strip_prefix("ticket=") {
            if ticket.replace(value).is_some() {
                return None;
            }
        } else {
            parameters.push(part);
        }
    }
    let query = parameters.join("&");
    let origin = crate::terminal_tickets::origin(req).ok()?;
    let snapshot = store.tickets.lock().ok()?.consume(
        ticket?,
        link,
        &query,
        origin.as_deref(),
        Instant::now(),
    )?;
    valid_query(&query).then_some(snapshot)
}
#[cfg(test)]
#[path = "share_ticket_tests.rs"]
mod tests;
