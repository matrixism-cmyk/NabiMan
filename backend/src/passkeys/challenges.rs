//! Challenges handed out and not yet answered: in memory, for a few
//! minutes, a bounded number of them, and each taken at most once — the
//! answer to a challenge, right or wrong, uses it up.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use webauthn_rs::prelude::{DiscoverableAuthentication, PasskeyRegistration};

/// How long a challenge may be answered (the browser prompt's own timeout).
pub(super) const LIFETIME: Duration = Duration::from_secs(300);
/// More than the rate limiter lets anyone ask for within `LIFETIME`.
const CAPACITY: usize = 2048;

pub(super) enum Pending {
    /// A sign-in: whose it is, is not known until it is answered.
    Login(DiscoverableAuthentication),
    /// A registration, for the account that asked and the password it gave.
    Register { user_id: String, password_hash: String, state: PasskeyRegistration },
}

#[derive(Default)]
pub(super) struct Challenges {
    pending: Mutex<HashMap<String, (Instant, Pending)>>,
}

impl Challenges {
    /// Keeps `pending` and gives the id to answer it with; `None` when full.
    pub(super) fn put(&self, pending: Pending) -> Option<String> {
        self.put_at(pending, Instant::now())
    }

    pub(super) fn put_at(&self, pending: Pending, now: Instant) -> Option<String> {
        use rand::RngCore;
        let mut map = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        map.retain(|_, (expires, _)| *expires > now);
        if map.len() >= CAPACITY {
            return None;
        }
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        let id = hex::encode(bytes);
        map.insert(id.clone(), (now + LIFETIME, pending));
        Some(id)
    }

    /// The challenge `id`, removed: a second answer finds nothing.
    pub(super) fn take(&self, id: &str) -> Option<Pending> {
        self.take_at(id, Instant::now())
    }

    pub(super) fn take_at(&self, id: &str, now: Instant) -> Option<Pending> {
        let mut map = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let (expires, pending) = map.remove(id)?;
        (expires > now).then_some(pending)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.pending.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login() -> Pending {
        let site = super::super::site::engine(Some("https://man.example.com"), "NabiMan").unwrap();
        Pending::Login(site.start_discoverable_authentication().unwrap().1)
    }

    #[test]
    fn a_challenge_is_answered_once_and_only_in_time() {
        let challenges = Challenges::default();
        let start = Instant::now();
        let id = challenges.put_at(login(), start).unwrap();
        assert_eq!(id.len(), 32);
        assert!(challenges.take_at("0000", start).is_none());
        assert!(challenges.take_at(&id, start).is_some());
        assert!(challenges.take_at(&id, start).is_none(), "used up");

        let late = challenges.put_at(login(), start).unwrap();
        assert!(challenges.take_at(&late, start + LIFETIME).is_none(), "expired");
        assert_eq!(challenges.len(), 0, "an expired one is dropped when asked for");
    }

    #[test]
    fn the_store_is_bounded_and_drops_what_expired() {
        let challenges = Challenges::default();
        let start = Instant::now();
        let pending = login();
        let Pending::Login(state) = &pending else { unreachable!() };
        for _ in 0..CAPACITY {
            assert!(challenges.put_at(Pending::Login(state.clone()), start).is_some());
        }
        assert!(challenges.put_at(Pending::Login(state.clone()), start).is_none(), "full");
        assert!(challenges.put_at(Pending::Login(state.clone()), start + LIFETIME).is_some(), "the old ones expired");
        assert_eq!(challenges.len(), 1);
    }
}
