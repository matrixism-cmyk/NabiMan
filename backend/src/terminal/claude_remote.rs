//! Claude Code on the servers NabiMan's SSH tiles are connected to.
//!
//! Each SSH tile shares its connection (OpenSSH ControlMaster, a socket per
//! session). While someone is looking at the session list, a short probe runs
//! over that same connection every little while: no new login, and whatever
//! the network in between, it lands under the very sshd the tile's shell is
//! under. That is how a Claude Code session there is known to be the tile's —
//! its process descends from the same connection. Sessions on that server
//! that are not under any tile (started at its desktop, say) are listed too.
//!
//! The probe sends back session records and, from their transcripts, only who
//! sent each peer message, when, and its id; message text stays where it is.
use crate::lock_safe::LockExt;

#[path = "claude_remote_probe.rs"]
mod probe;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use probe::{parse, run, Os};

use super::session::PtyStore;

/// How often the servers are looked at, while anyone is watching.
const EVERY: Duration = Duration::from_secs(15);
/// Nobody has asked for this long: stop probing.
const IDLE_AFTER: Duration = Duration::from_secs(90);
const MAX_KEPT: usize = 20000;

/// Where each SSH tile's shared-connection socket lives.
pub(crate) fn mux_dir() -> PathBuf {
    nabiman_server::data_dir().join("ssh").join("mux")
}

pub(crate) fn mux_path(session_id: &str) -> PathBuf {
    mux_dir().join(session_id)
}

/// ssh options that make a tile's connection shareable. ssh appends a random
/// suffix to the path while it sets up, so the whole stays under the 108-byte
/// limit for Unix sockets with our short names.
pub(crate) fn mux_options(session_id: &str) -> Option<String> {
    let dir = mux_dir();
    private_dir(&dir)?;
    let path = mux_path(session_id).to_string_lossy().into_owned();
    (path.len() + 20 < 104 && session_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .then(|| format!("-o ControlMaster=yes -o ControlPath={path} -o ControlPersist=no"))
}

/// Makes `dir` this process's own, mode 0700, or gives up: whoever could
/// write there could stand in for a tile's connection. (The container's start
/// hands all of /workspace to another user, so it is checked every time.)
fn private_dir(dir: &std::path::Path) -> Option<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    std::fs::create_dir_all(dir).ok()?;
    // SAFETY: geteuid has no preconditions.
    let me = unsafe { libc::geteuid() };
    if std::fs::metadata(dir).ok()?.uid() != me {
        std::os::unix::fs::chown(dir, Some(me), None).ok()?;
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).ok()?;
    let meta = std::fs::metadata(dir).ok()?;
    (meta.uid() == me && meta.mode() & 0o077 == 0).then_some(())
}

/// One Claude Code session on a remote server.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RemoteClaude {
    pub server_id: String,
    pub name: String,
    pub cwd: String,
    pub busy: bool,
    /// When `busy` last changed (seconds since 1970), if the record says.
    pub since: Option<u64>,
    /// The model that last answered in it, and its Claude Code version.
    pub model: Option<String>,
    pub version: String,
    /// Its messaging socket (or pipe) on that machine.
    pub sock: String,
    /// Its Remote Control session, when that is on.
    pub bridge: Option<String>,
    pub session_id: String,
    /// Its process on that machine: one conversation may be open in several
    /// (`claude -c` in two terminals), each its own Claude.
    pub pid: u32,
    /// The NabiMan session (tmux name) whose connection it runs under.
    pub tile: Option<String>,
}

impl RemoteClaude {
    /// One running Claude: its conversation and its process.
    pub(crate) fn key(&self) -> String {
        format!("{}:{}", self.session_id, self.pid)
    }
}

/// A peer message a remote session received: (server, receiver's session id,
/// from, when, the first line it said).
pub(crate) type RemoteMessage = (String, String, String, u64, String);

#[derive(Clone, Debug, Default)]
pub(crate) struct Snapshot {
    pub peers: Vec<RemoteClaude>,
    pub messages: Vec<RemoteMessage>,
    /// Server id → its name in NabiMan.
    pub servers: HashMap<String, String>,
    /// Tmux session → agent kinds running under its connection.
    pub agents: HashMap<String, Vec<String>>,
}

