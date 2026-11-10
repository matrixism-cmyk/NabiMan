//! One server per data directory. The JSON stores rewrite whole files from
//! what they hold in memory, so two servers on the same directory would
//! silently undo each other's changes. `data_dir/.lock` carries an exclusive
//! advisory lock (flock) for the life of the process and the holder's PID.
//! The kernel drops the lock when the process exits, however it exits; the
//! descriptor is close-on-exec, so tmux and other children never inherit it.

use std::fs;
use std::io::{self, Read, Seek, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub const LOCK_FILE: &str = ".lock";

/// Held for the life of the server; dropping it releases the directory.
#[derive(Debug)]
pub struct DataDirLock {
    _file: fs::File,
    pub path: PathBuf,
}

pub fn acquire(dir: &Path) -> io::Result<DataDirLock> {
    if !dir.exists() {
        fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    let path = dir.join(LOCK_FILE);
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .map_err(|e| io::Error::new(e.kind(), format!("Cannot open data directory lock {}: {e}", path.display())))?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other(format!("{} is not a regular file", path.display())));
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EWOULDBLOCK) {
            return Err(io::Error::new(error.kind(), format!("Cannot lock {}: {error}", path.display())));
        }
        let mut holder = String::new();
        let _ = file.read_to_string(&mut holder);
        let holder = holder.trim();
        let who = if holder.chars().all(|c| c.is_ascii_digit()) && !holder.is_empty() {
            format!("PID {holder}")
        } else {
            "an unknown process".to_string()
        };
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "NabiMan data directory {} is already in use by another server ({who}). \
                 Stop that server first, or give this one its own NABIMAN_DATA_DIR.",
                dir.display()
            ),
        ));
    }
    // Who holds it, for the next server's message and for operators (`cat .lock`).
    file.set_len(0)?;
    file.rewind()?;
    writeln!(file, "{}", std::process::id())?;
    let _ = file.sync_all();
    Ok(DataDirLock { _file: file, path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_holder_is_refused_with_the_first_ones_pid_until_it_lets_go() {
        let dir = std::env::temp_dir().join(format!("nabiman-lock-{}", uuid::Uuid::new_v4()));
        let first = acquire(&dir).unwrap();
        assert_eq!(fs::read_to_string(&first.path).unwrap().trim(), std::process::id().to_string());
        // flock is per open file description, so a second open in this process conflicts too.
        let refused = acquire(&dir).unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::WouldBlock);
        let message = refused.to_string();
        assert!(message.contains(&format!("PID {}", std::process::id())), "{message}");
        assert!(message.contains(&dir.display().to_string()), "{message}");
        drop(first);
        let again = acquire(&dir).unwrap();
        drop(again);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_data_directory_is_created_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("nabiman-lock-{}", uuid::Uuid::new_v4())).join("data");
        let lock = acquire(&dir).unwrap();
        assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::metadata(&lock.path).unwrap().permissions().mode() & 0o777, 0o600);
        drop(lock);
        fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}
