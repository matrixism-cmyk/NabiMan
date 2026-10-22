//! The saved servers as last read, kept while the file is unchanged.
//!
//! An open terminal to a saved server rechecks that server before every
//! frame of output, and reading and parsing the whole file each time cost
//! more than the terminal itself. Whether the file changed is told by its
//! identity — device, inode, size, and modification and change times to the
//! nanosecond — which a save (an atomic rename: a new inode) or any edit in
//! place alters, so a revocation still takes effect on the very next check.
//! A file changed in the last two seconds is never kept: two edits in one
//! tick of a coarse file clock would carry the same times.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::models::RemoteServer;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Stamp {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}

/// The file's identity, `None` when there is no file.
fn stamp(path: &Path) -> std::io::Result<Option<Stamp>> {
    match std::fs::metadata(path) {
        Ok(m) => Ok(Some(Stamp {
            dev: m.dev(),
            ino: m.ino(),
            len: m.len(),
            mtime: (m.mtime(), m.mtime_nsec()),
            ctime: (m.ctime(), m.ctime_nsec()),
        })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// How recent a change must not be for the file to be kept.
const SETTLED_SECS: i64 = 2;

fn settled(stamp: &Option<Stamp>, now: i64) -> bool {
    stamp.is_none_or(|s| now - s.mtime.0.max(s.ctime.0) > SETTLED_SECS)
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

type Entry = (PathBuf, Option<Stamp>, Vec<RemoteServer>);
static CACHE: Mutex<Option<Entry>> = Mutex::new(None);

/// What `read` gives for `path`, from the cache while the file is the same.
/// When its identity cannot be told, it is read every time, as before.
pub(super) fn cached(
    path: &Path,
    read: impl FnOnce(&Path) -> Result<Vec<RemoteServer>, String>,
) -> Result<Vec<RemoteServer>, String> {
    cached_at(path, read, now())
}

fn cached_at(
    path: &Path,
    read: impl FnOnce(&Path) -> Result<Vec<RemoteServer>, String>,
    now: i64,
) -> Result<Vec<RemoteServer>, String> {
    let Ok(before) = stamp(path) else { return read(path) };
    {
        let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((p, s, servers)) = cache.as_ref() {
            if p == path && *s == before {
                return Ok(servers.clone());
            }
        }
    }
    let servers = read(path)?;
    // Kept only if nothing changed while it was read, and not just now.
    if stamp(path).ok() == Some(before) && settled(&before, now) {
        *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((path.to_path_buf(), before, servers.clone()));
    }
    Ok(servers)
}

/// After a save from here: the next read goes to the file.
pub(super) fn forget() {
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn server(id: &str) -> RemoteServer {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "host": "h", "port": 22, "user": "u", "auth_method": "key", "created_at": "now", "tags": [], "memo": ""
        }))
        .unwrap()
    }

    #[test]
    fn the_file_is_read_again_only_when_it_changed() {
        let path = std::env::temp_dir().join(format!("nabiman-servers-{}.json", uuid::Uuid::new_v4()));
        let reads = Cell::new(0);
        let read = |p: &Path| {
            reads.set(reads.get() + 1);
            let text = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
            Ok(text.split(',').map(server).collect())
        };
        let later = now() + 60;
        std::fs::write(&path, "a,b").unwrap();
        cached_at(&path, read, now()).unwrap();
        cached_at(&path, read, now()).unwrap();
        assert_eq!(reads.get(), 2, "a file changed just now is read every time");
        assert_eq!(cached_at(&path, read, later).unwrap().len(), 2);
        cached_at(&path, read, later).unwrap();
        assert_eq!(reads.get(), 3, "a settled, unchanged file is not read again");

        // Same length, edited in place: seen at once. (What is kept has
        // settled, so a later edit always falls in a later tick of the clock.)
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&path, "a,c").unwrap();
        assert_eq!(cached_at(&path, read, later).unwrap()[1].id, "c");
        // Replaced by a rename, as a save does.
        let next = path.with_extension("next");
        std::fs::write(&next, "z").unwrap();
        std::fs::rename(&next, &path).unwrap();
        assert_eq!(cached_at(&path, read, later).unwrap()[0].id, "z");
        assert_eq!(reads.get(), 5);

        forget();
        cached_at(&path, read, later).unwrap();
        assert_eq!(reads.get(), 6, "a forgotten cache reads the file");
        std::fs::remove_file(&path).unwrap();
    }
}
