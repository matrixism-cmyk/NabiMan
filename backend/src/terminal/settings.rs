//! Per-user terminal preferences, stored next to the session registry.

use nix::fcntl::{Flock, FlockArg};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub(crate) fn default_scrollback() -> u32 {
    5000
}
fn default_true() -> bool {
    true
}
fn default_ka_interval() -> u64 {
    30
}
fn default_font_size() -> u32 {
    14
}

/// Preferences that outlive a browser: how much history to keep, whether the
/// session is kept alive server-side, and how big the text is.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TerminalSettings {
    #[serde(default = "default_scrollback")]
    pub scrollback_lines: u32,
    #[serde(default = "default_true")]
    pub keepalive: bool,
    #[serde(default = "default_ka_interval")]
    pub keepalive_interval_secs: u64,
    /// Seconds a detached session survives; 0 = never auto-closed.
    #[serde(default)]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    /// Replay the server-side history when a window reopens with an empty buffer.
    #[serde(default = "default_true")]
    pub restore_scrollback: bool,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            scrollback_lines: default_scrollback(),
            keepalive: true,
            keepalive_interval_secs: default_ka_interval(),
            idle_timeout_secs: 0,
            font_size: default_font_size(),
            restore_scrollback: true,
        }
    }
}

const MAX_BYTES: usize = 1024 * 1024;
const MAX_USERS: usize = 4096;
pub type SettingsStore = Arc<Mutex<Registry>>;
pub struct Registry {
    path: PathBuf,
    entries: HashMap<String, TerminalSettings>,
    healthy: bool,
    _owner: Flock<File>,
}
struct Entries(HashMap<String, TerminalSettings>);
impl<'de> serde::Deserialize<'de> for Entries {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Entries;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("unique terminal preference accounts")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut a: A,
            ) -> Result<Entries, A::Error> {
                let mut map = HashMap::new();
                while let Some((user, settings)) = a.next_entry::<String, TerminalSettings>()? {
                    if map.len() >= MAX_USERS || map.insert(user, settings).is_some() {
                        return Err(serde::de::Error::custom(
                            "Duplicate account or preference limit exceeded",
                        ));
                    }
                }
                Ok(Entries(map))
            }
        }
        d.deserialize_map(Visitor)
    }
}
fn encode(entries: &HashMap<String, TerminalSettings>) -> io::Result<Vec<u8>> {
    if entries.len() > MAX_USERS
        || entries.iter().any(|(user, s)| {
            user.is_empty()
                || user.len() > 256
                || user.chars().any(char::is_control)
                || !(100..=super::session::MAX_SCROLLBACK).contains(&s.scrollback_lines)
                || !(5..=3600).contains(&s.keepalive_interval_secs)
                || !(8..=28).contains(&s.font_size)
                || s.idle_timeout_secs > super::MAX_TIMEOUT_SECS
                || (s.keepalive && s.idle_timeout_secs != 0)
        })
    {
        return Err(io::Error::other("Invalid terminal preferences"));
    }
    let bytes = serde_json::to_vec(entries).map_err(io::Error::other)?;
    if bytes.len() > MAX_BYTES {
        return Err(io::Error::other(
            "Terminal preferences exceed storage limits",
        ));
    }
    Ok(bytes)
}
fn publish(path: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid preferences path"))?;
    let temporary = parent.join(format!(".terminal-settings-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
pub fn open(path: PathBuf) -> io::Result<SettingsStore> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid preferences path"))?;
    std::fs::create_dir_all(parent)?;
    let established = path.with_extension("lock").exists();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path.with_extension("lock"))?;
    let owner = Flock::lock(file, FlockArg::LockExclusiveNonblock)
        .map_err(|(_, e)| io::Error::other(format!("Terminal preferences already owned: {e}")))?;
    let entries = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(mut file) => {
            if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_BYTES as u64 {
                return Err(io::Error::other("Invalid terminal preferences file"));
            }
            file.sync_all()?;
            File::open(parent)?.sync_all()?;
            let mut bytes = Vec::new();
            Read::by_ref(&mut file)
                .take((MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes)?;
            if bytes.len() > MAX_BYTES {
                return Err(io::Error::other("Terminal preferences exceed size limit"));
            }
            let Entries(entries) = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
            entries
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound && !established => HashMap::new(),
        Err(e) => return Err(e),
    };
    let bytes = encode(&entries)?;
    if !path.exists() {
        publish(&path, &bytes)?;
    }
    Ok(Arc::new(Mutex::new(Registry {
        path,
        entries,
        healthy: true,
        _owner: owner,
    })))
}
impl Registry {
    pub fn get(&self, user: &str) -> io::Result<TerminalSettings> {
        if !self.healthy {
            return Err(io::Error::other(
                "Terminal preferences unavailable; storage recovery required",
            ));
        }
        Ok(self.entries.get(user).cloned().unwrap_or_default())
    }
    pub fn save(&mut self, user: &str, settings: TerminalSettings) -> io::Result<()> {
        if !self.healthy {
            return Err(io::Error::other(
                "Terminal preferences unavailable; storage recovery required",
            ));
        }
        let mut candidate = self.entries.clone();
        candidate.insert(user.to_owned(), settings);
        let bytes = encode(&candidate)?;
        if let Err(e) = publish(&self.path, &bytes) {
            self.healthy = false;
            return Err(e);
        }
        self.entries = candidate;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("nabiman-preferences-{}", uuid::Uuid::new_v4()))
            .join("terminal_settings.json")
    }
    #[test]
    fn private_exclusive_store_preserves_concurrent_accounts_and_restart() {
        use std::os::unix::fs::PermissionsExt;
        let path = path();
        let store = open(path.clone()).unwrap();
        assert!(open(path.clone()).is_err());
        let threads: Vec<_> = (0..12)
            .map(|i| {
                let store = store.clone();
                std::thread::spawn(move || {
                    let mut value = TerminalSettings::default();
                    value.font_size = 8 + i;
                    store
                        .lock()
                        .unwrap()
                        .save(&format!("user{i}"), value)
                        .unwrap();
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(store);
        let store = open(path.clone()).unwrap();
        for i in 0..12 {
            assert_eq!(
                store
                    .lock()
                    .unwrap()
                    .get(&format!("user{i}"))
                    .unwrap()
                    .font_size,
                8 + i
            );
        }
        drop(store);
        std::fs::remove_file(&path).unwrap();
        assert!(open(path.clone()).is_err());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn corrupt_duplicate_and_invalid_settings_are_not_defaults_or_overwritten() {
        let path = path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for bytes in [
            "{broken".to_owned(),
            format!("{{\"user\":{{}},\"user\":{{}}}}"),
            r#"{"user":{"font_size":100}}"#.to_owned(),
        ] {
            std::fs::write(&path, &bytes).unwrap();
            assert!(open(path.clone()).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
        }
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn failed_save_quarantines_reads_and_writes_without_changing_other_accounts() {
        let path = path();
        let store = open(path.clone()).unwrap();
        let mut registry = store.lock().unwrap();
        let mut value = TerminalSettings::default();
        value.font_size = 19;
        registry.save("first", value.clone()).unwrap();
        let mut invalid = value.clone();
        invalid.font_size = 100;
        assert!(registry.save("invalid", invalid).is_err());
        assert_eq!(registry.get("first").unwrap().font_size, 19);
        let before = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(registry.save("second", value).is_err());
        assert!(registry.get("first").is_err());
        assert!(registry.get("missing").is_err());
        assert!(registry.save("third", TerminalSettings::default()).is_err());
        assert_eq!(registry.entries.len(), 1);
        drop(registry);
        drop(store);
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, before).unwrap();
        let recovered = open(path.clone()).unwrap();
        assert_eq!(
            recovered.lock().unwrap().get("first").unwrap().font_size,
            19
        );
        drop(recovered);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
