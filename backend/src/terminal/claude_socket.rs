//! A local door for programs — Claude Code itself, mostly — to open a new
//! NabiMan session that starts Claude Code in a directory, the way an operator
//! does from the browser's "＋ Claude 세션".
//!
//! It is a Unix socket on this machine only. Who is asking comes from the
//! kernel (`SO_PEERCRED`), not from anything the caller says, so there is no
//! password or key to keep: a caller starts sessions as itself (root may name
//! any user), and only from inside a NabiMan session, whose NabiMan account
//! then owns the new one. One JSON line in, one JSON line out.
//!
//!   {"dir": "/workspace/x", "title": "x", "resume": true, "skip_permissions": true, "prompt": "…"}
//!   → {"ok": true, "session_id": "pty_…", "title": "x", "claude": "x-12 [ab12cd]"}

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::claude_start::{self, Start};
use super::session::{self, PtyStore, TmuxSession};

pub(crate) const SOCKET: &str = "/run/nabiman/claude.sock";

/// The socket path; `NABIMAN_CLAUDE_SOCKET` moves it (a test server beside the real one).
fn socket_path() -> String {
    std::env::var("NABIMAN_CLAUDE_SOCKET").ok().filter(|p| p.starts_with('/')).unwrap_or_else(|| SOCKET.to_string())
}
const MAX_REQUEST: u64 = 16 * 1024;
const MAX_PROMPT: usize = 8 * 1024;
const SCROLLBACK: u32 = 5000;
/// How long to wait for the new session's Claude Code to show its address.
const ADDRESS_WAIT: Duration = Duration::from_secs(25);

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
struct Ask {
    /// List the Claude sessions NabiMan knows instead of opening one.
    #[serde(default)]
    list: bool,
    #[serde(default)]
    dir: String,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default = "yes")]
    resume: bool,
    #[serde(default = "yes")]
    skip_permissions: bool,
    #[serde(default)]
    prompt: Option<String>,
    /// Wait for the new Claude Code's address (`name [ref]`) before answering.
    #[serde(default = "yes")]
    wait: bool,
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Debug, Default)]
struct Answer {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    /// The new session's Claude Code address, ready for SendMessage.
    #[serde(skip_serializing_if = "Option::is_none")]
    claude: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// For `list`: every Claude session NabiMan knows of.
    #[serde(skip_serializing_if = "Option::is_none")]
    sessions: Option<Vec<Listed>>,
}

/// One Claude session as `list` gives it.
#[derive(Serialize, Debug, Clone, PartialEq)]
struct Listed {
    /// What SendMessage's `to` takes: `name [ref]` here, `bridge:<session>`
    /// on another machine (or just the name, without Remote Control there).
    address: String,
    name: String,
    /// The saved server it runs on; `null` for this machine.
    server: Option<String>,
    cwd: String,
    busy: bool,
    /// The NabiMan tile it runs in, by name; `null` for one at a remote desktop.
    tile: Option<String>,
    /// Whether a message can reach it from here (Remote Control on, or same machine).
    reachable: bool,
}

fn refuse(why: impl Into<String>) -> Answer {
    Answer { ok: false, error: Some(why.into()), ..Default::default() }
}

/// Opens the socket and serves it on a thread of its own.
pub(crate) fn start(store: PtyStore) {
    let path = socket_path();
    let dir = Path::new(&path).parent().unwrap_or(Path::new("/run"));
    if std::fs::create_dir_all(dir).is_err() {
        eprintln!("claude socket: cannot make {}", dir.display());
        return;
    }
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("claude socket: {e}");
            return;
        }
    };
    // Anyone on this machine may knock; the kernel says who they are.
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666));
    std::thread::Builder::new()
        .name("claude-socket".into())
        .spawn(move || {
            for conn in listener.incoming().flatten() {
                let store = store.clone();
                let _ = std::thread::Builder::new().name("claude-socket-conn".into()).spawn(move || serve(conn, &store));
            }
        })
        .ok();
}

