//! How the scheduled backups are doing, for `/metrics` and the self-checks.

use super::cron::Cron;
use super::{BackupScheduleStore, Schedule};
use crate::lock_safe::LockExt;
use chrono::{DateTime, Duration, Timelike, Utc};

fn succeeded_at(row: &Schedule) -> Option<DateTime<Utc>> {
    let at = row.last_run.as_deref()?;
    DateTime::parse_from_rfc3339(at).ok().map(|t| t.with_timezone(&Utc))
}

/// Unix time of the newest successful scheduled backup.
pub fn last_success(store: &BackupScheduleStore) -> Option<i64> {
    store.lock_safe().rows.iter().filter_map(succeeded_at).map(|t| t.timestamp()).max()
}

/// The time between the first two runs of `expr` after `from`, looked for
/// minute by minute up to two months ahead (`None`: rarer, or not a schedule).
pub(crate) fn period(expr: &str, from: DateTime<Utc>) -> Option<Duration> {
    let cron = Cron::parse(expr).ok()?;
    let start = from.with_second(0)?.with_nanosecond(0)?;
    let mut first = None;
    for minute in 1..=(62 * 24 * 60) {
        let t = start + Duration::minutes(minute);
        if cron.matches(t) {
            match first {
                None => first = Some(t),
                Some(f) => return Some(t - f),
            }
        }
    }
    None
}

/// Whether one schedule's last success is older than twice its period. One
/// that never succeeded is late once a run of it has finished without success.
pub(crate) fn late(row: &Schedule, now: DateTime<Utc>) -> bool {
    if !row.enabled {
        return false;
    }
    let Some(period) = period(&row.cron_expr, now) else { return false };
    match succeeded_at(row) {
        Some(at) => now - at > period * 2,
        None => row.last_result.as_ref().is_some_and(|r| r.finished_at.is_some() && r.state != "succeeded"),
    }
}

/// The names of the enabled schedules whose backups are late.
pub(crate) fn overdue(store: &BackupScheduleStore, now: DateTime<Utc>) -> Vec<String> {
    let rows = store.lock_safe().rows.clone();
    rows.iter().filter(|r| late(r, now)).map(|r| r.name.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cron: &str, last_run: Option<&str>) -> Schedule {
        serde_json::from_value(serde_json::json!({
            "id": "s", "name": "nightly", "paths": [], "cron_expr": cron, "compression": false,
            "enabled": true, "last_run": last_run,
        }))
        .unwrap()
    }

    #[test]
    fn a_backup_is_late_past_twice_its_schedule() {
        let now = DateTime::parse_from_rfc3339("2026-11-11T12:00:00Z").unwrap().with_timezone(&Utc);
        assert_eq!(period("17 3 * * *", now), Some(Duration::days(1)));
        assert_eq!(period("*/15 * * * *", now), Some(Duration::minutes(15)));
        assert!(!late(&row("17 3 * * *", Some("2026-11-10T03:17:30Z")), now));
        assert!(late(&row("17 3 * * *", Some("2026-11-09T03:17:30Z")), now));
        assert!(!late(&row("17 3 * * *", None), now), "never run yet: not late");
        let mut failed = row("17 3 * * *", None);
        failed.last_result = Some(super::super::RunResult {
            state: "failed".into(),
            started_at: "2026-11-11T03:17:00Z".into(),
            finished_at: Some("2026-11-11T03:18:00Z".into()),
            filename: None,
            error: Some("disk full".into()),
        });
        assert!(late(&failed, now));
        failed.enabled = false;
        assert!(!late(&failed, now));
    }
}