#[derive(Default)]
struct Server {
    /// Session id → the session, from the last look.
    peers: HashMap<String, RemoteClaude>,
    offsets: HashMap<String, u64>,
    got: Vec<(String, String, u64, String)>,
    ids: HashSet<String>,
}

#[derive(Default)]
struct State {
    servers: HashMap<String, Server>,
    /// Tmux session → the agent kinds seen under its connection at the last look.
    agents: HashMap<String, Vec<String>>,
    names: HashMap<String, String>,
    os: HashMap<String, Os>,
    /// Servers looked at (answered or not) since they gained a tile.
    tried: HashSet<String>,
    wanted: Option<Instant>,
    store: Option<PtyStore>,
    running: bool,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}

/// Someone is looking at the sessions: keep the remote view fresh for a while.
pub(crate) fn want(store: &PtyStore) {
    let mut s = state().lock_safe();
    s.wanted = Some(Instant::now());
    if s.store.is_none() {
        s.store = Some(store.clone());
    }
    if !s.running {
        s.running = true;
        let _ = std::thread::Builder::new().name("claude-remote".into()).spawn(poll);
    }
}

/// Tiles are connected to remote servers that have not been looked at yet.
pub(crate) fn pending() -> bool {
    let s = state().lock_safe();
    let Some(store) = s.store.clone() else { return false };
    let looked = s.tried.clone();
    drop(s);
    tiles(&store).iter().any(|t| !looked.contains(&t.2))
}

/// What was last seen on the remote servers.
pub(crate) fn snapshot() -> Snapshot {
    let s = state().lock_safe();
    let mut out = Snapshot { servers: s.names.clone(), agents: s.agents.clone(), ..Default::default() };
    for (server, st) in &s.servers {
        out.peers.extend(st.peers.values().cloned());
        out.messages.extend(st.got.iter().map(|(sid, from, at, said)| (server.clone(), sid.clone(), from.clone(), *at, said.clone())));
    }
    out.peers.sort_by(|a, b| (&a.server_id, &a.name).cmp(&(&b.server_id, &b.name)));
    out
}

/// A tile to look through: (session id, tmux name, server id, user@host).
type Tile = (String, String, String, String);

fn tiles(store: &PtyStore) -> Vec<Tile> {
    let map = store.lock_safe();
    map.iter()
        .filter_map(|(sid, s)| {
            let b = s.remote_binding.as_ref()?;
            s.ssh_cmd.as_ref()?;
            if !s.label.starts_with("ssh:") || !mux_path(sid).exists() {
                return None;
            }
            Some((sid.clone(), s.tmux_name.clone(), b.server_id().to_string(), b.target()))
        })
        .collect()
}

fn poll() {
    loop {
        let (store, wanted) = {
            let s = state().lock_safe();
            (s.store.clone(), s.wanted)
        };
        let active = wanted.is_some_and(|w| w.elapsed() < IDLE_AFTER);
        if let (true, Some(store)) = (active, store) {
            look_once(&store);
        }
        std::thread::sleep(EVERY);
    }
}

