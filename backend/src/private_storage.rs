//! Atomic replacement for persisted state: a private temporary file in the
//! same directory, fsync, then rename over the old file. A crash or error at
//! any point leaves either the old file or the new one, never a torn mix.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Replace `path` with `bytes`, mode 0600 (credentials, authentication data, state).
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_mode(path, bytes, 0o600)
}

/// Replace `path` with `bytes` and the given mode (for files other programs read,
/// such as web server configuration). The mode is applied exactly, not through umask.
pub fn write_mode(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    write_with(path, bytes, mode, |_| Ok(()))
}

/// The mode an existing file has (so a rewrite keeps it), else `default`.
pub fn existing_mode(path: &Path, default: u32) -> u32 {
    fs::metadata(path).map(|m| m.permissions().mode() & 0o7777).unwrap_or(default)
}

/// `before_publish` runs after the temporary file is complete and synced, just
/// before the rename; an error from it (or anything earlier) leaves `path` as it was.
fn write_with(
    path: &Path,
    bytes: &[u8],
    mode: u32,
    before_publish: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent directory"))?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".nabiman-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        file.write_all(bytes)?;
        if mode != 0o600 {
            file.set_permissions(fs::Permissions::from_mode(mode))?;
        }
        file.sync_all()?;
        before_publish(&temp)?;
        fs::rename(&temp, path)?;
        // Rename has committed the replacement. A directory sync improves
        // crash durability but must not turn a successful save into an error.
        let _ = fs::File::open(parent).and_then(|dir| dir.sync_all());
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nabiman-storage-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn creates_and_replaces_private_file() {
        let dir = temp_dir();
        let path = dir.join("credentials.json");
        write(&path, b"first").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_interrupted_write_leaves_the_old_file_intact() {
        let dir = temp_dir();
        let path = dir.join("alert_rules.json");
        write(&path, b"[\"old\"]").unwrap();
        let mut staged = None;
        let result = write_with(&path, b"[\"new, never published\"]", 0o600, |temp| {
            // The new content is complete on disk, but the process "dies" before rename.
            assert_eq!(fs::read(temp).unwrap(), b"[\"new, never published\"]");
            staged = Some(temp.to_path_buf());
            Err(io::Error::other("simulated crash before rename"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"[\"old\"]");
        assert!(!staged.unwrap().exists(), "the temporary file is cleaned up");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_requested_mode_is_applied_exactly() {
        let dir = temp_dir();
        let path = dir.join("site.conf");
        write_mode(&path, b"server {}", 0o644).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o644);
        assert_eq!(existing_mode(&path, 0o600), 0o644);
        assert_eq!(existing_mode(&dir.join("missing"), 0o640), 0o640);
        fs::remove_dir_all(dir).unwrap();
    }
}
