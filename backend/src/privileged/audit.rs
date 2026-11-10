//! The helper's own record of every call: who connected (kernel peer uid and
//! pid), what was asked (the typed operation; no command output), and what the
//! helper decided. One JSON line each, in a root-only file next to the ledger.
//! The service ledger stays the durable intent/receipt store; this is the
//! audit trail, including refusals the ledger never sees.
use super::policy::validate_parent;
use serde::Serialize;
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

/// Past this size the file is renamed to `.1` (one generation kept).
const ROTATE_AT: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub at: String,
    pub peer_uid: u32,
    pub peer_pid: i32,
    pub request_id: Option<String>,
    pub actor_id: Option<String>,
    pub operation: Option<serde_json::Value>,
    /// `peer_rejected`, `unreadable`, `invalid`, `expired`, `denied`, `busy`,
    /// `dispatched`, `completed`, `conflict`, `not_found`.
    pub decision: &'static str,
    pub state: Option<&'static str>,
    pub exit_code: Option<i32>,
}
impl Entry {
    pub fn new(peer_uid: u32, peer_pid: i32, decision: &'static str) -> Self {
        Self {
            at: chrono::Utc::now().to_rfc3339(),
            peer_uid,
            peer_pid,
            request_id: None,
            actor_id: None,
            operation: None,
            decision,
            state: None,
            exit_code: None,
        }
    }
}

/// Where entries go: a file, or (tests) memory.
pub enum Audit {
    File(PathBuf),
    Memory(Vec<Entry>),
    Off,
}
impl Audit {
    /// `<dir of ledger>/audit.jsonl`, checked like the ledger (root-owned parents).
    pub fn beside(ledger: &Path) -> io::Result<Self> {
        let path = ledger.with_file_name("audit.jsonl");
        validate_parent(&path)?;
        Ok(Self::File(path))
    }
    /// Appends one entry. A failure is returned: a system operation is not
    /// dispatched without its audit line.
    pub fn record(&mut self, entry: Entry) -> io::Result<()> {
        match self {
            Self::Off => Ok(()),
            Self::Memory(entries) => {
                entries.push(entry);
                Ok(())
            }
            Self::File(path) => {
                if fs::symlink_metadata(&*path).is_ok_and(|m| m.len() > ROTATE_AT) {
                    fs::rename(&*path, path.with_extension("jsonl.1"))?;
                }
                let mut file = fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .mode(0o600)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                    .open(&*path)?;
                let metadata = file.metadata()?;
                if !metadata.is_file() || metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
                    return Err(io::Error::other("Unsafe helper audit file"));
                }
                let mut line = serde_json::to_vec(&entry).map_err(io::Error::other)?;
                line.push(b'\n');
                file.write_all(&line)?;
                file.sync_data()
            }
        }
    }
    #[cfg(test)]
    pub fn entries(&self) -> &[Entry] {
        match self {
            Self::Memory(entries) => entries,
            _ => &[],
        }
    }
}
