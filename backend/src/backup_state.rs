//! Scheduled backups of NabiMan itself: a `state` schedule takes a consistent
//! snapshot of the data directory (nabiman_server::state_snapshot) into
//! data_dir/backups and keeps the newest few. A fresh install gets one such
//! schedule by default; an existing install never gets one added behind its back.
use super::backup_dir;
use super::schedule::{save, Schedule};
use nabiman_server::state_snapshot::{create, Options};
use std::path::Path;

/// Daily at 03:17 UTC: off the hour, when most cron work starts.
pub const DEFAULT_CRON: &str = "17 3 * * *";
/// A week of daily snapshots.
pub const DEFAULT_KEEP: u32 = 7;
/// `off` to start a fresh install without the default schedule, or a five-field UTC cron.
pub const SCHEDULE_ENV: &str = "NABIMAN_DEFAULT_BACKUP_SCHEDULE";
pub const KEEP_ENV: &str = "NABIMAN_DEFAULT_BACKUP_KEEP";

pub(super) fn validate(name: &str, keep: Option<u32>) -> Result<(), String> {
    if name.trim().is_empty() || name.len() > 120 || name.chars().any(char::is_control) {
        return Err("Invalid backup name".into());
    }
    if keep.is_some_and(|k| !(1..=365).contains(&k)) {
        return Err("Keep between 1 and 365 snapshots".into());
    }
    Ok(())
}

fn prefix(row: &Schedule) -> String {
    let id: String = row.id.chars().filter(char::is_ascii_alphanumeric).take(8).collect();
    format!("nabiman-state-{id}")
}

pub(super) fn run(row: &Schedule) -> Result<String, String> {
    let options = Options { include_secret_key: row.include_secret_key, mec_db: Some(crate::mec::db::default_path()) };
    let dir = backup_dir();
    let prefix = prefix(row);
    let name = create(&nabiman_server::data_dir(), &dir, &prefix, &options)?;
    prune(&dir, &prefix, row.keep.unwrap_or(DEFAULT_KEEP));
    Ok(name)
}

/// Keep the newest `keep` snapshots of one schedule (names sort by their UTC time).
pub(super) fn prune(dir: &Path, prefix: &str, keep: u32) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut mine: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(&format!("{prefix}-")) && n.ends_with(".tar.gz"))
        .collect();
    mine.sort_unstable_by(|a, b| b.cmp(a));
    for old in mine.into_iter().skip(keep.max(1) as usize) {
        let _ = std::fs::remove_file(dir.join(old));
    }
}

/// The schedule a fresh install starts with, unless switched off. `None`: none.
pub(super) fn default_schedule(setting: Option<&str>, keep: Option<&str>) -> Result<Option<Schedule>, String> {
    let cron = match setting.map(str::trim) {
        Some("off") | Some("none") | Some("") => return Ok(None),
        Some(expr) => expr.to_string(),
        None => DEFAULT_CRON.to_string(),
    };
    super::schedule::check_cron(&cron)?;
    let keep = match keep {
        Some(k) => Some(k.trim().parse::<u32>().ok().filter(|k| (1..=365).contains(k)).ok_or("Default backup keep must be 1 to 365")?),
        None => Some(DEFAULT_KEEP),
    };
    Ok(Some(Schedule {
        id: uuid::Uuid::new_v4().to_string(),
        name: "NabiMan 데이터 자동 백업".into(),
        paths: vec![],
        cron_expr: cron,
        compression: true,
        enabled: true,
        last_run: None,
        next_run: None,
        last_result: None,
        last_slot: None,
        kind: "state".into(),
        keep,
        include_secret_key: false,
    }))
}

/// At startup, only for a data directory that has never held users or schedules:
/// write the default schedule. Returns what it did, for the log.
pub fn seed_default_schedule(dir: &Path, fresh: bool) -> Option<String> {
    let file = dir.join("backup_schedules.json");
    if !fresh || file.exists() {
        return None;
    }
    let setting = std::env::var(SCHEDULE_ENV).ok();
    let keep = std::env::var(KEEP_ENV).ok();
    match default_schedule(setting.as_deref(), keep.as_deref()) {
        Ok(Some(row)) => {
            let note = format!("Default backup schedule added: data directory snapshot at '{}' UTC, keeping {}", row.cron_expr, row.keep.unwrap_or(DEFAULT_KEEP));
            save(&file, std::slice::from_ref(&row)).ok().map(|_| note)
        }
        Ok(None) => None,
        Err(error) => Some(format!("Default backup schedule not added: {error}")),
    }
}

#[cfg(test)]
#[path = "backup_state_tests.rs"]
mod tests;
