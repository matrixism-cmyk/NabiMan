//! Sign-ins, kept on disk so that a restart — every deployment is one — does
//! not sign everyone out.
//!
//! The store is used like a mutex (`sessions.lock_safe()`), but its
//! guard notices any change made through it and saves the whole set before
//! the lock is let go. A sign-out or a revocation is therefore on disk the
//! moment it takes effect, and a restart can never bring a revoked sign-in
//! back. Sign-ins older than the refresh token's lifetime are dropped.
//!
//! The one exception is a sign-in's last activity ([`SessionGuard::touch`]),
//! which every page reports: it is saved at most once a minute (and with
//! any other change), so a restart may see it up to that much older.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LockResult, Mutex, MutexGuard, PoisonError, TryLockError, TryLockResult};

use serde::{Deserialize, Serialize};

use super::{SessionInfo, REFRESH_TTL};

type Map = HashMap<String, SessionInfo>;

/// How often activity alone causes a save, in seconds.
const ACTIVITY_SAVE_SECS: i64 = 60;

#[derive(Clone)]
pub struct SessionStore {
    map: Arc<Mutex<Map>>,
    /// Where the sign-ins are kept; none for a store that lives in memory (tests).
    file: Option<Arc<PathBuf>>,
    /// When the sign-ins were last saved (Unix seconds).
    saved_at: Arc<AtomicI64>,
}

/// The locked sign-ins; saved when let go, if they were changed.
pub struct SessionGuard<'a> {
    guard: MutexGuard<'a, Map>,
    file: Option<&'a Path>,
    changed: bool,
    saved_at: &'a AtomicI64,
}

impl SessionGuard<'_> {
    /// The sign-in `sid` was used at `now`. Kept in memory at once; saved only
    /// if the last save is a minute old, or with the next real change.
    pub fn touch(&mut self, sid: &str, now: i64) {
        if let Some(session) = self.guard.get_mut(sid) {
            session.last_activity = now;
            if now - self.saved_at.load(Ordering::Relaxed) >= ACTIVITY_SAVE_SECS {
                self.changed = true;
                self.saved_at.store(now, Ordering::Relaxed);
            }
        }
    }
}

impl Deref for SessionGuard<'_> {
    type Target = Map;
    fn deref(&self) -> &Map {
        &self.guard
    }
}

impl DerefMut for SessionGuard<'_> {
    fn deref_mut(&mut self) -> &mut Map {
        self.changed = true;
        &mut self.guard
    }
}

impl Drop for SessionGuard<'_> {
    fn drop(&mut self) {
        if let (true, Some(file)) = (self.changed, self.file) {
            prune(&mut self.guard);
            if let Err(e) = save(&self.guard, file) {
                eprintln!("[sessions] cannot save sign-ins to {}: {e}", file.display());
            }
            self.saved_at.fetch_max(chrono::Utc::now().timestamp(), Ordering::Relaxed);
        }
    }
}

/// On disk, a sign-in keeps what the API leaves out: its refresh generation
/// (so an old refresh token stays spent) and whether it passed MFA.
#[derive(Serialize, Deserialize)]
struct Stored {
    session_id: String,
    username: String,
    user_id: String,
    created_at: String,
    last_active: String,
    ip: String,
    refresh_generation: u64,
    mfa_verified: bool,
    #[serde(default)]
    last_activity: i64,
}

impl From<&SessionInfo> for Stored {
    fn from(s: &SessionInfo) -> Self {
        Stored {
            session_id: s.session_id.clone(), username: s.username.clone(), user_id: s.user_id.clone(),
            created_at: s.created_at.clone(), last_active: s.last_active.clone(), ip: s.ip.clone(),
            refresh_generation: s.refresh_generation, mfa_verified: s.mfa_verified, last_activity: s.last_activity,
        }
    }
}

