pub mod audit_store;
pub mod docs_store;
pub mod identity_store;
pub mod job_store;
pub mod membership_store;
pub mod schema;
pub mod tenant_store;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub type DbHandle = Arc<Mutex<Connection>>;

pub fn open(path: PathBuf) -> rusqlite::Result<DbHandle> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path)?;
    schema::initialize(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}

/// Production startup requires a durable file; tests may explicitly use memory.
pub fn open_persistent(path: PathBuf) -> std::io::Result<DbHandle> {
    if path.as_os_str().is_empty() || path == std::path::Path::new(":memory:") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "NABIMAN_MEC_DB must name a persistent database file",
        ));
    }
    open(path)
        .map_err(|e| std::io::Error::other(format!("Cannot open persistent MEC database: {e}")))
}

/// Keep the advisory lock alive for the entire server lifetime. SQLite's own
/// transaction locks do not identify whether a server process is still running.
pub struct OwnedDatabase {
    pub handle: DbHandle,
    _owner: nix::fcntl::Flock<std::fs::File>,
}

pub fn open_owned(path: PathBuf) -> std::io::Result<OwnedDatabase> {
    if path.as_os_str().is_empty() || path == std::path::Path::new(":memory:") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Persistent database file required",
        ));
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)?;
    let owner = nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock)
        .map_err(|(_, e)| std::io::Error::new(if e == nix::errno::Errno::EWOULDBLOCK { std::io::ErrorKind::WouldBlock } else { std::io::ErrorKind::Other }, "Cannot acquire exclusive MEC database ownership; stop the existing server before starting another"))?;
    let handle = open_persistent(path)?;
    let interrupted = job_store::JobStore::new(handle.clone())
        .recover_interrupted()
        .map_err(|e| {
            std::io::Error::other(format!("Cannot reconcile interrupted MEC jobs: {e}"))
        })?;
    membership_store::recover_interrupted(&handle).map_err(|e| {
        std::io::Error::other(format!(
            "Cannot reconcile interrupted membership operations: {e}"
        ))
    })?;
    if interrupted > 0 {
        println!("MEC recovery: {interrupted} interrupted jobs require resource verification");
    }
    Ok(OwnedDatabase {
        handle,
        _owner: owner,
    })
}

pub fn open_in_memory() -> rusqlite::Result<DbHandle> {
    let conn = Connection::open_in_memory()?;
    schema::initialize(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}

pub fn default_path() -> PathBuf {
    std::env::var("NABIMAN_MEC_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./data/mec.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_open_does_not_replace_failures_with_memory() {
        let dir = std::env::temp_dir().join(format!("nabiman-db-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        assert!(open_persistent(dir.clone()).is_err());
        assert!(open_persistent(PathBuf::from(":memory:")).is_err());
        assert!(open_persistent(PathBuf::new()).is_err());
        let path = dir.join("state.db");
        let db = open_persistent(path.clone()).unwrap();
        db.lock().unwrap().execute("INSERT INTO mec_config (key, value, updated_at) VALUES ('durability-test', 'saved', '2026-10-05T00:00:00Z')", []).unwrap();
        drop(db);
        let reopened = open_persistent(path).unwrap();
        let value: String = reopened
            .lock()
            .unwrap()
            .query_row(
                "SELECT value FROM mec_config WHERE key='durability-test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(value, "saved");
        drop(reopened);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn open_memory() {
        let db = open_in_memory().unwrap();
        let conn = db.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(count >= 4);
    }
}

#[cfg(test)]
mod recovery_tests;