fn serve(mut conn: UnixStream, store: &PtyStore) {
    let _ = conn.set_read_timeout(Some(Duration::from_secs(10)));
    let answer = match peer(&conn) {
        Some((uid, pid)) => {
            let mut line = String::new();
            let read = BufReader::new(std::io::Read::take(&conn, MAX_REQUEST)).read_line(&mut line);
            match read {
                Ok(_) => match serde_json::from_str::<Ask>(line.trim()) {
                    Ok(ask) => open(ask, uid, pid, store),
                    Err(e) => refuse(format!("bad request: {e}")),
                },
                Err(e) => refuse(format!("read: {e}")),
            }
        }
        None => refuse("cannot tell who is asking"),
    };
    let mut text = serde_json::to_string(&answer).unwrap_or_else(|_| "{\"ok\":false}".into());
    text.push('\n');
    let _ = conn.write_all(text.as_bytes());
}

/// Who is on the other end, as the kernel recorded it: (uid, pid).
fn peer(conn: &UnixStream) -> Option<(u32, u32)> {
    use std::os::fd::AsRawFd;
    let mut cred = libc::ucred { pid: 0, uid: 0, gid: 0 };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: a valid socket fd and a ucred-sized buffer with its length.
    let rc = unsafe {
        libc::getsockopt(conn.as_raw_fd(), libc::SOL_SOCKET, libc::SO_PEERCRED, &mut cred as *mut _ as *mut libc::c_void, &mut len)
    };
    (rc == 0 && cred.pid > 0).then_some((cred.uid, cred.pid as u32))
}

/// The user a uid is, among those sessions may run as.
fn user_of(uid: u32) -> Option<String> {
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    let name = passwd.lines().find_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.len() > 2 && f[2].parse::<u32>().ok() == Some(uid)).then(|| f[0].to_string())
    })?;
    claude_start::accounts().iter().any(|a| a.name == name).then_some(name)
}

/// Who may run as whom: oneself, or anyone when root.
fn run_as(caller: &str, asked: Option<&str>) -> Result<String, String> {
    match asked {
        None => Ok(caller.to_string()),
        Some(u) if u == caller || caller == "root" => Ok(u.to_string()),
        Some(u) => Err(format!("{caller} cannot start a session as {u}")),
    }
}

/// Every Claude session NabiMan knows of, here and on the servers its tiles
/// are connected to. For callers inside a NabiMan session, like opening one.
fn list(pid: u32, store: &PtyStore) -> Answer {
    if super::claude_peers::tmux_session_of_pid(pid).is_none() {
        return refuse("only from inside a NabiMan session");
    }
    super::claude_remote::want(store);
    // Right after a quiet spell the remote servers have not been looked at yet.
    let until = Instant::now() + Duration::from_secs(20);
    while super::claude_remote::pending() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(500));
    }
    let titles: std::collections::HashMap<String, String> = store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .map(|s| (s.tmux_name.clone(), if s.title.is_empty() { s.label.clone() } else { s.title.clone() }))
        .collect();
    let mut out: Vec<Listed> = Vec::new();
    for (card, seen) in super::claude_peers::look().iter() {
        for p in &seen.claude {
            out.push(Listed {
                address: p.address.clone(),
                name: p.name.clone(),
                server: p.server.clone(),
                cwd: p.cwd.clone(),
                busy: p.busy,
                tile: titles.get(card).cloned(),
                reachable: p.server.is_none() || p.address.starts_with("bridge:"),
            });
        }
    }
    out.sort_by(|a, b| (&a.server, &a.name).cmp(&(&b.server, &b.name)));
    Answer { ok: true, sessions: Some(out), ..Default::default() }
}

