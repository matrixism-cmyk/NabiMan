//! Durable single-writer terminal metadata. It does not recover unknown OS work.
use super::session::{PtyStore, TmuxSession};
use nix::fcntl::{Flock, FlockArg};
use std::{
    collections::{HashMap, HashSet},
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_SESSIONS: usize = 4096;

struct Entries(HashMap<String, TmuxSession>);
impl<'de> serde::Deserialize<'de> for Entries {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Entries;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("unique terminal session IDs")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut access: A,
            ) -> Result<Self::Value, A::Error> {
                let mut entries = HashMap::new();
                while let Some((id, session)) = access.next_entry::<String, TmuxSession>()? {
                    if entries.contains_key(&id) || entries.len() >= MAX_SESSIONS {
                        return Err(serde::de::Error::custom(
                            "duplicate ID or terminal limit exceeded",
                        ));
                    }
                    entries.insert(id, session);
                }
                Ok(Entries(entries))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

pub struct OwnedRegistry {
    pub store: PtyStore,
    _owner: Flock<File>,
}
fn validate(map: &HashMap<String, TmuxSession>) -> io::Result<()> {
    if map.len() > MAX_SESSIONS {
        return Err(io::Error::other("Terminal registry exceeds session limit"));
    }
    let mut names = HashSet::new();
    for (id, session) in map {
        if !id.starts_with("pty_")
            || id.len() > 128
            || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || !session.tmux_name.starts_with("nb_")
            || session.tmux_name.len() > 256
            || !session
                .tmux_name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_')
            || !names.insert(&session.tmux_name)
            || session.owner.is_empty()
            || session.owner.len() > 128
            || session.owner.chars().any(char::is_control)
            || session.owner_id.len() > 256
            || session.owner_id.chars().any(char::is_control)
            || session.label.is_empty()
            || session.label.len() > 1024
            || session.title.len() > 1024
            || session.timeout_secs > super::MAX_TIMEOUT_SECS
            || !(100..=super::session::MAX_SCROLLBACK).contains(&session.scrollback)
        {
            return Err(io::Error::other(
                "Invalid or ambiguous terminal registry entry",
            ));
        }
    }
    Ok(())
}
fn load(path: &Path) -> io::Result<HashMap<String, TmuxSession>> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(e),
    };
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_BYTES as u64 {
        return Err(io::Error::other("Invalid terminal registry file"));
    }
    file.sync_all()?;
    File::open(
        path.parent()
            .ok_or_else(|| io::Error::other("Invalid registry path"))?,
    )?
    .sync_all()?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(io::Error::other("Terminal registry exceeds size limit"));
    }
    let Entries(map) = serde_json::from_slice(&bytes)
        .map_err(|_| io::Error::other("Invalid terminal registry JSON"))?;
    validate(&map)?;
    Ok(map)
}
pub fn open(path: PathBuf) -> io::Result<OwnedRegistry> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid registry path"))?;
    std::fs::create_dir_all(parent)?;
    let established = path.with_extension("lock").exists();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path.with_extension("lock"))?;
    let owner = Flock::lock(lock, FlockArg::LockExclusiveNonblock).map_err(|(_, error)| {
        io::Error::other(format!("Terminal registry already owned: {error}"))
    })?;
    if established && !path.exists() {
        return Err(io::Error::other(
            "Established terminal registry is missing; restore reviewed metadata",
        ));
    }
    let map = load(&path)?;
    if !path.exists() {
        publish(&path, &map)?;
    }
    Ok(OwnedRegistry {
        store: Arc::new(Mutex::new(map)),
        _owner: owner,
    })
}
fn encode(map: &HashMap<String, TmuxSession>) -> io::Result<Vec<u8>> {
    validate(map)?;
    let bytes = serde_json::to_vec(map).map_err(io::Error::other)?;
    if bytes.len() > MAX_BYTES {
        return Err(io::Error::other("Terminal registry exceeds size limit"));
    }
    Ok(bytes)
}
fn publish(path: &Path, map: &HashMap<String, TmuxSession>) -> io::Result<()> {
    let bytes = encode(map)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid registry path"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".terminal-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(&bytes)?;
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
pub(super) fn prepare_update(
    map: &HashMap<String, TmuxSession>,
    id: &str,
    edit: impl FnOnce(&mut TmuxSession),
) -> io::Result<HashMap<String, TmuxSession>> {
    let mut candidate = map.clone();
    let session = candidate
        .get_mut(id)
        .ok_or_else(|| io::Error::other("Terminal session not found"))?;
    edit(session);
    encode(&candidate)?;
    Ok(candidate)
}
/// Called while holding the store mutex, before creating an OS session. Reserve
/// room for every mutable persisted field so subsequent ordinary changes fit.
pub(super) fn prepare_insert(
    map: &HashMap<String, TmuxSession>,
    id: String,
    session: TmuxSession,
) -> io::Result<HashMap<String, TmuxSession>> {
    if map.contains_key(&id) || map.len() >= MAX_SESSIONS {
        return Err(io::Error::other("Terminal session capacity reached"));
    }
    let mut candidate = map.clone();
    candidate.insert(id, session);
    validate(&candidate)?;
    let mut largest = candidate.clone();
    for session in largest.values_mut() {
        // A JSON control byte occupies six bytes. This bounds all valid titles
        // (at most 1,024 UTF-8 bytes), including legacy metadata.
        session.title = "\0".repeat(1024);
        session.created_unix = u64::MAX;
        session.last_active_unix = u64::MAX;
        session.timeout_secs = super::MAX_TIMEOUT_SECS;
        session.keepalive = false;
        session.shared = false;
    }
    if serde_json::to_vec(&largest)
        .map_err(io::Error::other)?
        .len()
        > MAX_BYTES
    {
        return Err(io::Error::other("Terminal metadata capacity reached"));
    }
    Ok(candidate)
}
pub(super) fn persist(path: &Path, map: &HashMap<String, TmuxSession>) {
    if publish(path, map).is_err() {
        eprintln!("Terminal registry publication failed; terminating server for storage recovery");
        std::process::exit(79);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "nabiman-terminal-registry-{}",
            uuid::Uuid::new_v4()
        ))
    }
    fn sessions() -> HashMap<String, TmuxSession> {
        let session = serde_json::from_value(serde_json::json!({"tmux_name":"nb_admin_pty_one","owner":"admin","label":"local","title":"reviewed","timeout_secs":300})).unwrap();
        HashMap::from([("pty_one".into(), session)])
    }
    #[test]
    fn private_atomic_registry_exclusive_owner_and_roundtrip() {
        use std::os::unix::fs::PermissionsExt;
        let dir = directory();
        let path = dir.join("terminal_sessions.json");
        let owner = open(path.clone()).unwrap();
        assert!(open(path.clone()).is_err());
        publish(&path, &sessions()).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        drop(owner);
        let restored = open(path.clone()).unwrap();
        assert_eq!(restored.store.lock().unwrap()["pty_one"].title, "reviewed");
        drop(restored);
        std::fs::remove_file(&path).unwrap();
        assert!(open(path).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn corrupt_and_ambiguous_metadata_are_preserved_and_rejected() {
        let dir = directory();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("terminal_sessions.json");
        std::fs::write(&path, "{broken").unwrap();
        assert!(open(path.clone()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{broken");
        let entry = serde_json::to_string(&sessions()["pty_one"]).unwrap();
        std::fs::write(
            &path,
            format!("{{\"pty_one\":{entry},\"pty_one\":{entry}}}"),
        )
        .unwrap();
        assert!(open(path.clone()).is_err());
        let mut map = sessions();
        map.insert("pty_alias".into(), map["pty_one"].clone());
        std::fs::write(&path, serde_json::to_vec(&map).unwrap()).unwrap();
        assert!(open(path.clone()).is_err());
        map.remove("pty_alias");
        map.get_mut("pty_one").unwrap().timeout_secs = super::super::MAX_TIMEOUT_SECS + 1;
        assert!(publish(&path, &map).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failed_publication_exits_without_returning_success() {
        const CHILD: &str = "NABIMAN_TEST_TERMINAL_REGISTRY_CHILD";
        if let Some(directory) = std::env::var_os(CHILD) {
            persist(
                &PathBuf::from(directory).join("terminal_sessions.json"),
                &sessions(),
            );
            panic!("Failed persistence returned");
        }
        let dir = directory();
        std::fs::create_dir_all(dir.join("terminal_sessions.json")).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "terminal::registry::tests::failed_publication_exits_without_returning_success",
                "--nocapture",
            ])
            .env(CHILD, &dir)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(79));
        assert!(dir.join("terminal_sessions.json").is_dir());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn admission_rejects_invalid_duplicate_and_full_metadata_without_mutation() {
        let original = sessions();
        let entry = original["pty_one"].clone();
        assert!(prepare_insert(&original, "pty_one".into(), entry.clone()).is_err());
        let mut invalid = entry.clone();
        invalid.tmux_name = "nb_other".into();
        invalid.owner = "a".repeat(129);
        assert!(prepare_insert(&original, "pty_other".into(), invalid).is_err());
        assert_eq!(original.len(), 1);
        assert_eq!(original["pty_one"].title, "reviewed");
        let full = (0..MAX_SESSIONS)
            .map(|i| {
                let mut s = entry.clone();
                s.tmux_name = format!("nb_{i}");
                (format!("pty_{i}"), s)
            })
            .collect();
        assert!(prepare_insert(&full, "pty_extra".into(), entry).is_err());
        assert_eq!(full.len(), MAX_SESSIONS);
    }
    #[test]
    fn admission_reserves_space_for_later_metadata_growth() {
        let mut entry = sessions().remove("pty_one").unwrap();
        entry.title.clear();
        entry.ssh_cmd = Some("x".repeat(MAX_BYTES - 4096));
        let plain = HashMap::from([("pty_one".to_string(), entry.clone())]);
        assert!(serde_json::to_vec(&plain).unwrap().len() < MAX_BYTES);
        assert!(prepare_insert(&HashMap::new(), "pty_one".into(), entry).is_err());
        let candidate = prepare_insert(
            &HashMap::new(),
            "pty_one".into(),
            sessions().remove("pty_one").unwrap(),
        )
        .unwrap();
        assert_eq!(candidate.len(), 1);
    }
    #[test]
    fn update_checks_full_snapshot_and_preserves_original_on_rejection() {
        let mut map = sessions();
        let session = map.get_mut("pty_one").unwrap();
        session.title.clear();
        session.ssh_cmd = Some(String::new());
        let overhead = encode(&map).unwrap().len();
        map.get_mut("pty_one").unwrap().ssh_cmd = Some("x".repeat(MAX_BYTES - overhead));
        assert_eq!(encode(&map).unwrap().len(), MAX_BYTES);
        assert!(prepare_update(&map, "pty_one", |s| s.title = "new".into()).is_err());
        assert!(prepare_update(&map, "pty_one", |s| s.timeout_secs = u64::MAX).is_err());
        assert!(prepare_update(&map, "missing", |_| {}).is_err());
        assert!(map["pty_one"].title.is_empty());
        let smaller = prepare_update(&map, "pty_one", |s| s.shared = true).unwrap();
        assert!(encode(&smaller).unwrap().len() < MAX_BYTES);
        assert!(!map["pty_one"].shared);
    }
}
