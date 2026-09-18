//! The tmux-backed session registry: what is running, for whom, and for how
//! long — plus the tmux commands that create and inspect those sessions.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::settings::default_scrollback;
use crate::ssh_auth;

pub(crate) const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
pub(crate) const MAX_SCROLLBACK: u32 = 200_000;

pub(crate) fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub(crate) fn data_dir() -> String {
    std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".to_string())
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TmuxSession {
    pub(crate) tmux_name: String,
    pub(crate) owner: String,
    pub(crate) label: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) ssh_cmd: Option<String>,
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
}

pub type PtyStore = Arc<Mutex<HashMap<String, TmuxSession>>>;
pub fn new_pty_store() -> PtyStore { Arc::new(Mutex::new(HashMap::new())) }

fn sessions_file() -> String { format!("{}/terminal_sessions.json", data_dir()) }

/// Write the session registry to disk so a server restart does not orphan the
/// tmux sessions the browser still knows about.
pub(crate) fn persist(map: &HashMap<String, TmuxSession>) {
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = std::fs::create_dir_all(data_dir());
        let _ = std::fs::write(sessions_file(), json);
    }
}

/// Re-adopt sessions that survived a restart; drop the ones whose tmux session
/// is gone. Called once at startup.
pub fn restore_sessions(store: &PtyStore) {
    let saved: HashMap<String, TmuxSession> = std::fs::read_to_string(sessions_file())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default();
    let mut map = store.lock().unwrap();
    for (id, mut session) in saved {
        if tmux_session_exists(&session.tmux_name) {
            session.ws_count = 0;
            map.insert(id, session);
        }
    }
    if !map.is_empty() {
        println!("Terminal: restored {} live tmux session(s)", map.len());
        // Mouse and clipboard bindings live on the tmux server, so sessions
        // started by an older build pick up the current ones here.
        if let Some(path) = write_tmux_conf(default_scrollback()) {
            let _ = std::process::Command::new("tmux").args(["source-file", &path]).output();
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
        let expired: Vec<String> = map.iter()
            .filter(|(_, s)| {
                s.ws_count == 0 && s.timeout_secs > 0
                    && now.saturating_sub(s.last_active_unix) > s.timeout_secs
            })
            .map(|(id, _)| id.clone()).collect();
        // A session whose tmux process died (host rebooted, user typed exit)
        // is dropped too, so the session list never shows ghosts.
        let dead: Vec<String> = map.iter()
            .filter(|(_, s)| s.ws_count == 0 && !tmux_session_exists(&s.tmux_name))
            .map(|(id, _)| id.clone()).collect();
        let mut changed = false;
        for id in expired {
            if let Some(s) = map.remove(&id) {
                let _ = std::process::Command::new("tmux").args(["kill-session", "-t", &s.tmux_name]).output();
                changed = true;
            }
        }
        for id in dead {
            map.remove(&id);
            changed = true;
        }
        if changed { persist(&map); }
    });
}

#[allow(dead_code)]
pub fn kill_user_sessions(store: &PtyStore, username: &str) {
    let mut map = store.lock().unwrap();
    let ids: Vec<String> = map.iter().filter(|(_, s)| s.owner == username).map(|(id, _)| id.clone()).collect();
    for id in ids {
        if let Some(s) = map.remove(&id) {
            let _ = std::process::Command::new("tmux").args(["kill-session", "-t", &s.tmux_name]).output();
        }
    }
    persist(&map);
}

pub(crate) fn generate_session_id() -> String {
    use rand::Rng;
    format!("pty_{:016x}", rand::thread_rng().gen::<u64>())
}

pub(crate) fn tmux_session_exists(name: &str) -> bool {
    std::process::Command::new("tmux").args(["has-session", "-t", name])
        .output().map(|o| o.status.success()).unwrap_or(false)
}

