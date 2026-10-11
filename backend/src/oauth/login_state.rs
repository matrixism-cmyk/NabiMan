//! Short-lived, single-use OAuth requests bound to the initiating browser.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const COOKIE: &str = "nabiman_oauth_browser";
const TTL: Duration = Duration::from_secs(600);
const LIMIT: usize = 1024;

#[derive(Default)]
struct Pending(HashMap<String, (String, Instant)>);

impl Pending {
    fn issue(&mut self, now: Instant) -> Result<(String, String), &'static str> {
        self.0.retain(|_, (_, expires)| *expires > now);
        if self.0.len() >= LIMIT {
            return Err("Too many pending OAuth logins; try again later");
        }
        let state = super::oidc::generate_state();
        let browser = super::oidc::generate_state();
        self.0.insert(state.clone(), (browser.clone(), now + TTL));
        Ok((state, browser))
    }

    fn consume(&mut self, state: &str, browser: &str, now: Instant) -> bool {
        self.0.retain(|_, (_, expires)| *expires > now);
        if !self
            .0
            .get(state)
            .is_some_and(|(expected, _)| expected == browser)
        {
            return false;
        }
        self.0.remove(state);
        true
    }
}

fn store() -> &'static Mutex<Pending> {
    static STORE: OnceLock<Mutex<Pending>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Pending::default()))
}

pub fn issue() -> Result<(String, String), &'static str> {
    store()
        .lock()
        .map_err(|_| "OAuth login state unavailable")?
        .issue(Instant::now())
}

pub fn consume(state: &str, browser: &str) -> bool {
    store()
        .lock()
        .map(|mut pending| pending.consume(state, browser, Instant::now()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_is_bound_to_browser_and_single_use() {
        let mut pending = Pending::default();
        let now = Instant::now();
        let (state, browser) = pending.issue(now).unwrap();
        assert!(!pending.consume(&state, "another-browser", now));
        assert!(!pending.consume("unknown", &browser, now));
        assert!(pending.consume(&state, &browser, now));
        assert!(!pending.consume(&state, &browser, now));
    }

    #[test]
    fn expired_requests_are_rejected_and_capacity_is_bounded() {
        let mut pending = Pending::default();
        let now = Instant::now();
        let (state, browser) = pending.issue(now).unwrap();
        assert!(!pending.consume(&state, &browser, now + TTL));
        for _ in 0..LIMIT {
            pending.issue(now).unwrap();
        }
        assert!(pending.issue(now).is_err());
        assert!(pending.issue(now + TTL).is_ok());
    }
}
