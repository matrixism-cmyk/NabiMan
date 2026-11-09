//! The tmux-backed session registry: what is running, for whom, and for how
//! long — plus the tmux commands that create and inspect those sessions.

#[path = "tmux_operations.rs"]
mod tmux_operations;
use tmux_operations::write_tmux_conf;
pub(crate) use tmux_operations::{
    capture_scrollback, client_size, create_tmux_session, exact_session,
    pane_preview, paste_into_pane, pipe_pane_record, rename_tmux_window, tmux, tmux_session_exists,
    tmux_session_state,
};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::settings::default_scrollback;
use crate::ssh_auth;

/// Closes the tmux session, then hangs up what ran under its panes.
///
/// Closing a pane hangs up only its own terminal; `su` (and `su --pty`)
/// starts the user's command in a session of its own, so the shell, Claude
/// Code and anything else started there would live on as orphans, as they
/// did for "＋ Claude 세션" tiles. Every process found under the panes before
/// the close gets the SIGHUP a terminal hangup would have sent it: what
/// ignores that (`nohup`) keeps running, as it would anywhere.
pub(crate) fn close_tmux_session(name: &str) -> std::io::Result<()> {
    let under = processes_under(&pane_pids(name));
    tmux_operations::close_tmux_session(name)?;
    if !under.is_empty() {
        std::thread::spawn(move || {
            // Let tmux finish its own hangup first.
            std::thread::sleep(Duration::from_millis(300));
            hang_up(&under);
        });
    }
    Ok(())
}

fn pane_pids(name: &str) -> Vec<u32> {
    let Ok(out) = tmux().args(["list-panes", "-s", "-t", &exact_session(name), "-F", "#{pane_pid}"]).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.trim().parse().ok()).collect()
}

/// Every process below the given ones, with its start time (so a pid reused
/// later is never signalled).
fn processes_under(roots: &[u32]) -> Vec<(u32, String)> {
    if roots.is_empty() {
        return Vec::new();
    }
    let mut children: HashMap<u32, Vec<(u32, String)>> = HashMap::new();
    for entry in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
        let Some((start, parent)) = std::fs::read_to_string(entry.path().join("stat")).ok().and_then(|s| super::claude_peers::stat_fields(&s)) else {
            continue;
        };
        children.entry(parent).or_default().push((pid, start));
    }
    descendants(roots, &children)
}

fn descendants(roots: &[u32], children: &HashMap<u32, Vec<(u32, String)>>) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let mut queue: Vec<u32> = roots.to_vec();
    while let Some(p) = queue.pop() {
        for (c, start) in children.get(&p).into_iter().flatten() {
            if *c > 1 && !out.iter().any(|(o, _)| o == c) {
                out.push((*c, start.clone()));
                queue.push(*c);
            }
        }
    }
    out
}

fn hang_up(procs: &[(u32, String)]) {
    for (pid, start) in procs {
        let now = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok().and_then(|s| super::claude_peers::stat_fields(&s));
        if now.is_some_and(|(s, _)| &s == start) {
            if let Ok(p) = i32::try_from(*pid) {
                // SAFETY: kill(2) with a pid read from /proc whose start time still matches.
                unsafe { libc::kill(p, libc::SIGHUP) };
            }
        }
    }
}

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

#[cfg(test)]
mod hangup_tests {
    use super::*;

    #[test]
    fn a_closed_session_hangs_up_everything_under_its_panes() {
        let mut children: HashMap<u32, Vec<(u32, String)>> = HashMap::new();
        // pane 10 → su 11 → bash 12 → claude 13; another pane's 20 → 21 is left alone.
        children.insert(10, vec![(11, "a".into())]);
        children.insert(11, vec![(12, "b".into())]);
        children.insert(12, vec![(13, "c".into())]);
        children.insert(20, vec![(21, "d".into())]);
        let mut got: Vec<u32> = descendants(&[10], &children).into_iter().map(|(p, _)| p).collect();
        got.sort_unstable();
        assert_eq!(got, vec![11, 12, 13]);
        assert!(processes_under(&[]).is_empty());
    }
}