fn look_once(store: &PtyStore) {
    let tiles = tiles(store);
    let live: HashSet<&str> = tiles.iter().map(|t| t.2.as_str()).collect();
    {
        let mut s = state().lock_safe();
        // Servers no tile is connected to any more are forgotten.
        s.servers.retain(|id, _| live.contains(id.as_str()));
        for id in &live {
            if !s.names.contains_key(*id) {
                let name = crate::remote_servers::find_server(id).ok().flatten().map(|srv| srv.name).unwrap_or_else(|| id.to_string());
                s.names.insert(id.to_string(), name);
            }
        }
    }
    // Each server's sessions as its tiles see them; the tile a session is under claims it.
    let mut fresh: HashMap<String, HashMap<String, RemoteClaude>> = HashMap::new();
    let mut ok_servers: HashSet<String> = HashSet::new();
    for (sid, tmux_name, server_id, target) in &tiles {
        let offsets = {
            let s = state().lock_safe();
            s.servers.get(server_id).map(|st| st.offsets.clone()).unwrap_or_default()
        };
        let Some(out) = probe(sid, target) else { continue };
        ok_servers.insert(server_id.clone());
        let found = parse(&out, server_id, tmux_name);
        let mut s = state().lock_safe();
        s.agents.insert(tmux_name.clone(), found.agents.clone());
        let st = s.servers.entry(server_id.clone()).or_default();
        for (session, size) in found.sizes {
            st.offsets.insert(session, size);
        }
        for (session, from, at, id, said) in found.messages {
            // The same message shows up twice (as delivered and as queued).
            if id.is_empty() || st.ids.insert(id) {
                st.got.push((session, from, at, said));
            }
        }
        if st.got.len() > MAX_KEPT {
            let cut = st.got.len() - MAX_KEPT;
            st.got.drain(..cut);
        }
        let _ = offsets;
        merge(fresh.entry(server_id.clone()).or_default(), found.peers);
    }
    let mut s = state().lock_safe();
    s.tried = tiles.iter().map(|t| t.2.clone()).collect();
    for (server, peers) in fresh {
        if ok_servers.contains(&server) {
            s.servers.entry(server).or_default().peers = peers;
        }
    }
}

/// Adds what one tile's probe found to its server's sessions: each running
/// Claude once, claimed by the tile it runs under.
fn merge(seen: &mut HashMap<String, RemoteClaude>, peers: Vec<RemoteClaude>) {
    for peer in peers {
        match seen.get_mut(&peer.key()) {
            Some(have) if have.tile.is_none() && peer.tile.is_some() => *have = peer,
            Some(_) => {}
            None => {
                seen.insert(peer.key(), peer);
            }
        }
    }
}

/// Runs the probe over the tile's shared connection; its output, or `None`.
fn probe(session_id: &str, target: &str) -> Option<String> {
    let os = state().lock_safe().os.get(session_id).copied();
    let offsets = {
        let s = state().lock_safe();
        s.servers.values().flat_map(|st| st.offsets.clone()).collect::<HashMap<_, _>>()
    };
    let order: Vec<Os> = match os {
        Some(o) => vec![o],
        None => vec![Os::Unix, Os::Windows],
    };
    for o in order {
        if let Some(out) = run(session_id, target, o, &offsets) {
            if out.lines().any(|l| l.starts_with("C\t")) {
                state().lock_safe().os.insert(session_id.to_string(), o);
                return Some(out);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::probe::b64_encode;
    use super::*;

    #[test]
    fn one_conversation_open_in_several_claudes_shows_each() {
        // spark: `claude -c` in two tiles and a desktop terminal, one conversation.
        let rec = |pid: u32, name: &str| b64_encode(format!(r#"{{"pid":{pid},"sessionId":"c9ab-04","cwd":"/home","name":"{name}","status":"idle","messagingSocketPath":"/run/{pid}.sock"}}"#).as_bytes());
        let tile_a = format!("R\t1\t{}\nR\t0\t{}\nR\t0\t{}\n", rec(1807, "home-f7"), rec(412, "home-6a"), rec(54, "home-ba"));
        let tile_b = format!("R\t0\t{}\nR\t1\t{}\nR\t0\t{}\n", rec(1807, "home-f7"), rec(412, "home-6a"), rec(54, "home-ba"));
        let mut seen = HashMap::new();
        merge(&mut seen, parse(&tile_a, "spark", "nb_a").peers);
        merge(&mut seen, parse(&tile_b, "spark", "nb_b").peers);
        let mut got: Vec<(String, Option<String>)> = seen.values().map(|p| (p.name.clone(), p.tile.clone())).collect();
        got.sort();
        assert_eq!(got, vec![
            ("home-6a".into(), Some("nb_b".into())),
            ("home-ba".into(), None),
            ("home-f7".into(), Some("nb_a".into())),
        ]);
    }

    #[test]
    fn the_socket_folder_is_made_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("nabiman-mux-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(private_dir(&dir).is_some());
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
