use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const CAPACITY: usize = 4096;
const MAX_IN_FLIGHT: usize = 16;
/// Every attempt, from anywhere: protects the bcrypt workers.
const GLOBAL: (usize, u64) = (300, 60);
/// Attempts from one client address (the real one, behind a trusted proxy).
const SOURCE: (usize, u64) = (60, 60);
/// Failures for one account from one address: the hard limit.
const PAIR: (usize, u64) = (5, 300);
/// Failures for one account from everywhere: free up to this many in the
/// window, then each further attempt waits twice as long as the last, up to
/// `BACKOFF_CAP`. It slows a spread-out guesser without ever shutting the
/// account's owner out for longer than that.
const ACCOUNT_FREE: usize = 10;
const ACCOUNT_WINDOW: u64 = 900;
const BACKOFF_CAP: u64 = 60;
/// A source that signed in to an account is remembered this long: the
/// account-wide backoff does not apply to it, and nor does the global cap.
const KNOWN_GOOD: Duration = Duration::from_secs(30 * 24 * 3600);
type Key = [u8; 32];
#[derive(Default)]
pub(super) struct State {
    buckets: HashMap<Key, Bucket>,
    /// Hashes of (account, source) and of source that recently succeeded,
    /// with when that stops counting.
    good: HashMap<Key, Instant>,
    in_flight: usize,
    sequence: u64,
}
struct Bucket {
    window: Duration,
    entries: Vec<(u64, Instant)>,
}
pub(super) type Shared = Arc<Mutex<State>>;
fn key(kind: &str, value: &str) -> Key {
    Sha256::digest(format!("{kind}\0{value}").as_bytes()).into()
}
fn seconds(d: Duration) -> u64 {
    d.as_secs() + u64::from(d.subsec_nanos() > 0)
}
impl State {
    fn prune(&mut self, now: Instant) {
        self.buckets.retain(|_, b| {
            b.entries.retain(|(_, at)| now.duration_since(*at) < b.window);
            !b.entries.is_empty()
        });
        self.good.retain(|_, until| *until > now);
    }
    fn check(&self, key: Key, (limit, window): (usize, u64), now: Instant) -> Result<(), u64> {
        if let Some(b) = self.buckets.get(&key) {
            if b.entries.len() >= limit {
                let remaining = Duration::from_secs(window).saturating_sub(now.duration_since(b.entries[0].1));
                return Err(seconds(remaining));
            }
        } else if self.buckets.len() >= CAPACITY {
            return Err(60);
        }
        Ok(())
    }
    /// The account-wide backoff: nothing until `ACCOUNT_FREE` failures, then
    /// 1 s, 2 s, 4 s … since the latest attempt, at most `BACKOFF_CAP`.
    fn backoff(&self, key: Key, now: Instant) -> Result<(), u64> {
        let Some(b) = self.buckets.get(&key) else {
            return if self.buckets.len() >= CAPACITY { Err(60) } else { Ok(()) };
        };
        let n = b.entries.len();
        if n < ACCOUNT_FREE {
            return Ok(());
        }
        let wait = Duration::from_secs(1u64 << (n - ACCOUNT_FREE).min(6)).min(Duration::from_secs(BACKOFF_CAP));
        let last = b.entries.iter().map(|(_, at)| *at).max().unwrap_or(now);
        match wait.checked_sub(now.duration_since(last)) {
            Some(left) if !left.is_zero() => Err(seconds(left)),
            _ => Ok(()),
        }
    }
    fn record(&mut self, key: Key, window: u64, id: u64, now: Instant) {
        self.buckets
            .entry(key)
            .or_insert_with(|| Bucket { window: Duration::from_secs(window), entries: vec![] })
            .entries
            .push((id, now));
    }
    fn forget(&mut self, key: Key, id: u64) {
        if let Some(b) = self.buckets.get_mut(&key) {
            b.entries.retain(|(i, _)| *i != id);
            if b.entries.is_empty() {
                self.buckets.remove(&key);
            }
        }
    }
    fn remember_good(&mut self, key: Key, now: Instant) {
        if self.good.len() >= CAPACITY && !self.good.contains_key(&key) {
            // Forgetting a good source only puts it back under the normal rules.
            if let Some(oldest) = self.good.iter().min_by_key(|(_, until)| **until).map(|(k, _)| *k) {
                self.good.remove(&oldest);
            }
        }
        self.good.insert(key, now + KNOWN_GOOD);
    }
}
/// Reservation counts before asynchronous work. Cancellation retains attempt budgets,
/// but releases the concurrent-work permit. No raw usernames or IP strings are retained.
pub(super) struct Permit {
    state: Shared,
    id: u64,
    source: String,
    /// (pair, account, known-good) keys of the account this attempt is for.
    account: Option<(Key, Key, Key)>,
}
impl Permit {
    pub fn begin(state: &Shared, source: &str, now: Instant) -> Result<Self, u64> {
        let mut s = state.lock().map_err(|_| 60u64)?;
        s.prune(now);
        if s.in_flight >= MAX_IN_FLIGHT {
            return Err(1);
        }
        let global = key("global", "");
        let src = key("source", source);
        if !s.good.contains_key(&key("good-source", source)) {
            s.check(global, GLOBAL, now)?;
        }
        s.check(src, SOURCE, now)?;
        // Both new buckets must fit before either is published.
        let needed = usize::from(!s.buckets.contains_key(&global)) + usize::from(!s.buckets.contains_key(&src));
        if s.buckets.len() + needed > CAPACITY {
            return Err(60);
        }
        s.sequence = s.sequence.checked_add(1).ok_or(60u64)?;
        let id = s.sequence;
        s.record(global, GLOBAL.1, id, now);
        s.record(src, SOURCE.1, id, now);
        s.in_flight += 1;
        Ok(Self { state: state.clone(), id, source: source.to_owned(), account: None })
    }
    pub fn account(&mut self, category: &str, identity: &str, now: Instant) -> Result<(), u64> {
        let mut s = self.state.lock().map_err(|_| 60u64)?;
        s.prune(now);
        let pair = key(&format!("{category}\0pair"), &format!("{identity}\0{}", self.source));
        let account = key(category, identity);
        let good = key(&format!("{category}\0good"), &format!("{identity}\0{}", self.source));
        s.check(pair, PAIR, now)?;
        if !s.good.contains_key(&good) {
            s.backoff(account, now)?;
        }
        let needed = usize::from(!s.buckets.contains_key(&pair)) + usize::from(!s.buckets.contains_key(&account));
        if s.buckets.len() + needed > CAPACITY {
            return Err(60);
        }
        s.record(pair, PAIR.1, self.id, now);
        s.record(account, ACCOUNT_WINDOW, self.id, now);
        self.account = Some((pair, account, good));
        Ok(())
    }
    /// The attempt succeeded: it was not a failure, and its source is now
    /// known to this account.
    pub fn success(&self) {
        let Some((pair, account, good)) = self.account else { return };
        if let Ok(mut s) = self.state.lock() {
            s.forget(pair, self.id);
            s.forget(account, self.id);
            let now = Instant::now();
            s.remember_good(good, now);
            s.remember_good(key("good-source", &self.source), now);
        }
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        if let Ok(mut s) = self.state.lock() {
            s.in_flight = s.in_flight.saturating_sub(1);
        }
    }
}
#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
