use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const CAPACITY: usize = 4096;
const MAX_IN_FLIGHT: usize = 16;
type Key = [u8; 32];
#[derive(Default)]
pub(super) struct State {
    buckets: HashMap<Key, Bucket>,
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
impl State {
    fn prune(&mut self, now: Instant) {
        self.buckets.retain(|_, b| {
            b.entries
                .retain(|(_, at)| now.duration_since(*at) < b.window);
            !b.entries.is_empty()
        });
    }
    fn check(&self, key: Key, limit: usize, window: u64, now: Instant) -> Result<(), u64> {
        if let Some(b) = self.buckets.get(&key) {
            if b.entries.len() >= limit {
                let remaining =
                    Duration::from_secs(window).saturating_sub(now.duration_since(b.entries[0].1));
                return Err(remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0));
            }
        } else if self.buckets.len() >= CAPACITY {
            return Err(60);
        }
        Ok(())
    }
    fn record(&mut self, key: Key, window: u64, id: u64, now: Instant) {
        self.buckets
            .entry(key)
            .or_insert_with(|| Bucket {
                window: Duration::from_secs(window),
                entries: vec![],
            })
            .entries
            .push((id, now));
    }
}
/// Reservation counts before asynchronous work. Cancellation retains attempt budgets,
/// but releases the concurrent-work permit. No raw usernames or IP strings are retained.
pub(super) struct Permit {
    state: Shared,
    id: u64,
    account: Option<Key>,
}
impl Permit {
    pub fn begin(state: &Shared, peer: &str, now: Instant) -> Result<Self, u64> {
        let mut s = state.lock().map_err(|_| 60u64)?;
        s.prune(now);
        if s.in_flight >= MAX_IN_FLIGHT {
            return Err(1);
        }
        let global = key("global", "");
        let source = key("source", peer);
        s.check(global, 300, 60, now)?;
        s.check(source, 60, 60, now)?;
        // Both new buckets must fit before either is published.
        let needed = usize::from(!s.buckets.contains_key(&global))
            + usize::from(!s.buckets.contains_key(&source));
        if s.buckets.len() + needed > CAPACITY {
            return Err(60);
        }
        s.sequence = s.sequence.checked_add(1).ok_or(60u64)?;
        let id = s.sequence;
        s.record(global, 60, id, now);
        s.record(source, 60, id, now);
        s.in_flight += 1;
        Ok(Self {
            state: state.clone(),
            id,
            account: None,
        })
    }
    pub fn account(&mut self, category: &str, identity: &str, now: Instant) -> Result<(), u64> {
        let mut s = self.state.lock().map_err(|_| 60u64)?;
        s.prune(now);
        let k = key(category, identity);
        s.check(k, 5, 300, now)?;
        s.record(k, 300, self.id, now);
        self.account = Some(k);
        Ok(())
    }
    pub fn success(&self) {
        if let (Some(k), Ok(mut s)) = (self.account, self.state.lock()) {
            if let Some(b) = s.buckets.get_mut(&k) {
                b.entries.retain(|(id, _)| *id != self.id);
            }
            if s.buckets.get(&k).is_some_and(|b| b.entries.is_empty()) {
                s.buckets.remove(&k);
            }
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
mod tests {
    use super::*;
    #[test]
    fn failures_are_account_bound_across_sources_and_expire() {
        let s = Shared::default();
        let now = Instant::now();
        for i in 0..5 {
            let mut p = Permit::begin(&s, &i.to_string(), now).unwrap();
            p.account("login", "a", now).unwrap();
        }
        let mut p = Permit::begin(&s, "new", now).unwrap();
        assert_eq!(p.account("login", "a", now).unwrap_err(), 300);
        p.account("login", "b", now).unwrap();
        p.success();
        drop(p);
        let mut p = Permit::begin(&s, "new", now + Duration::from_secs(300)).unwrap();
        p.account("login", "a", now + Duration::from_secs(300))
            .unwrap();
    }
    #[test]
    fn simultaneous_reservations_and_cancellation_never_bypass_limits() {
        let s = Shared::default();
        let now = Instant::now();
        let mut active = vec![];
        for _ in 0..MAX_IN_FLIGHT {
            active.push(Permit::begin(&s, "peer", now).unwrap());
        }
        assert!(Permit::begin(&s, "other", now).is_err());
        active.clear();
        assert_eq!(s.lock().unwrap().in_flight, 0);
        for _ in MAX_IN_FLIGHT..60 {
            drop(Permit::begin(&s, "peer", now).unwrap());
        }
        assert!(Permit::begin(&s, "peer", now).is_err());
    }
    #[test]
    fn successful_attempt_only_removes_its_own_account_reservation() {
        let s = Shared::default();
        let now = Instant::now();
        let mut failed = Permit::begin(&s, "p", now).unwrap();
        failed.account("login", "u", now).unwrap();
        drop(failed);
        let mut good = Permit::begin(&s, "p", now).unwrap();
        good.account("login", "u", now).unwrap();
        good.success();
        drop(good);
        assert_eq!(
            s.lock().unwrap().buckets[&key("login", "u")].entries.len(),
            1
        );
        assert_eq!(
            s.lock().unwrap().buckets[&key("source", "p")].entries.len(),
            2
        );
    }
    #[test]
    fn full_storage_does_not_evict_unexpired_failures() {
        let s = Shared::default();
        let now = Instant::now();
        for i in 0..CAPACITY {
            s.lock()
                .unwrap()
                .record(key("fixture", &i.to_string()), 300, i as u64, now);
        }
        assert!(Permit::begin(&s, "p", now).is_err());
        assert_eq!(s.lock().unwrap().buckets.len(), CAPACITY);
        assert!(Permit::begin(&s, "p", now + Duration::from_secs(300)).is_ok());
    }
}
