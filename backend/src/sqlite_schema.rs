//! Versioned SQLite schemas. Each database lists its migration steps in order;
//! `PRAGMA user_version` records how many have been applied. Opening runs the
//! missing steps in one transaction (all or none) and refuses a database that
//! a newer NabiMan has already moved past, instead of guessing at its tables.
//!
//! Step 1 of every database is the schema as it stood before versioning, written
//! with `IF NOT EXISTS`, so a version-0 file that already has its tables (every
//! database created before this module) migrates to version 1 without change.
//! Later changes are new steps appended to the list; a released step is never edited.

use rusqlite::{ffi, Connection};
use std::sync::Mutex;

pub type Step = fn(&Connection) -> rusqlite::Result<()>;

pub struct Schema {
    /// The file name, for messages.
    pub name: &'static str,
    pub steps: &'static [Step],
}

impl Schema {
    /// The version this code writes and understands.
    pub const fn current(&self) -> i64 {
        self.steps.len() as i64
    }

    /// Bring `db` to the current version. Returns the version it had before.
    /// Must not be called inside an open transaction.
    pub fn migrate(&self, db: &Connection) -> rusqlite::Result<i64> {
        db.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| {
            let found: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            let known = self.current();
            if found > known || found < 0 {
                return Err(too_new(self.name, found, known));
            }
            for step in &self.steps[found as usize..] {
                step(db)?;
            }
            if found != known {
                // A header field, part of this transaction like the steps.
                db.execute_batch(&format!("PRAGMA user_version = {known}"))?;
            }
            Ok(found)
        })();
        match result {
            Ok(found) => db.execute_batch("COMMIT").map(|_| found),
            Err(error) => {
                let _ = db.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }
}

fn too_new(name: &str, found: i64, known: i64) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_CANTOPEN),
        Some(format!(
            "{name} has schema version {found}, but this NabiMan knows only up to {known}: \
             it was written by a newer NabiMan. Run that version again, or restore a backup \
             taken before the upgrade; the file was not changed."
        )),
    )
}

static REPORTED: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// For stores that open lazily and retry: say why a database cannot be opened, once.
pub fn report_once(name: &'static str, error: &rusqlite::Error) {
    use crate::lock_safe::LockExt;
    let mut seen = REPORTED.lock_safe();
    if !seen.contains(&name) {
        seen.push(name);
        eprintln!("Cannot open {name}: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v1(db: &Connection) -> rusqlite::Result<()> {
        db.execute_batch("CREATE TABLE IF NOT EXISTS notes(id INTEGER PRIMARY KEY, text TEXT NOT NULL);")
    }
    fn v2(db: &Connection) -> rusqlite::Result<()> {
        db.execute_batch("ALTER TABLE notes ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;")
    }
    fn broken(db: &Connection) -> rusqlite::Result<()> {
        db.execute_batch("CREATE TABLE half(x); SELECT * FROM no_such_table;")
    }
    const ONE: Schema = Schema { name: "notes.sqlite3", steps: &[v1] };
    const TWO: Schema = Schema { name: "notes.sqlite3", steps: &[v1, v2] };

    fn version(db: &Connection) -> i64 {
        db.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn a_fresh_database_gets_every_step() {
        let db = Connection::open_in_memory().unwrap();
        assert_eq!(TWO.migrate(&db).unwrap(), 0);
        assert_eq!(version(&db), 2);
        db.execute("INSERT INTO notes(text, pinned) VALUES('a', 1)", []).unwrap();
        // Opening again changes nothing.
        assert_eq!(TWO.migrate(&db).unwrap(), 2);
    }

    #[test]
    fn an_unversioned_database_with_its_tables_and_rows_migrates_without_loss() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE notes(id INTEGER PRIMARY KEY, text TEXT NOT NULL); INSERT INTO notes(text) VALUES('kept');").unwrap();
        assert_eq!(version(&db), 0);
        assert_eq!(TWO.migrate(&db).unwrap(), 0);
        assert_eq!(version(&db), 2);
        let row: (String, i64) = db.query_row("SELECT text, pinned FROM notes", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(row, ("kept".to_string(), 0));
    }

    #[test]
    fn only_the_missing_steps_run() {
        let db = Connection::open_in_memory().unwrap();
        ONE.migrate(&db).unwrap();
        db.execute("INSERT INTO notes(text) VALUES('v1 row')", []).unwrap();
        assert_eq!(TWO.migrate(&db).unwrap(), 1);
        assert_eq!(version(&db), 2);
    }

    #[test]
    fn a_database_from_a_newer_release_is_refused_untouched() {
        let db = Connection::open_in_memory().unwrap();
        TWO.migrate(&db).unwrap();
        let error = ONE.migrate(&db).unwrap_err().to_string();
        assert!(error.contains("notes.sqlite3 has schema version 2"), "{error}");
        assert!(error.contains("knows only up to 1"), "{error}");
        assert_eq!(version(&db), 2);
        assert!(db.is_autocommit(), "no transaction is left open");
    }

    #[test]
    fn a_failing_step_rolls_the_whole_migration_back() {
        let db = Connection::open_in_memory().unwrap();
        let bad = Schema { name: "notes.sqlite3", steps: &[v1, broken] };
        assert!(bad.migrate(&db).is_err());
        assert_eq!(version(&db), 0);
        let tables: i64 = db.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0)).unwrap();
        assert_eq!(tables, 0, "step 1 was rolled back with step 2");
    }
}

#[cfg(test)]
#[path = "sqlite_schema_tests.rs"]
mod database_tests;
