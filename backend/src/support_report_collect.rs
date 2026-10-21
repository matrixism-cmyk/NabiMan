use super::{Section, State};
use serde::Serialize;
use std::{io::Read, path::Path};
/// A file's bytes, up to `limit`; never a raw error.
pub(crate) fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>, State> {
    let file = std::fs::File::open(path).map_err(|_| State::Unavailable)?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| State::Unavailable)?;
    if bytes.len() as u64 > limit {
        return Err(State::TooLarge);
    }
    Ok(bytes)
}
fn kib(bytes: &[u8], name: &str) -> Option<u64> {
    std::str::from_utf8(bytes).ok()?.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key != name {
            return None;
        }
        let mut parts = value.split_whitespace();
        let number = parts.next()?.parse().ok()?;
        (parts.next() == Some("kB") && parts.next().is_none()).then_some(number)
    })
}
#[derive(Serialize)]
pub(super) struct Process {
    os: &'static str,
    architecture: &'static str,
    runs_as_root: bool,
    rss_kib: Option<u64>,
    host_memory_total_kib: Option<u64>,
    host_memory_available_kib: Option<u64>,
}
pub(super) fn process() -> Process {
    let status = bounded(Path::new("/proc/self/status"), 65536).unwrap_or_default();
    let memory = bounded(Path::new("/proc/meminfo"), 65536).unwrap_or_default();
    Process {
        os: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        runs_as_root: unsafe { libc::geteuid() } == 0,
        rss_kib: kib(&status, "VmRSS"),
        host_memory_total_kib: kib(&memory, "MemTotal"),
        host_memory_available_kib: kib(&memory, "MemAvailable"),
    }
}
#[derive(Serialize, Default)]
pub(super) struct Accounts {
    total: usize,
    active: usize,
    pending: usize,
    rejected: usize,
    suspended: usize,
    active_mfa_configured: usize,
}
pub(super) fn accounts(store: &crate::users::UserStore) -> Section<Accounts> {
    let Ok(users) = store.try_lock() else {
        return Section::missing(State::Busy);
    };
    let mut counts = Accounts {
        total: users.len(),
        ..Default::default()
    };
    for user in users.iter() {
        match user.status {
            crate::models::AccountStatus::Active => counts.active += 1,
            crate::models::AccountStatus::Pending => counts.pending += 1,
            crate::models::AccountStatus::Rejected => counts.rejected += 1,
            crate::models::AccountStatus::Suspended => counts.suspended += 1,
        }
        if user.active() && user.totp_enabled {
            counts.active_mfa_configured += 1;
        }
    }
    Section::ready(counts)
}
#[derive(Serialize)]
pub(super) struct Logins {
    registered: usize,
    mfa_verified: usize,
}
pub(super) fn logins(store: &crate::jwt_sessions::SessionStore) -> Section<Logins> {
    let Ok(sessions) = store.try_lock() else {
        return Section::missing(State::Busy);
    };
    Section::ready(Logins {
        registered: sessions.len(),
        mfa_verified: sessions.values().filter(|s| s.mfa_verified).count(),
    })
}
#[derive(Serialize)]
pub(super) struct Storage {
    capacity_bytes: u64,
    available_bytes: u64,
    directory_private: bool,
}
pub(super) fn storage() -> Section<Storage> {
    use std::os::unix::fs::MetadataExt;
    let path = nabiman_server::data_dir();
    let Ok(metadata) = std::fs::metadata(&path) else {
        return Section::missing(State::Unavailable);
    };
    let Ok(stats) = nix::sys::statvfs::statvfs(&path) else {
        return Section::missing(State::Unavailable);
    };
    Section::ready(Storage {
        capacity_bytes: stats.blocks().saturating_mul(stats.fragment_size()),
        available_bytes: stats
            .blocks_available()
            .saturating_mul(stats.fragment_size()),
        directory_private: metadata.is_dir() && (metadata.mode() & 0o077) == 0,
    })
}
#[derive(Serialize, Default)]
pub(super) struct Jobs {
    sampled: usize,
    older_jobs_excluded: bool,
    pending: usize,
    running: usize,
    completed: usize,
    failed: usize,
    cancelled: usize,
    unknown_result: usize,
    other: usize,
}
pub(super) fn jobs(db: &crate::mec::db::DbHandle) -> Section<Jobs> {
    let Ok(connection) = db.try_lock() else {
        return Section::missing(State::Busy);
    };
    let query = || -> rusqlite::Result<Jobs> {
        // Never return the raw JSON status, which can include credentials in error strings.
        let mut statement=connection.prepare("SELECT CASE WHEN length(status)>4096 THEN 'other' WHEN json_valid(status) THEN CASE json_extract(status,'$.state') WHEN 'pending' THEN 'pending' WHEN 'running' THEN 'running' WHEN 'completed' THEN 'completed' WHEN 'failed' THEN 'failed' WHEN 'cancelled' THEN 'cancelled' WHEN 'unknown_result' THEN 'unknown_result' ELSE 'other' END ELSE 'other' END FROM jobs ORDER BY rowid DESC LIMIT 1001")?;
        let mut rows = statement.query([])?;
        let mut counts = Jobs::default();
        while let Some(row) = rows.next()? {
            if counts.sampled == 1000 {
                counts.older_jobs_excluded = true;
                break;
            }
            counts.sampled += 1;
            match row.get::<_, String>(0)?.as_str() {
                "pending" => counts.pending += 1,
                "running" => counts.running += 1,
                "completed" => counts.completed += 1,
                "failed" => counts.failed += 1,
                "cancelled" => counts.cancelled += 1,
                "unknown_result" => counts.unknown_result += 1,
                _ => counts.other += 1,
            }
        }
        Ok(counts)
    };
    match query() {
        Ok(counts) => Section::ready(counts),
        Err(_) => Section::missing(State::Unavailable),
    }
}
#[cfg(test)]
#[path = "support_report_tests.rs"]
mod tests;
