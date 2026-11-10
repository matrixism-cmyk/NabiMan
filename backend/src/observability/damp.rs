//! Repeated identical failures, logged once: the first, then a reminder at
//! most every `every` with how many there were, then one line when it
//! recovers. A different failure starts over (it is news). The state also
//! answers "what is the last error" for diagnostics.

use serde::Serialize;
use std::time::{Duration, Instant};

/// What to log for this failure or success.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Say {
    /// A new failure (or a different one): log it.
    First,
    /// Still failing: log a reminder with the count since the first.
    Again { count: u64 },
    /// Still failing, reminded recently: say nothing.
    Quiet,
    /// It works again after `count` failures.
    Recovered { count: u64, lasted: Duration },
    /// It works, as before.
    Nothing,
}

struct Streak {
    message: String,
    since: Instant,
    since_wall: String,
    reminded: Instant,
    count: u64,
}

pub(crate) struct Repeats {
    every: Duration,
    streak: Option<Streak>,
    last_success: Option<String>,
}

/// The state for diagnostics: no paths or secrets beyond the error's (redacted) text.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub(crate) struct Status {
    pub failing: bool,
    pub last_error: Option<String>,
    pub failing_since: Option<String>,
    pub consecutive_failures: u64,
    pub last_success: Option<String>,
}

fn wall() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl Repeats {
    pub(crate) fn new(every: Duration) -> Self {
        Self { every, streak: None, last_success: None }
    }

    pub(crate) fn failure(&mut self, message: &str, now: Instant) -> Say {
        match &mut self.streak {
            Some(s) if s.message == message => {
                s.count += 1;
                if now.duration_since(s.reminded) >= self.every {
                    s.reminded = now;
                    Say::Again { count: s.count }
                } else {
                    Say::Quiet
                }
            }
            _ => {
                self.streak = Some(Streak { message: message.into(), since: now, since_wall: wall(), reminded: now, count: 1 });
                Say::First
            }
        }
    }

    pub(crate) fn success(&mut self, now: Instant) -> Say {
        self.last_success = Some(wall());
        match self.streak.take() {
            Some(s) => Say::Recovered { count: s.count, lasted: now.duration_since(s.since) },
            None => Say::Nothing,
        }
    }

    /// Log `outcome` of one try at `what`, damped (target `target`).
    pub(crate) fn report(&mut self, target: &str, what: &str, outcome: Result<(), String>) {
        let now = Instant::now();
        match outcome {
            Ok(()) => {
                if let Say::Recovered { count, lasted } = self.success(now) {
                    log_info!(target, format!("{what} recovered"), failures = count, failed_for_secs = lasted.as_secs());
                }
            }
            Err(error) => match self.failure(&error, now) {
                Say::First => log_warn!(
                    target,
                    format!("{what} failed; while it fails the same way, a reminder follows every {} min", self.every.as_secs() / 60),
                    error = error
                ),
                Say::Again { count } => log_warn!(target, format!("{what} still failing"), failures = count, error = error),
                Say::Quiet | Say::Recovered { .. } | Say::Nothing => {}
            },
        }
    }

    pub(crate) fn status(&self) -> Status {
        Status {
            failing: self.streak.is_some(),
            last_error: self.streak.as_ref().map(|s| super::redact::text(&s.message)),
            failing_since: self.streak.as_ref().map(|s| s.since_wall.clone()),
            consecutive_failures: self.streak.as_ref().map_or(0, |s| s.count),
            last_success: self.last_success.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_failure_is_logged_once_then_reminded_then_recovered() {
        let mut r = Repeats::new(Duration::from_secs(600));
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        assert_eq!(r.failure("nodes is forbidden", at(0)), Say::First);
        for s in (10..600).step_by(10) {
            assert_eq!(r.failure("nodes is forbidden", at(s)), Say::Quiet);
        }
        assert_eq!(r.failure("nodes is forbidden", at(600)), Say::Again { count: 61 });
        assert_eq!(r.failure("nodes is forbidden", at(610)), Say::Quiet);
        let status = r.status();
        assert!(status.failing && status.consecutive_failures == 62);
        assert_eq!(status.last_error.as_deref(), Some("nodes is forbidden"));
        assert_eq!(r.failure("metrics-server down", at(620)), Say::First, "a different failure is news");
        assert_eq!(r.success(at(630)), Say::Recovered { count: 1, lasted: Duration::from_secs(10) });
        assert_eq!(r.success(at(640)), Say::Nothing);
        let status = r.status();
        assert!(!status.failing && status.last_error.is_none() && status.last_success.is_some());
    }

    #[test]
    fn the_last_error_is_shown_redacted() {
        let mut r = Repeats::new(Duration::from_secs(60));
        r.failure("401 for token=abcdef123", Instant::now());
        assert!(!r.status().last_error.unwrap().contains("abcdef123"));
    }
}