impl From<Stored> for SessionInfo {
    fn from(s: Stored) -> Self {
        SessionInfo {
            session_id: s.session_id, username: s.username, user_id: s.user_id,
            created_at: s.created_at, last_active: s.last_active, ip: s.ip,
            refresh_generation: s.refresh_generation, mfa_verified: s.mfa_verified,
            // A sign-in saved before activity was kept counts as used now.
            last_activity: if s.last_activity > 0 { s.last_activity } else { chrono::Utc::now().timestamp() },
        }
    }
}

/// Whether a sign-in has outlived its refresh token (or its date is unreadable).
fn expired(s: &SessionInfo, now: chrono::NaiveDateTime) -> bool {
    chrono::NaiveDateTime::parse_from_str(&s.created_at, "%Y-%m-%d %H:%M:%S")
        .map(|at| (now - at).num_seconds() > REFRESH_TTL as i64)
        .unwrap_or(true)
}

fn prune(map: &mut Map) {
    let now = chrono::Utc::now().naive_utc();
    map.retain(|_, s| !expired(s, now));
}

fn save(map: &Map, file: &Path) -> std::io::Result<()> {
    let list: Vec<Stored> = map.values().map(Stored::from).collect();
    let json = serde_json::to_vec(&list).map_err(std::io::Error::other)?;
    crate::private_storage::write(file, &json)
}

fn load(file: &Path) -> Map {
    let Ok(bytes) = std::fs::read(file) else { return Map::new() };
    let list: Vec<Stored> = match serde_json::from_slice(&bytes) {
        Ok(list) => list,
        Err(e) => {
            eprintln!("[sessions] {} is unreadable ({e}); everyone signs in again", file.display());
            return Map::new();
        }
    };
    let mut map: Map = list.into_iter().map(|s| (s.session_id.clone(), SessionInfo::from(s))).collect();
    prune(&mut map);
    map
}

impl SessionStore {
    /// Sign-ins that last only as long as the process (tests).
    pub fn in_memory() -> Self {
        SessionStore { map: Arc::new(Mutex::new(Map::new())), file: None, saved_at: Arc::default() }
    }

    /// Sign-ins kept in `file`, with those already there taken up again.
    pub fn open(file: PathBuf) -> Self {
        let map = load(&file);
        let saved_at = Arc::new(AtomicI64::new(chrono::Utc::now().timestamp()));
        SessionStore { map: Arc::new(Mutex::new(map)), file: Some(Arc::new(file)), saved_at }
    }

