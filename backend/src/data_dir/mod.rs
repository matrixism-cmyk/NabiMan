//! The data directory as a whole: what must hold before any store opens it.

pub mod lock;

/// The data directory, locked for this server, and whether it was brand new.
pub struct Prepared {
    _lock: lock::DataDirLock,
    fresh: bool,
}

/// Run once at startup, before any store reads or writes the data directory.
/// The returned value holds the lock and must be kept for the life of the server.
pub fn prepare() -> std::io::Result<Prepared> {
    let dir = nabiman_server::data_dir();
    let held = lock::acquire(&dir)?;
    // Never held users or schedules: a new install, which gets the defaults.
    let fresh = !dir.join("users.json").exists() && !dir.join("backup_schedules.json").exists();
    // Before the MEC database is opened: it may still be at its old ./data/mec.db.
    crate::mec::db::location::adopt_legacy_default()?;
    Ok(Prepared { _lock: held, fresh })
}

impl Prepared {
    /// After the MEC database opened (a server refused there leaves nothing behind):
    /// what a brand-new data directory starts with.
    pub fn seed_defaults(&self) {
        if let Some(note) = crate::backup::seed_default_schedule(&nabiman_server::data_dir(), self.fresh) {
            println!("{note}");
        }
    }
}
