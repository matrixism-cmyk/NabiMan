//! A backup of a NabiMan install: a consistent copy of everything in the data
//! directory that a working install needs, as one gzip tar with a manifest,
//! and its restore into a new, empty directory.
//!
//! * JSON and other state files are copied as they are (every store replaces
//!   its file atomically, so a copy is always one whole version of it).
//! * SQLite databases are copied with `VACUUM INTO`, never byte for byte, so a
//!   live database in WAL mode is captured at one transaction boundary.
//! * `secret.key`, which decrypts the stored server passwords and forwarding
//!   credentials, is left out unless asked for: a backup that leaks then holds
//!   only ciphertext. Keep the key separately (offline) and supply it at restore.
//! * Caches, live device links, lock files and earlier backups are left out;
//!   the manifest lists what was excluded and why.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

mod create;
mod restore;
pub use create::create;
pub use restore::{restore, verify, Restored};

pub const MANIFEST: &str = "nabiman-state.json";
pub const FORMAT: &str = "nabiman-state";
pub const FORMAT_VERSION: u32 = 1;
pub const SECRET_KEY: &str = "secret.key";
/// The manifest of the snapshot a restored directory came from.
pub const RESTORED_FROM: &str = ".restored-from-snapshot.json";

#[derive(Default, Clone, Debug)]
pub struct Options {
    /// Put `secret.key` in the archive too (off by default; see above).
    pub include_secret_key: bool,
    /// The MEC database when `NABIMAN_MEC_DB` puts it outside the data directory;
    /// it is stored as `mec.db` in the archive.
    pub mec_db: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub created_at: String,
    pub nabiman_version: String,
    pub secret_key_included: bool,
    pub files: BTreeMap<String, Entry>,
    pub excluded: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub sha256: String,
    pub size: u64,
    /// Set for SQLite databases: the schema version (PRAGMA user_version) captured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sqlite_user_version: Option<i64>,
}

/// Top-level entries never copied, and why (recorded in the manifest).
pub const EXCLUDED_TOP: &[(&str, &str)] = &[
    ("backups", "backup archives, including this one"),
    ("preview-cache", "office preview cache, rebuilt on demand"),
    ("serial", "live serial device links"),
    ("guac-drive", "RDP transfer drive (user files, not NabiMan state)"),
    ("session-recordings", "session recordings: large, back them up separately if kept"),
    ("db_backups", "database panel dumps of other databases"),
    ("tmux.conf", "regenerated at every start"),
];

/// Names skipped anywhere: hidden files (the data directory lock, temporary
/// files of atomic writes, staging), store lock files and SQLite sidecars
/// (each database is copied whole by VACUUM INTO).
pub(crate) fn skipped_name(name: &str) -> bool {
    name.starts_with('.')
        || name.ends_with(".lock")
        || name.ends_with("-wal")
        || name.ends_with("-shm")
        || name.ends_with("-journal")
}

pub(crate) fn is_sqlite(path: &std::path::Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 16];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok_and(|_| &head == b"SQLite format 3\0")
}

#[cfg(test)]
#[path = "../state_snapshot_tests.rs"]
mod tests;
