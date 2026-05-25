pub mod schema;
pub mod audit_store;
pub mod job_store;
pub mod tenant_store;
pub mod docs_store;

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
