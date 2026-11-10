//! NabiMan watching itself: a few built-in checks whose failures go out
//! through the notification channels the alert rules use. Each check
//! notifies when it starts failing, reminds at most every
//! `NABIMAN_SELF_CHECK_REPEAT_HOURS` (6) while it keeps failing, and says
//! when it recovers. `NABIMAN_SELF_CHECKS`: `on` (the default), `off`, or
//! the checks to run (`disk,audit,backup,tmux,timeouts`).

use super::health::Tmux;
use crate::install_config::values::{self, Lookup};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Free space on the data directory's filesystem below which `disk` fails.
const DISK_FREE_MIN_PERCENT: f64 = 10.0;
/// `timeouts` fails at this many child-process timeouts within `TIMEOUT_WINDOW`.
const TIMEOUTS_MAX: u64 = 5;
const TIMEOUT_WINDOW: Duration = Duration::from_secs(15 * 60);
const EVERY: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Check {
    Disk,
    Audit,
    Backup,
    Tmux,
    Timeouts,
}

const ALL: [Check; 5] = [Check::Disk, Check::Audit, Check::Backup, Check::Tmux, Check::Timeouts];

impl Check {
    fn name(self) -> &'static str {
        match self {
            Check::Disk => "disk",
            Check::Audit => "audit",
            Check::Backup => "backup",
            Check::Tmux => "tmux",
            Check::Timeouts => "timeouts",
        }
    }
}

/// What the checks look at, gathered once a minute (or made up in tests).
#[derive(Clone, Debug, Default)]
pub(crate) struct Facts {
    pub disk_free_percent: Option<f64>,
    pub audit_ok: bool,
    pub audit_failures_total: u64,
    pub backups_overdue: Vec<String>,
    pub tmux: Option<Tmux>,
    pub registered_sessions: usize,
    pub child_timeouts_total: u64,
}

/// Which checks run, from `NABIMAN_SELF_CHECKS`.
pub(crate) fn enabled_in(get: Lookup) -> Vec<Check> {
    match values::text_in(get, "NABIMAN_SELF_CHECKS").map(|v| v.to_ascii_lowercase()) {
        None => ALL.to_vec(),
        Some(v) if matches!(v.as_str(), "on" | "true" | "1" | "all") => ALL.to_vec(),
        Some(v) if matches!(v.as_str(), "off" | "false" | "0" | "none") => vec![],
        Some(_) => {
            let named = values::list_in(get, "NABIMAN_SELF_CHECKS");
            ALL.into_iter().filter(|c| named.iter().any(|n| n.eq_ignore_ascii_case(c.name()))).collect()
        }
    }
}

pub(crate) struct SelfChecks {
    enabled: Vec<Check>,
    repeat: Duration,
    audit_failures_seen: Option<u64>,
    timeouts: VecDeque<(Instant, u64)>,
    /// Failing checks: when the last notice went out.
    firing: HashMap<Check, Instant>,
}

/// A notice to send: subject and body.
pub(crate) type Notice = (String, String);

impl SelfChecks {
    pub(crate) fn new(enabled: Vec<Check>, repeat: Duration) -> Self {
        Self { enabled, repeat, audit_failures_seen: None, timeouts: VecDeque::new(), firing: HashMap::new() }
    }

    /// Why `check` fails now, if it does.
    fn problem(&mut self, check: Check, f: &Facts, now: Instant) -> Option<String> {
        match check {
            Check::Disk => f
                .disk_free_percent
                .filter(|free| *free < DISK_FREE_MIN_PERCENT)
                .map(|free| format!("The data directory's disk has {free:.1}% free (below {DISK_FREE_MIN_PERCENT}%).")),
            Check::Audit => {
                let new = self.audit_failures_seen.map_or(0, |seen| f.audit_failures_total.saturating_sub(seen));
                self.audit_failures_seen = Some(f.audit_failures_total);
                (!f.audit_ok || new > 0).then(|| {
                    format!("The local audit journal is not recording ({new} failed writes since the last check). Inspect its storage.")
                })
            }
            Check::Backup => (!f.backups_overdue.is_empty()).then(|| {
                format!("Scheduled backups older than twice their schedule: {}.", f.backups_overdue.join(", "))
            }),
            Check::Tmux => {
                let down = match f.tmux {
                    Some(Tmux::Unreachable) => true,
                    Some(Tmux::NoServer) => f.registered_sessions > 0,
                    _ => false,
                };
                down.then(|| "The terminal's tmux server does not answer; terminal sessions may be unusable.".to_string())
            }
            Check::Timeouts => {
                self.timeouts.push_back((now, f.child_timeouts_total));
                while self.timeouts.front().is_some_and(|(at, _)| now.duration_since(*at) > TIMEOUT_WINDOW) {
                    self.timeouts.pop_front();
                }
                let oldest = self.timeouts.front().map_or(f.child_timeouts_total, |(_, n)| *n);
                let recent = f.child_timeouts_total.saturating_sub(oldest);
                (recent >= TIMEOUTS_MAX).then(|| {
                    format!("{recent} child processes were stopped at their deadline in the last {} minutes.", TIMEOUT_WINDOW.as_secs() / 60)
                })
            }
        }
    }