fn open(ask: Ask, uid: u32, pid: u32, store: &PtyStore) -> Answer {
    let Some(caller) = user_of(uid) else {
        return refuse("this user cannot start NabiMan sessions");
    };
    if ask.list {
        return list(pid, store);
    }
    let user = match run_as(&caller, ask.user.as_deref()) {
        Ok(u) => u,
        Err(e) => return refuse(e),
    };
    if !claude_start::valid_user(&user) || !claude_start::accounts().iter().any(|a| a.name == user) {
        return refuse(format!("unknown user {user}"));
    }
    let dir = ask.dir.trim().to_string();
    if !dir.starts_with('/') || dir.len() > 1024 || dir.chars().any(char::is_control) || !Path::new(&dir).is_dir() {
        return refuse(format!("no such directory: {dir}"));
    }
    if ask.prompt.as_ref().is_some_and(|p| p.len() > MAX_PROMPT) {
        return refuse("the first message is too long");
    }
    // The NabiMan session the caller runs in: its account owns the new one.
    let Some(parent_tmux) = super::claude_peers::tmux_session_of_pid(pid) else {
        return refuse("only from inside a NabiMan session");
    };
    let title: String = ask
        .title
        .unwrap_or_else(|| dir.rsplit('/').find(|p| !p.is_empty()).unwrap_or("claude").to_string())
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect();
    let start = Start {
        user,
        dir,
        skip_permissions: ask.skip_permissions,
        resume: ask.resume,
        title: title.clone(),
        prompt: ask.prompt.filter(|p| !p.trim().is_empty()),
        root_shell_after: false,
    };

    let (sid, tmux_name) = {
        let mut map = store.lock().unwrap_or_else(|e| e.into_inner());
        let Some(parent) = map.values().find(|s| s.tmux_name == parent_tmux).cloned() else {
            return refuse("the calling session is not one of NabiMan's");
        };
        if parent.ssh_cmd.is_some() || parent.remote_binding.is_some() || parent.label != "local" {
            return refuse("only from a local NabiMan session");
        }
        let sid = session::generate_session_id();
        let safe_user: String = parent.owner.chars().filter(|c| c.is_alphanumeric() || *c == '_').take(64).collect();
        let tmux_name = format!("nb_{}_{}", safe_user, sid);
        let candidate = match super::registry::prepare_insert(
            &map,
            sid.clone(),
            TmuxSession {
                tmux_name: tmux_name.clone(),
                owner: parent.owner.clone(),
                owner_id: parent.owner_id.clone(),
                label: "local".into(),
                ssh_cmd: None,
                remote_binding: None,
                created_unix: session::now_unix(),
                last_active_unix: session::now_unix(),
                ws_count: 0,
                shared: false,
                timeout_secs: 0,
                scrollback: SCROLLBACK,
                keepalive: true,
                title,
            },
        ) {
            Ok(c) => c,
            Err(_) => return refuse("terminal capacity reached"),
        };
        let size = (200, 50);
        if let Err(e) = session::create_tmux_session(&tmux_name, Some(&claude_start::command(&start)), SCROLLBACK, size) {
            return refuse(format!("tmux: {e}"));
        }
        *map = candidate;
        session::persist(&map);
        if let Some(s) = map.get(&sid) {
            super::recording::start(&tmux_name, &sid, &s.owner, &s.owner_id, &s.label, size);
        }
        (sid, tmux_name)
    };
    let claude = if ask.wait { wait_for_address(&tmux_name) } else { None };
    Answer { ok: true, session_id: Some(sid), title: Some(start.title), claude, error: None, sessions: None }
}

/// The new session's Claude Code address, once it has one.
fn wait_for_address(tmux_name: &str) -> Option<String> {
    let until = Instant::now() + ADDRESS_WAIT;
    while Instant::now() < until {
        std::thread::sleep(Duration::from_millis(1500));
        if let Some(peer) = super::claude_peers::look().get(tmux_name).and_then(|s| s.claude.first().cloned()) {
            return Some(peer.address);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callers_run_as_themselves_and_only_root_may_name_another() {
        assert_eq!(run_as("nabi", None).unwrap(), "nabi");
        assert_eq!(run_as("nabi", Some("nabi")).unwrap(), "nabi");
        assert!(run_as("nabi", Some("root")).is_err());
        assert!(run_as("nabi", Some("tooktak")).is_err());
        assert_eq!(run_as("root", Some("tooktak")).unwrap(), "tooktak");
    }

    #[test]
    fn a_request_is_one_strict_json_line_with_safe_defaults() {
        let ask: Ask = serde_json::from_str(r#"{"dir":"/w"}"#).unwrap();
        assert!(ask.resume && ask.skip_permissions && ask.wait && ask.user.is_none() && ask.prompt.is_none() && !ask.list);
        assert!(serde_json::from_str::<Ask>(r#"{"list":true}"#).unwrap().list);
        assert!(serde_json::from_str::<Ask>(r#"{"dir":"/w","command":"rm -rf /"}"#).is_err(), "no extra fields");
    }
}