/// Options and mouse bindings every NabiMan tmux session runs with.
///
/// The mouse bindings differ from tmux's defaults in one way: a selection is
/// copied *without* leaving copy-mode, so the highlight stays on screen instead
/// of blinking out the moment the button is released. `set-clipboard on` makes
/// tmux hand that copied text to the browser as an OSC 52 sequence.
fn tmux_conf_body(scrollback: u32) -> String {
    // Keeping the highlight means staying in copy-mode, where keystrokes go to
    // copy-mode instead of the shell — so a click of either button leaves it
    // again, which is also what makes a right-click paste land.
    //
    // `copy-pipe-no-clear "tmux load-buffer -w -"` is what actually reaches the
    // browser: it keeps the highlight on screen after the button comes up, and
    // -w makes tmux hand the text over as an OSC 52 sequence, which the
    // frontend turns into a clipboard write. set-clipboard alone does not do
    // it for tmux's own copy commands.
    let copy = r#"send -X copy-pipe-no-clear "tmux load-buffer -w -""#;
    format!(
        "set -g history-limit {scrollback}\n\
         set -g mouse on\n\
         set -g set-clipboard on\n\
         set -as terminal-features ',xterm*:clipboard'\n\
         bind -T copy-mode    MouseDragEnd1Pane {copy}\n\
         bind -T copy-mode-vi MouseDragEnd1Pane {copy}\n\
         bind -T copy-mode    DoubleClick1Pane send -X select-word \\; {copy}\n\
         bind -T copy-mode-vi DoubleClick1Pane send -X select-word \\; {copy}\n\
         bind -T copy-mode    TripleClick1Pane send -X select-line \\; {copy}\n\
         bind -T copy-mode-vi TripleClick1Pane send -X select-line \\; {copy}\n\
         bind -T root DoubleClick1Pane select-pane \\; copy-mode -H \\; send -X select-word \\; {copy}\n\
         bind -T root TripleClick1Pane select-pane \\; copy-mode -H \\; send -X select-line \\; {copy}\n\
         bind -T copy-mode    MouseDown1Pane send -X cancel\n\
         bind -T copy-mode-vi MouseDown1Pane send -X cancel\n\
         bind -T copy-mode    MouseDown3Pane send -X cancel\n\
         bind -T copy-mode-vi MouseDown3Pane send -X cancel\n",
        scrollback = scrollback,
        copy = copy,
    )
}

/// Seed options for a tmux server this process may be about to start.
fn write_tmux_conf(scrollback: u32) -> Option<String> {
    let path = format!("{}/tmux.conf", data_dir());
    std::fs::create_dir_all(data_dir()).ok()?;
    std::fs::write(&path, tmux_conf_body(scrollback)).ok()?;
    Some(path)
}

pub(crate) fn create_tmux_session(name: &str, command: Option<&str>, scrollback: u32, size: (u16, u16)) -> Result<(), String> {
    // history-limit only applies to panes created when they are made, so the
    // value has to be in place *before* the session is spawned. Two paths cover
    // both cases: -f seeds the options when this command starts the tmux server
    // (the first session after a reboot), and set-option -g covers every later
    // session on an already running server.
    let conf = write_tmux_conf(scrollback);
    // -f only applies when this command starts the server, so a server that is
    // already up gets the same file sourced explicitly.
    if let Some(ref path) = conf {
        let _ = std::process::Command::new("tmux").args(["source-file", path]).output();
    }
    let _ = std::process::Command::new("tmux")
        .args(["set-option", "-g", "history-limit", &scrollback.to_string()])
        .output();

    // Create the pane at the size the browser already has: attaching a smaller
    // client would otherwise shrink the pane and push the first lines (an ssh
    // error, for instance) straight out of view.
    let cols = size.0.to_string();
    let rows = size.1.to_string();
    let mut args: Vec<&str> = Vec::new();
    if let Some(ref path) = conf {
        args.push("-f");
        args.push(path);
    }
    args.extend(["new-session", "-d", "-s", name, "-x", &cols, "-y", &rows]);
    if let Some(cmd) = command {
        args.push(cmd);
    }
    let out = std::process::Command::new("tmux").args(&args).output()
        .map_err(|e| format!("tmux: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    // Enable mouse so the wheel scrolls tmux's own history (copy-mode) like a
    // native terminal — otherwise, in tmux's alternate screen, xterm.js turns
    // the wheel into arrow keys, which bash treats as command-history nav.
    let _ = std::process::Command::new("tmux")
        .args(["set-option", "-t", name, "mouse", "on"])
        .output();
    let _ = std::process::Command::new("tmux")
        .args(["set-option", "-t", name, "history-limit", &scrollback.to_string()])
        .output();
    // Detaching a browser must never tear the session down.
    let _ = std::process::Command::new("tmux")
        .args(["set-option", "-t", name, "destroy-unattached", "off"])
        .output();
    Ok(())
}

/// Read back the pane history (ANSI colours included) so a reopened window can
/// show what scrolled past while it was closed.
pub(crate) fn capture_scrollback(name: &str, lines: u32) -> Result<String, String> {
    let start = format!("-{}", lines.min(MAX_SCROLLBACK));
    let out = std::process::Command::new("tmux")
        .args(["capture-pane", "-p", "-e", "-J", "-S", &start, "-t", name])
        .output().map_err(|e| format!("tmux: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
