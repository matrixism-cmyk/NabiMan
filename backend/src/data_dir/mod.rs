//! The data directory as a whole: what must hold before any store opens it.

pub mod lock;

/// Run once at startup, before any store reads or writes the data directory.
/// The returned lock must be kept for the life of the server.
pub fn prepare() -> std::io::Result<lock::DataDirLock> {
    let dir = nabiman_server::data_dir();
    let held = lock::acquire(&dir)?;
    // Before the MEC database is opened: it may still be at its old ./data/mec.db.
    crate::mec::db::location::adopt_legacy_default()?;
    Ok(held)
}