    /// The notices this round of facts calls for.
    pub(crate) fn tick(&mut self, f: &Facts, now: Instant) -> Vec<Notice> {
        let mut out = Vec::new();
        for check in self.enabled.clone() {
            let name = check.name();
            match (self.problem(check, f, now), self.firing.get(&check).copied()) {
                (Some(why), None) => {
                    self.firing.insert(check, now);
                    out.push((format!("[NabiMan] Self-check failed: {name}"), why));
                }
                (Some(why), Some(sent)) if now.duration_since(sent) >= self.repeat => {
                    self.firing.insert(check, now);
                    out.push((format!("[NabiMan] Self-check still failing: {name}"), why));
                }
                (None, Some(_)) => {
                    self.firing.remove(&check);
                    out.push((format!("[NabiMan] Self-check recovered: {name}"), format!("The {name} check passes again.")));
                }
                _ => {}
            }
        }
        out
    }
}

/// Free space on the filesystem holding `dir`, in percent.
pub(crate) fn free_percent(dir: &std::path::Path) -> Option<f64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut st) } != 0 || st.f_blocks == 0 {
        return None;
    }
    Some(st.f_bavail as f64 / st.f_blocks as f64 * 100.0)
}

/// What the running server's checks read.
pub(crate) struct Sources {
    pub channels: crate::notifications::ChannelStore,
    pub audit: crate::audit::AuditLog,
    pub backups: crate::backup::BackupScheduleStore,
    pub sessions: crate::terminal::PtyStore,
}

fn gather(s: &Sources, enabled: &[Check]) -> Facts {
    use crate::lock_safe::LockExt;
    use std::sync::atomic::Ordering::Relaxed;
    Facts {
        disk_free_percent: free_percent(&nabiman_server::data_dir()),
        audit_ok: s.audit.lock_safe().error.is_none(),
        audit_failures_total: super::metrics::AUDIT_WRITE_FAILURES.load(Relaxed),
        backups_overdue: crate::backup::overdue(&s.backups, chrono::Utc::now()),
        // Only probed when the check runs (it starts a tmux client).
        tmux: enabled.contains(&Check::Tmux).then(super::health::tmux_state),
        registered_sessions: s.sessions.lock_safe().len(),
        child_timeouts_total: super::metrics::CHILD_TIMEOUTS.load(Relaxed),
    }
}

/// Start the checks on their own thread (nothing, when all are off).
pub(crate) fn start(sources: Sources) {
    let enabled = enabled_in(&values::env);
    if enabled.is_empty() {
        log_info!("selfcheck", "self-checks are off (NABIMAN_SELF_CHECKS)");
        return;
    }
    let repeat = Duration::from_secs(values::number::<u64>("NABIMAN_SELF_CHECK_REPEAT_HOURS", 6).max(1) * 3600);
    let names: Vec<&str> = enabled.iter().map(|c| c.name()).collect();
    log_info!("selfcheck", "self-checks running", checks = names.join(","), repeat_hours = repeat.as_secs() / 3600);
    std::thread::spawn(move || {
        let mut checks = SelfChecks::new(enabled.clone(), repeat);
        loop {
            std::thread::sleep(EVERY);
            let facts = gather(&sources, &enabled);
            for (subject, body) in checks.tick(&facts, Instant::now()) {
                log_warn!("selfcheck", subject.clone(), detail = body);
                let channels = { use crate::lock_safe::LockExt; sources.channels.lock_safe().clone() };
                crate::notifications::send_notification(&channels, &subject, &body);
            }
        }
    });
}

#[cfg(test)]
#[path = "self_check_tests.rs"]
mod tests;
