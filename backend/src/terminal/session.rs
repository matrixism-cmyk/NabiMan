//! The tmux-backed session registry: what is running, for whom, and for how
//! long — plus the tmux commands that create and inspect those sessions.

#[path = "tmux_operations.rs"]
mod tmux_operations;
use tmux_operations::write_tmux_conf;
pub(crate) use tmux_operations::{
    capture_scrollback, client_size, close_tmux_session, create_tmux_session, exact_session,
    pane_preview, paste_into_pane, rename_tmux_window, tmux, tmux_session_exists,
    tmux_session_state,
};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::settings::default_scrollback;
use crate::ssh_auth;

pub(crate) const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
pub(crate) const MAX_SCROLLBACK: u32 = 200_000;

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(crate) fn data_dir() -> String {
    nabiman_server::data_dir().to_string_lossy().into_owned()
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TmuxSession {
    pub(crate) tmux_name: String,
    pub(crate) owner: String,
    /// Display names are historical metadata, never proof of ownership.
    #[serde(default)]
    pub(crate) owner_id: String,
    pub(crate) label: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) ssh_cmd: Option<String>,
    #[serde(default)]
    pub(crate) remote_binding: Option<super::remote_access::RemoteBinding>,
    #[serde(default = "now_unix")]
    pub(crate) created_unix: u64,
    #[serde(default = "now_unix")]
    pub(crate) last_active_unix: u64,
    /// Live attachments. Not persisted — a restart detaches every browser.
    #[serde(skip)]
    pub(crate) ws_count: u32,
    #[serde(default)]
    pub(crate) shared: bool,
    /// 0 = permanent (keep-alive): the session is never reaped while detached.
    #[serde(default)]
    pub(crate) timeout_secs: u64,
    #[serde(default = "default_scrollback")]
    pub(crate) scrollback: u32,
    #[serde(default)]
    pub(crate) keepalive: bool,
    /// A name the operator gave this session; the label stays as its address.
    #[serde(default)]
    pub(crate) title: String,
}

pub type PtyStore = Arc<Mutex<HashMap<String, TmuxSession>>>;
impl TmuxSession {
    pub(crate) fn owned_by(&self, user_id: &str) -> bool {
        !self.owner_id.is_empty() && !user_id.is_empty() && self.owner_id == user_id
    }
}
pub(crate) fn visible_to(
    session: &TmuxSession,
    user_id: &str,
    users: &crate::users::UserStore,
) -> bool {
    !user_id.is_empty()
        && !session.owner_id.is_empty()
        && (session.owned_by(user_id) || session.shared)
        && users.lock().ok().is_some_and(|m| {
            !m.iter().any(|u| {
                u.id == user_id && !super::remote_access::member_may_use(u, session)
            }) && m.iter().any(|u| {
                u.id == session.owner_id
                    && crate::scope_access::permission(u, "GET", "/api/terminal")
            })
        })
}

#[cfg(test)]
#[path = "session_ownership_tests.rs"]
mod ownership_tests;

fn sessions_file() -> String {
    format!("{}/terminal_sessions.json", data_dir())
}

/// Write the session registry to disk so a server restart does not orphan the
/// tmux sessions the browser still knows about.
pub(crate) fn persist(map: &HashMap<String, TmuxSession>) {
    super::registry::persist(std::path::Path::new(&sessions_file()), map);
}

/// Re-adopt sessions that survived a restart; drop the ones whose tmux session
/// is gone. Called once at startup.
pub fn restore_sessions(store: &PtyStore) {
    let mut map = store.lock().unwrap();
    map.retain(|_, session| {
        session.ws_count = 0;
        // An unavailable tmux command is not proof that the session vanished.
        tmux_session_state(&session.tmux_name).unwrap_or(true)
    });
    if !map.is_empty() {
        println!(
            "Terminal: retained {} registered tmux session(s)",
            map.len()
        );
        // Mouse and clipboard bindings live on the tmux server, so sessions
        // started by an older build pick up the current ones here.
        if let Some(path) = write_tmux_conf(default_scrollback()) {
            let _ = tmux().args(["source-file", &path]).output();
        }
    }
    persist(&map);
    ssh_auth::cleanup_password_files();
}

pub fn start_pty_cleanup(store: PtyStore) {
    std::thread::spawn(move || loop {
        std::thread::sleep(CLEANUP_INTERVAL);
        let mut map = store.lock().unwrap();
        let now = now_unix();
        let expired: Vec<String> = map
            .iter()
            .filter(|(_, s)| {
                s.ws_count == 0
                    && s.timeout_secs > 0
                    && now.saturating_sub(s.last_active_unix) > s.timeout_secs
            })
            .map(|(id, _)| id.clone())
            .collect();
        // A session whose tmux process died (host rebooted, user typed exit)
        // is dropped too, so the session list never shows ghosts.
        let dead: Vec<String> = map
            .iter()
            .filter(|(_, s)| {
                s.ws_count == 0 && matches!(tmux_session_state(&s.tmux_name), Ok(false))
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut changed = false;
        for id in expired {
            if let Some(s) = map.get(&id) {
                if close_tmux_session(&s.tmux_name).is_ok() {
                    map.remove(&id);
                    changed = true;
                }
            }
        }
        for id in dead {
            map.remove(&id);
            changed = true;
        }
        if changed {
            persist(&map);
        }
    });
}

#[allow(dead_code)]
pub fn kill_user_sessions(store: &PtyStore, user_id: &str) {
    let mut map = store.lock().unwrap();
    let ids: Vec<String> = map
        .iter()
        .filter(|(_, s)| s.owned_by(user_id))
        .map(|(id, _)| id.clone())
        .collect();
    for id in ids {
        if let Some(s) = map.get(&id) {
            if close_tmux_session(&s.tmux_name).is_ok() {
                map.remove(&id);
            }
        }
    }

    persist(&map);
}

pub(crate) fn generate_session_id() -> String {
    use rand::Rng;
    format!("pty_{:016x}", rand::thread_rng().gen::<u64>())
}

impl crate::stream_access::ConnectionGrant {
    /// Whether this account may see `session` (its own, or one shared with it).
    pub(crate) fn can_access_session(&self, session: &TmuxSession) -> bool {
        visible_to(session, &self.user_id, &self.users)
    }
}
