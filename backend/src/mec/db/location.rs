//! Where the MEC database lives: `data_dir/mec.db`, unless `NABIMAN_MEC_DB`
//! names another file. It used to default to `./data/mec.db`, relative to
//! whatever directory the server was started from, so it sat outside the
//! data directory (and outside its backups). On the first start after the
//! change the old file is adopted once: renamed into place when it is on the
//! same filesystem, otherwise copied consistently and the old one renamed aside.

use std::fs;
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

/// The old default, relative to the working directory.
pub const LEGACY_RELATIVE: &str = "data/mec.db";
/// What a copied-away legacy database is renamed to, so it is never adopted twice.
pub const LEGACY_SET_ASIDE: &str = "mec.db.moved-to-data-dir";

pub fn default_path() -> PathBuf {
    std::env::var("NABIMAN_MEC_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| nabiman_server::data_dir().join("mec.db"))
}

#[derive(Debug, PartialEq)]
pub enum Adoption {
    NothingToDo,
    /// The new path already exists; the legacy file was left alone.
    LegacyIgnored,
    Moved,
    /// Copied across filesystems; the old file was renamed to this.
    Copied(PathBuf),
}

/// At startup, with the data directory locked. Does nothing when `NABIMAN_MEC_DB` is set.
pub fn adopt_legacy_default() -> io::Result<()> {
    if std::env::var_os("NABIMAN_MEC_DB").is_some() {
        return Ok(());
    }
    let legacy = std::env::current_dir()?.join(LEGACY_RELATIVE);
    let target = nabiman_server::data_dir().join("mec.db");
    match adopt(&legacy, &target)? {
        Adoption::NothingToDo => {}
        Adoption::LegacyIgnored => eprintln!(
            "MEC database: using {}; the old {} is no longer read (remove it once you no longer need it)",
            target.display(),
            legacy.display()
        ),
        Adoption::Moved => println!(
            "MEC database moved from {} to {} (it now lives in the data directory)",
            legacy.display(),
            target.display()
        ),
        Adoption::Copied(aside) => println!(
            "MEC database copied from {} to {} (other filesystem); the old file is kept as {}",
            legacy.display(),
            target.display(),
            aside.display()
        ),
    }
    Ok(())
}

pub fn adopt(legacy: &Path, target: &Path) -> io::Result<Adoption> {
    if fs::symlink_metadata(target).is_ok() {
        let same = fs::canonicalize(legacy).ok() == fs::canonicalize(target).ok();
        return Ok(if legacy.exists() && !same { Adoption::LegacyIgnored } else { Adoption::NothingToDo });
    }
    if !fs::symlink_metadata(legacy).is_ok_and(|m| m.is_file()) {
        return Ok(Adoption::NothingToDo);
    }
    // An older server holds this lock for as long as it runs (see open_owned).
    let owner = fs::OpenOptions::new().read(true).write(true).open(legacy)?;
    if unsafe { libc::flock(owner.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!("The MEC database {} is still in use by another NabiMan server; stop it first", legacy.display()),
        ));
    }
    // Fold any write-ahead log (or roll back a hot journal) so the main file is complete.
    {
        let db = rusqlite::Connection::open(legacy).map_err(io::Error::other)?;
        let _ = db.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(legacy, target) {
        Ok(()) => {
            for suffix in ["-wal", "-shm", "-journal"] {
                let side = sidecar(legacy, suffix);
                if side.exists() {
                    let _ = fs::rename(&side, sidecar(target, suffix));
                }
            }
            sync_parent(target);
            sync_parent(legacy);
            Ok(Adoption::Moved)
        }
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            copy_into(legacy, target)?;
            let aside = legacy.with_file_name(LEGACY_SET_ASIDE);
            fs::rename(legacy, &aside)?;
            sync_parent(legacy);
            Ok(Adoption::Copied(aside))
        }
        Err(e) => Err(e),
    }
}

/// A consistent copy (SQLite's own VACUUM INTO, never a raw copy), published by rename.
pub(super) fn copy_into(source: &Path, target: &Path) -> io::Result<()> {
    let parent = target.parent().ok_or_else(|| io::Error::other("MEC database path has no parent"))?;
    let temp = parent.join(format!(".mec-db-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let db = rusqlite::Connection::open(source).map_err(io::Error::other)?;
        db.execute("VACUUM INTO ?1", [temp.to_string_lossy()]).map_err(io::Error::other)?;
        drop(db);
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
        fs::File::open(&temp)?.sync_all()?;
        fs::rename(&temp, target)?;
        sync_parent(target);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::File::open(parent).and_then(|d| d.sync_all());
    }
}

#[cfg(test)]
#[path = "location_tests.rs"]
mod tests;
