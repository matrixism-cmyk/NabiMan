//! Single-process shared registry with durable atomic publication and fail-closed writes.
use super::share::ShareLink;
use nix::fcntl::{Flock, FlockArg};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub type ShareStore = Arc<Mutex<Registry>>;
pub struct Registry {
    path: PathBuf,
    links: HashMap<String, ShareLink>,
    healthy: bool,
    _owner: Flock<File>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> PathBuf {
        std::env::temp_dir().join(format!("nabiman-share-store-{}", uuid::Uuid::new_v4()))
    }
    fn link(token: &str) -> ShareLink {
        ShareLink {
            token: token.into(),
            session_id: "one".into(),
            owner: "owner".into(),
            owner_id: "immutable-id".into(),
            owner_authorization_version: "fixture-version".into(),
            label: "fixture".into(),
            created_unix: 1,
            expires_unix: 3601,
            password_hash: None,
            read_only: true,
            last_used_unix: 0,
            needs_password: false,
            alive: true,
            usable: true,
        }
    }
    #[test]
    fn exclusive_owner_atomic_private_file_and_revocation_survive_restart() {
        use std::os::unix::fs::PermissionsExt;
        let dir = directory();
        let path = dir.join("terminal_shares.json");
        let store = open(path.clone()).unwrap();
        assert!(open(path.clone()).is_err());
        store
            .lock()
            .unwrap()
            .update(|m| {
                m.insert("first".into(), link("first"));
                m.insert("unrelated".into(), link("unrelated"));
                Ok(())
            })
            .unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(store);
        let store = open(path.clone()).unwrap();
        assert!(store.lock().unwrap().get("first").is_some());
        store
            .lock()
            .unwrap()
            .update(|m| {
                m.remove("first");
                Ok(())
            })
            .unwrap();
        drop(store);
        let store = open(path).unwrap();
        assert!(store.lock().unwrap().get("first").is_none());
        assert!(store.lock().unwrap().get("unrelated").is_some());
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failed_publication_quarantines_old_snapshot_and_never_reports_success() {
        let dir = directory();
        let path = dir.join("terminal_shares.json");
        let store = open(path.clone()).unwrap();
        store
            .lock()
            .unwrap()
            .update(|m| {
                m.insert("first".into(), link("first"));
                Ok(())
            })
            .unwrap();
        // Publication will fail at rename. The last successfully published snapshot
        // must not remain usable in this process, even if it contains the old link.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let mut registry = store.lock().unwrap();
        assert!(registry
            .update(|m| {
                m.remove("first");
                Ok(())
            })
            .is_err());
        assert!(!registry.ready());
        assert!(registry.get("first").is_none());
        assert!(registry.all().is_err());
        assert!(registry.update(|_| Ok(())).is_err());
        drop(registry);
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn malformed_or_mismatched_registry_is_not_replaced_by_empty_data() {
        let dir = directory();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("terminal_shares.json");
        std::fs::write(&path, "invalid-json").unwrap();
        assert!(open(path.clone()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid-json");
        let entries = HashMap::from([("wrong-key", link("different-token"))]);
        std::fs::write(&path, serde_json::to_vec(&entries).unwrap()).unwrap();
        assert!(open(path).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn concurrent_creations_and_revocations_preserve_unrelated_links() {
        let dir = directory();
        let path = dir.join("terminal_shares.json");
        let store = open(path).unwrap();
        let threads: Vec<_> = (0..12)
            .map(|i| {
                let store = store.clone();
                std::thread::spawn(move || {
                    let token = format!("{i}");
                    store
                        .lock()
                        .unwrap()
                        .update(|m| {
                            m.insert(token.clone(), link(&token));
                            Ok(())
                        })
                        .unwrap();
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let threads: Vec<_> = (0..6)
            .map(|i| {
                let store = store.clone();
                std::thread::spawn(move || {
                    store
                        .lock()
                        .unwrap()
                        .update(|m| {
                            m.remove(&format!("{i}"));
                            Ok(())
                        })
                        .unwrap();
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let records = store.lock().unwrap().all().unwrap();
        assert_eq!(records.len(), 6);
        assert!(records
            .iter()
            .all(|l| l.token.parse::<usize>().unwrap() >= 6));
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
const MAX_LINKS: usize = 4096;
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

pub fn open(path: PathBuf) -> io::Result<ShareStore> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| io::Error::other("Share registry requires a parent directory"))?;
    std::fs::create_dir_all(parent)?;
    // A stable companion inode remains locked while atomic rename replaces the data inode.
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(path.with_extension("lock"))?;
    let owner = Flock::lock(lock, FlockArg::LockExclusiveNonblock).map_err(|(_, e)| {
        io::Error::other(format!("Share registry already owned or unavailable: {e}"))
    })?;
    let links: HashMap<String, ShareLink> = match std::fs::metadata(&path) {
        Ok(meta) => {
            if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
                return Err(io::Error::other("Invalid share registry file"));
            }
            serde_json::from_slice(&std::fs::read(&path)?).map_err(io::Error::other)?
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => HashMap::new(),
        Err(e) => return Err(e),
    };
    if links.len() > MAX_LINKS
        || links.iter().any(|(key, link)| {
            key != &link.token || link.session_id.is_empty() || link.owner.is_empty()
        })
    {
        return Err(io::Error::other("Invalid share registry records"));
    }
    let registry = Registry {
        path,
        links,
        healthy: true,
        _owner: owner,
    };
    // Verify publication permissions even for a new, empty registry.
    registry.publish(&registry.links)?;
    Ok(Arc::new(Mutex::new(registry)))
}

impl Registry {
    pub(super) fn ready(&self) -> bool {
        self.healthy
    }
    fn publish(&self, links: &HashMap<String, ShareLink>) -> io::Result<()> {
        let bytes = serde_json::to_vec(links).map_err(io::Error::other)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(io::Error::other("Share registry exceeds size limit"));
        }
        let temporary = self
            .path
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::rename(&temporary, &self.path)?;
            File::open(self.path.parent().unwrap())?.sync_all()
        })();
        let _ = std::fs::remove_file(temporary);
        result
    }
    pub(super) fn all(&self) -> io::Result<Vec<ShareLink>> {
        if !self.healthy {
            return Err(io::Error::other(
                "Share registry unavailable after publication failure",
            ));
        }
        Ok(self.links.values().cloned().collect())
    }
    pub(super) fn get(&self, token: &str) -> Option<ShareLink> {
        self.healthy
            .then(|| self.links.get(token).cloned())
            .flatten()
    }
    pub(super) fn touch(&mut self, token: &str) {
        if self.healthy {
            if let Some(link) = self.links.get_mut(token) {
                link.last_used_unix = super::session::now_unix();
            }
        }
    }
    pub(super) fn update<F>(&mut self, change: F) -> io::Result<()>
    where
        F: FnOnce(&mut HashMap<String, ShareLink>) -> io::Result<()>,
    {
        if !self.healthy {
            return Err(io::Error::other("Share registry unavailable"));
        }
        let mut next = self.links.clone();
        change(&mut next)?;
        if next.len() > MAX_LINKS {
            return Err(io::Error::other("Share link limit reached"));
        }
        if let Err(e) = self.publish(&next) {
            // Rename may already have happened. Never continue admitting visitors from
            // an old in-memory snapshot after an uncertain durable write.
            self.healthy = false;
            return Err(e);
        }
        self.links = next;
        Ok(())
    }
}
