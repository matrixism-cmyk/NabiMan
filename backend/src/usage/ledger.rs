//! The usage ledger: once a minute, how many terminal sessions were open (and
//! attached), how many users and servers there were, added up per UTC day in
//! `data_dir/usage.sqlite3`. Session-hours and peaks for `/api/usage` come
//! from here; the audit log keeps only its last records in memory and the
//! terminal registry forgets a session when it closes, so neither can say how
//! long sessions ran last month.

use crate::lock_safe::LockExt;
use rusqlite::{params, Connection, OptionalExtension};
use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub(crate) const INTERVAL: Duration = Duration::from_secs(60);
/// A longer gap (the machine slept, the process stalled) counts as this much.
const MAX_STEP_SECS: u64 = 2 * 60;
/// Days older than this are dropped.
const KEEP_DAYS: i64 = 800;
pub(crate) const FILE: &str = "usage.sqlite3";

static DB: Mutex<Option<Connection>> = Mutex::new(None);

/// Versions of `usage.sqlite3` (see `sqlite_schema`); append, never edit.
pub(crate) const SCHEMA: crate::sqlite_schema::Schema =
    crate::sqlite_schema::Schema { name: FILE, steps: &[schema_v1] };

fn schema_v1(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS usage_daily(
           day TEXT PRIMARY KEY,
           samples INTEGER NOT NULL DEFAULT 0,
           up_seconds INTEGER NOT NULL DEFAULT 0,
           session_seconds INTEGER NOT NULL DEFAULT 0,
           attached_seconds INTEGER NOT NULL DEFAULT 0,
           peak_sessions INTEGER NOT NULL DEFAULT 0,
           peak_users INTEGER NOT NULL DEFAULT 0,
           peak_servers INTEGER NOT NULL DEFAULT 0
         );",
    )
}

pub(crate) fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    SCHEMA.migrate(&db)?;
    Ok(db)
}

pub(crate) fn with_db<T>(f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Option<T> {
    let mut slot = DB.lock_safe();
    if slot.is_none() {
        *slot = open(&nabiman_server::data_dir().join(FILE))
            .map_err(|e| crate::sqlite_schema::report_once(SCHEMA.name, &e))
            .ok();
    }
    slot.as_ref().and_then(|db| f(db).ok())
}

/// One look at the instance, covering the `seconds` since the last.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Sample {
    pub seconds: u64,
    pub sessions: u64,
    pub attached: u64,
    pub users: u64,
    pub servers: u64,
}

pub(crate) fn record(db: &Connection, day: &str, s: &Sample) -> rusqlite::Result<()> {
    let secs = s.seconds.min(MAX_STEP_SECS) as i64;
    db.execute(
        "INSERT INTO usage_daily(day,samples,up_seconds,session_seconds,attached_seconds,peak_sessions,peak_users,peak_servers)
         VALUES (?1,1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(day) DO UPDATE SET samples=samples+1, up_seconds=up_seconds+?2,
           session_seconds=session_seconds+?3, attached_seconds=attached_seconds+?4,
           peak_sessions=max(peak_sessions,?5), peak_users=max(peak_users,?6), peak_servers=max(peak_servers,?7)",
        params![day, secs, secs * s.sessions as i64, secs * s.attached as i64, s.sessions as i64, s.users as i64, s.servers as i64],
    )?;
    Ok(())
}

fn prune(db: &Connection, today: chrono::NaiveDate) -> rusqlite::Result<()> {
    let cutoff = (today - chrono::Duration::days(KEEP_DAYS)).to_string();
    db.execute("DELETE FROM usage_daily WHERE day < ?1", params![cutoff]).map(drop)
}

/// The ledger's sums over `from..=to` (UTC days, `YYYY-MM-DD`).
#[derive(serde::Serialize, Debug, Default, PartialEq)]
pub(crate) struct Totals {
    pub sampled_days: i64,
    pub first_day: Option<String>,
    pub up_seconds: i64,
    pub session_seconds: i64,
    pub attached_seconds: i64,
    pub peak_sessions: i64,
    pub peak_users: i64,
    pub peak_servers: i64,
}

pub(crate) fn totals(db: &Connection, from: &str, to: &str) -> rusqlite::Result<Totals> {
    let mut t = db.query_row(
        "SELECT count(*), coalesce(sum(up_seconds),0), coalesce(sum(session_seconds),0), coalesce(sum(attached_seconds),0),
                coalesce(max(peak_sessions),0), coalesce(max(peak_users),0), coalesce(max(peak_servers),0)
         FROM usage_daily WHERE day BETWEEN ?1 AND ?2",
        params![from, to],
        |r| {
            Ok(Totals {
                sampled_days: r.get(0)?,
                first_day: None,
                up_seconds: r.get(1)?,
                session_seconds: r.get(2)?,
                attached_seconds: r.get(3)?,
                peak_sessions: r.get(4)?,
                peak_users: r.get(5)?,
                peak_servers: r.get(6)?,
            })
        },
    )?;
    // The first day ever measured: before it, the ledger simply did not exist yet.
    t.first_day = db.query_row("SELECT min(day) FROM usage_daily", [], |r| r.get(0)).optional()?.flatten();
    Ok(t)
}

/// Once a minute for the life of the server. `look` reads the stores.
pub(crate) fn start(look: impl Fn() -> Sample + Send + 'static) {
    std::thread::spawn(move || {
        let mut last = Instant::now();
        let mut pruned = None;
        loop {
            std::thread::sleep(INTERVAL);
            let now = Instant::now();
            let sample = Sample { seconds: now.duration_since(last).as_secs(), ..look() };
            last = now;
            let today = chrono::Utc::now().date_naive();
            with_db(|db| {
                record(db, &today.to_string(), &sample)?;
                if pruned != Some(today) {
                    prune(db, today)?;
                    pruned = Some(today);
                }
                Ok(())
            });
        }
    });
}