    pub fn lock(&self) -> LockResult<SessionGuard<'_>> {
        let file = self.file.as_deref().map(PathBuf::as_path);
        let saved_at = &*self.saved_at;
        let wrap = |guard| SessionGuard { guard, file, changed: false, saved_at };
        match self.map.lock() {
            Ok(guard) => Ok(wrap(guard)),
            Err(poisoned) => Err(PoisonError::new(wrap(poisoned.into_inner()))),
        }
    }

    /// The sign-ins, taken up again if an earlier holder panicked: the map is
    /// whole between statements, so one bad request cannot sign everyone out.
    pub fn lock_safe(&self) -> SessionGuard<'_> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn try_lock(&self) -> TryLockResult<SessionGuard<'_>> {
        let file = self.file.as_deref().map(PathBuf::as_path);
        let saved_at = &*self.saved_at;
        let wrap = |guard| SessionGuard { guard, file, changed: false, saved_at };
        match self.map.try_lock() {
            Ok(guard) => Ok(wrap(guard)),
            Err(TryLockError::Poisoned(p)) => Err(TryLockError::Poisoned(PoisonError::new(wrap(p.into_inner())))),
            Err(TryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(sid: &str, created_at: &str) -> SessionInfo {
        SessionInfo {
            session_id: sid.into(), username: "admin".into(), user_id: "u1".into(), created_at: created_at.into(),
            last_active: created_at.into(), ip: "127.0.0.1".into(), refresh_generation: 3, mfa_verified: true,
            last_activity: 0,
        }
    }

    /// A fresh directory for one test, gone with it.
    struct Dir(PathBuf);
    impl Dir {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("nabiman-sessions-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Dir(p)
        }
        fn path(&self) -> &Path { &self.0 }
    }
    impl Drop for Dir {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }

    fn now() -> String {
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }

    #[test]
    fn sign_ins_outlive_a_restart_with_everything_they_need() {
        let dir = Dir::new();
        let file = dir.path().join("login_sessions.json");
        let store = SessionStore::open(file.clone());
        store.lock().unwrap().insert("s1".into(), info("s1", &now()));
        let again = SessionStore::open(file.clone());
        let map = again.lock().unwrap();
        let s = map.get("s1").expect("kept");
        assert_eq!((s.refresh_generation, s.mfa_verified, s.username.as_str()), (3, true, "admin"));
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn a_sign_out_is_on_disk_at_once_and_stays_out() {
        let dir = Dir::new();
        let file = dir.path().join("login_sessions.json");
        let store = SessionStore::open(file.clone());
        store.lock().unwrap().insert("s1".into(), info("s1", &now()));
        store.lock().unwrap().insert("s2".into(), info("s2", &now()));
        store.lock().unwrap().remove("s1");
        store.lock().unwrap().retain(|_, s| s.session_id != "s2");
        assert!(SessionStore::open(file).lock().unwrap().is_empty());
    }

    #[test]
    fn reading_saves_nothing_and_old_sign_ins_are_dropped() {
        let dir = Dir::new();
        let file = dir.path().join("login_sessions.json");
        let store = SessionStore::open(file.clone());
        assert!(store.lock().unwrap().get("x").is_none());
        assert!(!file.exists(), "a read does not write");
        store.lock().unwrap().insert("old".into(), info("old", "2020-01-01 00:00:00"));
        store.lock().unwrap().insert("new".into(), info("new", &now()));
        let again = SessionStore::open(file);
        let keys: Vec<String> = again.lock().unwrap().keys().cloned().collect();
        assert_eq!(keys, vec!["new".to_string()]);
    }

    #[test]
    fn a_damaged_file_means_signing_in_again() {
        let dir = Dir::new();
        let file = dir.path().join("login_sessions.json");
        std::fs::write(&file, b"{not json").unwrap();
        assert!(SessionStore::open(file).lock().unwrap().is_empty());
    }

    #[test]
    fn a_panic_while_holding_the_sign_ins_does_not_sign_everyone_out() {
        let dir = Dir::new();
        let file = dir.path().join("login_sessions.json");
        let store = SessionStore::open(file.clone());
        let held = store.clone();
        let _ = std::thread::spawn(move || {
            let mut map = held.lock_safe();
            map.insert("s1".into(), info("s1", &now()));
            panic!("a handler panicked mid-request");
        })
        .join();
        assert!(store.lock().is_err(), "the mutex is poisoned");
        store.lock_safe().insert("s2".into(), info("s2", &now()));
        let again = SessionStore::open(file);
        assert_eq!(again.lock_safe().len(), 2, "both sign-ins were saved");
    }

    #[test]
    fn activity_is_saved_at_most_once_a_minute_but_never_lost_to_a_change() {
        let dir = Dir::new();
        let file = dir.path().join("login_sessions.json");
        let store = SessionStore::open(file.clone());
        store.lock_safe().insert("s1".into(), info("s1", &now()));
        let on_disk = || SessionStore::open(file.clone()).lock_safe()["s1"].last_activity;
        let t = chrono::Utc::now().timestamp();
        store.lock_safe().touch("s1", t + 5);
        assert_ne!(on_disk(), t + 5, "a touch within the minute is not written");
        assert_eq!(store.lock_safe()["s1"].last_activity, t + 5, "but counts at once");
        store.lock_safe().touch("s1", t + ACTIVITY_SAVE_SECS + 1);
        assert_eq!(on_disk(), t + ACTIVITY_SAVE_SECS + 1, "a minute on, it is");
        store.lock_safe().touch("s1", t + ACTIVITY_SAVE_SECS + 2);
        store.lock_safe().insert("s2".into(), info("s2", &now()));
        assert_eq!(on_disk(), t + ACTIVITY_SAVE_SECS + 2, "a real change saves pending activity");
    }
}
