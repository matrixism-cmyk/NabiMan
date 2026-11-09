//! Claude Code sessions running inside NabiMan's sessions, and whether each
//! session is busy.
//!
//! A Claude Code session that other sessions can message is known to them by
//! an address such as `artisland-bd [946528]`: its name, and a ref worked out
//! from its messaging socket. The terminal header shows that address, so a
//! person can tell one Claude session from another and hand it to another
//! Claude as the `to` of a message.
//!
//! Where it comes from: every Claude Code process writes
//! `~/.claude/sessions/<pid>.json` (its name, working folder, socket, start
//! time and whether it is busy). Its parent processes lead up to the shell
//! tmux started in a pane, and tmux names the session that pane is in. (The
//! process's environment would name the pane too, but root here may not read
//! another account's environment: the container has no `CAP_SYS_PTRACE`.)
//! A file only counts when its process is still the one that wrote it (the
//! start time matches), belongs to the file's owner, and runs under a pane of
//! this server's tmux; so a file left behind, or one written to claim
//! someone else's process, shows nothing.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};

/// How long one look is reused: the list and every open window ask often.
const FRESH: Duration = Duration::from_secs(2);
const MAX_FILES: usize = 400;
const MAX_FILE: u64 = 16 * 1024;
/// How far up the parent processes a pane's shell is looked for.
const MAX_HOPS: usize = 32;
/// The shortest ref Claude Code prints; longer only to tell two apart.
const MIN_REF: usize = 6;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct ClaudePeer {
    pub name: String,
    #[serde(rename = "ref")]
    pub reference: String,
    /// What another session's `to` takes: `name [ref]`.
    pub address: String,
    pub cwd: String,
    /// Working on a turn now, not waiting for the person.
    pub busy: bool,
    /// The saved server it runs on, when that is not this machine. Its
    /// `address` is then `bridge:<Remote Control session>` (what SendMessage
    /// takes for a session on another machine), or just its name without
    /// Remote Control.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
}

/// What is known about one tmux session.
#[derive(Default, Clone, Debug)]
pub(crate) struct Seen {
    pub claude: Vec<ClaudePeer>,
    /// The tmux sessions its Claude has handed work to lately (see `claude_links`).
    pub leads: Vec<String>,
    /// It leads others and none leads it.
    pub orchestrator: bool,
    /// For a remote Claude session no tile is connected to: its server's name.
    pub ghost_server: Option<String>,
    /// A Claude session on this machine outside every tile (a Claude Code
    /// background session, or one started in another terminal).
    pub ghost_local: bool,
    /// That session is a Claude Code background session (`claude --bg`).
    pub background: bool,
    /// For each card it leads: when it last sent work there and the first line of it.
    pub notes: HashMap<String, (u64, String)>,
}

/// The id a remote Claude session with no tile goes by among the tiles.
pub(crate) fn ghost_id(server_id: &str, session_id: &str) -> String {
    format!("rc:{server_id}:{session_id}")
}

/// A running Claude Code background session, as a tile opens it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Background {
    /// Its owner: the tile runs `claude attach` as them.
    pub uid: u32,
    /// What `claude attach` takes: letters, digits, `-` and `_` only.
    pub job: String,
    pub cwd: String,
    pub name: String,
}

fn safe_job(job: &str) -> bool {
    !job.is_empty() && job.len() <= 64 && job.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The live background session whose card is `lc:<session id>`; `None` for
/// anything else (a tile's session, one that ended, a made-up id).
pub(crate) fn background(card: &str) -> Option<Background> {
    let session = card.strip_prefix("lc:")?;
    session_files().into_iter().filter_map(|p| read_record(&p)).find_map(|rec| {
        let job = if rec.job.is_empty() { rec.session_id.chars().take(8).collect() } else { rec.job.clone() };
        (rec.background && !session.is_empty() && rec.session_id == session && safe_job(&job) && alive(&rec)).then(|| Background {
            uid: rec.uid,
            job,
            name: if rec.name.is_empty() { base_name(&rec.cwd) } else { rec.name.clone() },
            cwd: rec.cwd,
        })
    })
}

/// The id a Claude session on this machine outside every tile goes by.
pub(crate) fn local_ghost_id(rec_session: &str, sock: &str) -> String {
    if rec_session.is_empty() { format!("lc:{}", base_name(sock)) } else { format!("lc:{rec_session}") }
}

impl Seen {
    pub fn busy(&self) -> bool {
        self.claude.iter().any(|p| p.busy)
    }
}

type Cache = Mutex<Option<(Instant, Arc<HashMap<String, Seen>>)>>;
static CACHE: Cache = Mutex::new(None);

/// The Claude sessions in each tmux session, by tmux name.
pub(crate) fn look() -> Arc<HashMap<String, Seen>> {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, seen)) = cache.as_ref() {
        if at.elapsed() < FRESH {
            return seen.clone();
        }
    }
    let seen = Arc::new(scan());
    *cache = Some((Instant::now(), seen.clone()));
    seen
}

struct Panes {
    server_pid: u32,
    /// The shell tmux started in each pane, to the tmux session it is in.
    session_of: HashMap<u32, String>,
}

fn panes() -> Option<Panes> {
    // Plain names only: a time format (`#{session_activity}`) once crashed
    // the tmux server it was asked of, taking every session with it.
    let out = super::session::tmux()
        .args(["list-panes", "-a", "-F", "#{pid}\t#{pane_pid}\t#{session_name}"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_panes(&String::from_utf8_lossy(&out.stdout))
}

fn parse_panes(text: &str) -> Option<Panes> {
    let mut p = Panes { server_pid: 0, session_of: HashMap::new() };
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 3 {
            continue;
        }
        p.server_pid = f[0].parse().ok()?;
        if let Ok(pane_pid) = f[1].parse() {
            p.session_of.insert(pane_pid, f[2].to_string());
        }
    }
    (p.server_pid != 0).then_some(p)
}

fn scan() -> HashMap<String, Seen> {
    let Some(panes) = panes() else { return HashMap::new() };
    let mut seen: HashMap<String, Seen> = HashMap::new();
    // Every live Claude session here: in a tile (its tmux name), or not (an
    // `lc:` card of its own, so work handed to or from it still shows).
    let mut loose: Vec<String> = Vec::new();
    let found: Vec<(Record, String)> = session_files()
        .into_iter()
        .filter_map(|path| {
            let rec = read_record(&path)?;
            if !alive(&rec) {
                return None;
            }
            match session_of(&rec, &panes) {
                Some(session) => Some((rec, session)),
                None => {
                    let card = local_ghost_id(&rec.session_id, &rec.sock);
                    loose.push(card.clone());
                    Some((rec, card))
                }
            }
        })
        .collect();
    let refs = refs(&found.iter().map(|(r, _)| r.sock.as_str()).collect::<Vec<_>>());
    let remote = super::claude_remote::snapshot();
    link(&found, &remote, &mut seen);
    add_remote(&remote, &mut seen);
    for ((rec, tmux_name), reference) in found.into_iter().zip(refs) {
        let name = if rec.name.is_empty() { base_name(&rec.cwd) } else { rec.name.clone() };
        let peer = ClaudePeer {
            address: format!("{name} [{reference}]"),
            name,
            reference,
            cwd: rec.cwd,
            busy: rec.busy,
            server: None,
        };
        let background = rec.background;
        let entry = seen.entry(tmux_name.clone()).or_default();
        if loose.contains(&tmux_name) {
            entry.ghost_local = true;
            entry.background = background;
        }
        entry.claude.push(peer);
        entry.claude.sort_by(|a, b| a.address.cmp(&b.address));
    }
    seen
}

/// Remote Claude sessions: into their tile's entry, or one of their own.
fn add_remote(remote: &super::claude_remote::Snapshot, seen: &mut HashMap<String, Seen>) {
    for p in &remote.peers {
        let server = remote.servers.get(&p.server_id).cloned().unwrap_or_else(|| p.server_id.clone());
        let card = p.tile.clone().unwrap_or_else(|| ghost_id(&p.server_id, &p.session_id));
        let name = if p.name.is_empty() { base_name(&p.cwd) } else { p.name.clone() };
        let peer = ClaudePeer {
            address: p.bridge.as_ref().map(|b| format!("bridge:{b}")).unwrap_or_else(|| name.clone()),
            name,
            reference: String::new(),
            cwd: p.cwd.clone(),
            busy: p.busy,
            server: Some(server.clone()),
        };
        let entry = seen.entry(card).or_default();
        if p.tile.is_none() {
            entry.ghost_server = Some(server);
        }
        entry.claude.push(peer);
        entry.claude.sort_by(|a, b| a.address.cmp(&b.address));
    }
}

/// `%XX` escapes in a peer address (Claude Code escapes unusual characters).
fn unescape(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(b) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// Who has been handing work to whom among the sessions found, here and on
/// the servers the tiles are connected to. A sender is named by its socket on
/// the receiver's machine (`uds:…`) or, across machines, by its Remote Control
/// session (`bridge:…`).
fn link(found: &[(Record, String)], remote: &super::claude_remote::Snapshot, seen: &mut HashMap<String, Seen>) {
    // Each session's card (tmux name, or a remote one's own id), by address.
    let mut card: HashMap<String, String> = HashMap::new();
    for (r, t) in found {
        card.insert(format!("local|uds:{}", r.sock), t.clone());
        if let Some(b) = &r.bridge {
            card.insert(format!("bridge:{b}"), t.clone());
        }
    }
    let mut remote_card: HashMap<(&str, &str), String> = HashMap::new();
    for p in &remote.peers {
        let c = p.tile.clone().unwrap_or_else(|| ghost_id(&p.server_id, &p.session_id));
        card.insert(format!("{}|uds:{}", p.server_id, p.sock), c.clone());
        if let Some(b) = &p.bridge {
            card.insert(format!("bridge:{b}"), c.clone());
        }
        remote_card.insert((p.server_id.as_str(), p.session_id.as_str()), c);
    }
    let sender = |machine: &str, from: &str| -> Option<String> {
        let from = unescape(from);
        if from.starts_with("bridge:") {
            card.get(&from).cloned()
        } else {
            card.get(&format!("{machine}|{from}")).cloned()
        }
    };
    let transcripts: Vec<(PathBuf, &str)> = found
        .iter()
        .filter_map(|(r, t)| {
            let dir = r.file.parent()?.parent()?;
            Some((super::claude_links::transcript_of(dir, &r.cwd, &r.session_id)?, t.as_str()))
        })
        .collect();
    let paths: Vec<PathBuf> = transcripts.iter().map(|(p, _)| p.clone()).collect();
    let now = super::session::now_unix();
    let got = super::claude_links::received(&paths);
    let mut messages = Vec::new();
    // The latest message along each pair, for the tooltip on its arrow.
    let mut latest: HashMap<(String, String), (u64, String)> = HashMap::new();
    let mut note = |f: &str, t: &str, at: u64, said: &str| {
        let slot = latest.entry((f.to_string(), t.to_string())).or_insert((0, String::new()));
        if at >= slot.0 && !said.is_empty() {
            *slot = (at, said.to_string());
        }
    };
    for (path, to) in &transcripts {
        for (from, at, said) in got.get(path).into_iter().flatten() {
            if let Some(f) = sender("local", from) {
                note(&f, to, *at, said);
                messages.push((f, to.to_string(), *at));
            }
        }
    }
    for (server, receiver, from, at, said) in &remote.messages {
        let (Some(to), Some(f)) = (remote_card.get(&(server.as_str(), receiver.as_str())), sender(server, from)) else { continue };
        note(&f, to, *at, said);
        messages.push((f, to.clone(), *at));
    }
    let links = super::claude_links::links(&messages, now);
    for (from, to) in links.leads {
        let entry = seen.entry(from.clone()).or_default();
        entry.orchestrator = links.orchestrators.contains(&from);
        entry.notes = to.iter().filter_map(|t| Some((t.clone(), latest.get(&(from.clone(), t.clone()))?.clone()))).collect();
        entry.leads = to;
    }
}

/// Every account's `~/.claude/sessions/*.json`, at most `MAX_FILES` of them.
fn session_files() -> Vec<PathBuf> {
    let mut homes: Vec<PathBuf> = vec![PathBuf::from("/root")];
    if let Ok(passwd) = fs::read_to_string("/etc/passwd") {
        for line in passwd.lines() {
            let f: Vec<&str> = line.split(':').collect();
            if f.len() >= 6 && f[2].parse::<u32>().is_ok_and(|uid| (1000..65534).contains(&uid)) {
                homes.push(PathBuf::from(f[5]));
            }
        }
    }
    homes.sort();
    homes.dedup();
    let mut files = Vec::new();
    for home in homes {
        let Ok(dir) = fs::read_dir(home.join(".claude/sessions")) else { continue };
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                files.push(path);
                if files.len() >= MAX_FILES {
                    return files;
                }
            }
        }
    }
    files
}

#[derive(Debug, PartialEq)]
struct Record {
    pid: u32,
    proc_start: String,
    sock: String,
    name: String,
    cwd: String,
    busy: bool,
    session_id: String,
    /// Its Remote Control session, when that is on.
    bridge: Option<String>,
    /// A Claude Code background session (`kind: bg`), run by its daemon rather than a terminal.
    background: bool,
    /// The id `claude attach` takes for a background session (its `jobId`).
    job: String,
    /// Who wrote the file.
    uid: u32,
    /// The record's own path, under the account's `.claude` folder.
    file: PathBuf,
}

fn read_record(path: &Path) -> Option<Record> {
    let file = fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE {
        return None;
    }
    let mut text = String::new();
    file.take(MAX_FILE).read_to_string(&mut text).ok()?;
    parse_record(&text, meta.uid(), path)
}

fn parse_record(text: &str, uid: u32, file: &Path) -> Option<Record> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let rec = Record {
        pid: u32::try_from(v.get("pid")?.as_u64()?).ok()?,
        proc_start: s("procStart"),
        sock: s("messagingSocketPath"),
        name: clip(&s("name"), 80),
        cwd: clip(&s("cwd"), 300),
        busy: s("status") == "busy",
        session_id: s("sessionId"),
        bridge: Some(s("bridgeSessionId")).filter(|b| !b.is_empty()),
        background: s("kind") == "bg",
        job: s("jobId"),
        uid,
        file: file.to_path_buf(),
    };
    (!rec.sock.is_empty() && !rec.proc_start.is_empty()).then_some(rec)
}

fn clip(s: &str, n: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(n).collect()
}

/// The record's process is running: same owner, same start time (not a reused pid).
fn alive(rec: &Record) -> bool {
    let proc_dir = PathBuf::from(format!("/proc/{}", rec.pid));
    let owner = fs::metadata(&proc_dir).map(|m| m.uid()).ok();
    let start = fs::read_to_string(proc_dir.join("stat")).ok().and_then(|s| stat_fields(&s)).map(|(start, _)| start);
    owner == Some(rec.uid) && start.as_deref() == Some(rec.proc_start.as_str())
}

/// The tmux session the record's process runs in, if it is still that
/// process, the file's owner runs it, and a pane of this tmux server's is
/// among its parents.
fn session_of(rec: &Record, panes: &Panes) -> Option<String> {
    if !alive(rec) {
        return None;
    }
    up_to_pane(rec.pid, panes, |pid| {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        stat_fields(&stat).map(|(_, ppid)| ppid)
    })
}

/// The tmux session a process runs in, found through its parents.
pub(crate) fn tmux_session_of_pid(pid: u32) -> Option<String> {
    let panes = panes()?;
    up_to_pane(pid, &panes, |p| {
        let stat = fs::read_to_string(format!("/proc/{p}/stat")).ok()?;
        stat_fields(&stat).map(|(_, ppid)| ppid)
    })
}

/// Climbs from `pid` through `parent` to a pane's shell, stopping at the
/// tmux server itself or after `MAX_HOPS`.
fn up_to_pane(mut pid: u32, panes: &Panes, parent: impl Fn(u32) -> Option<u32>) -> Option<String> {
    for _ in 0..MAX_HOPS {
        if let Some(session) = panes.session_of.get(&pid) {
            return Some(session.clone());
        }
        if pid <= 1 || pid == panes.server_pid {
            return None;
        }
        pid = parent(pid)?;
    }
    None
}

/// The start time (field 22) and parent (field 4) in `/proc/<pid>/stat`.
pub(crate) fn stat_fields(stat: &str) -> Option<(String, u32)> {
    // The command name (field 2) may hold spaces and parentheses.
    let rest: Vec<&str> = stat[stat.rfind(')')? + 1..].split_whitespace().collect();
    Some((rest.get(19)?.to_string(), rest.get(1)?.parse().ok()?))
}

/// Refs as Claude Code prints them: the start of sha256(`session:<socket>`),
/// six characters, or longer where two would otherwise share a start.
fn refs(socks: &[&str]) -> Vec<String> {
    let full: Vec<String> = socks
        .iter()
        .map(|s| hex::encode(Sha256::digest(format!("session:{s}").as_bytes())))
        .collect();
    let mut sorted = full.clone();
    sorted.sort();
    sorted.dedup();
    full.iter()
        .map(|h| {
            let i = sorted.binary_search(h).unwrap_or(0);
            let shared = |o: Option<&String>| o.map_or(0, |o| h.chars().zip(o.chars()).take_while(|(a, b)| a == b).count());
            let near = shared(i.checked_sub(1).and_then(|j| sorted.get(j))).max(shared(sorted.get(i + 1)));
            h[..h.len().min(MIN_REF.max(near + 1))].to_string()
        })
        .collect()
}

fn base_name(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_background_record_keeps_the_id_attach_takes() {
        let rec = parse_record(r#"{"pid":7,"procStart":"1","messagingSocketPath":"/s","sessionId":"cd2cae1c-62","kind":"bg","jobId":"cd2cae1c","cwd":"/w"}"#, 1000, Path::new("/x")).unwrap();
        assert!(rec.background);
        assert_eq!(rec.job, "cd2cae1c");
        assert!(safe_job("cd2cae1c") && !safe_job("a b") && !safe_job("x';rm") && !safe_job(""));
        assert_eq!(background("lc:cd2cae1c-62"), None, "a record whose process is not running opens nothing");
        assert_eq!(background("rc:x:y"), None);
    }

    use super::*;

    #[test]
    fn refs_match_what_claude_code_prints() {
        // Seen in ListAgents: nabiman-13 [df126b] and artisland-bd [946528].
        let r = refs(&["/tmp/cc-socks/1406230.sock", "/tmp/cc-socks/1673538.sock"]);
        assert_eq!(r, vec!["df126b", "946528"]);
    }

    #[test]
    fn refs_grow_only_to_tell_two_apart() {
        let r = refs(&["a", "b"]);
        assert!(r.iter().all(|x| x.len() == MIN_REF));
        let one = hex::encode(Sha256::digest(b"session:a"));
        assert_eq!(refs(&["a"])[0], one[..6]);
    }

    #[test]
    fn stat_survives_odd_command_names() {
        let stat = "1406230 (my (odd) cmd) S 1406007 1406230 1 34816 1 4194560 1 2 3 4 5 6 7 8 20 0 12 0 511134282 1 2";
        assert_eq!(stat_fields(stat), Some(("511134282".to_string(), 1406007)));
    }

    #[test]
    fn a_process_belongs_to_the_pane_its_parents_lead_to() {
        // claude → bash → su → the pane's shell, as `su nabi` in a pane runs it.
        let panes = parse_panes("427843\t1292830\tnb_a\n427843\t500\tnb_b\n").unwrap();
        let parents = HashMap::from([(1406230, 1406007), (1406007, 1406005), (1406005, 1292830), (1292830, 427843), (900, 1)]);
        let parent = |pid: u32| parents.get(&pid).copied();
        assert_eq!(up_to_pane(1406230, &panes, parent).as_deref(), Some("nb_a"));
        // Not under any pane: started elsewhere, or reparented to init.
        assert_eq!(up_to_pane(900, &panes, parent), None);
        // A loop in a made-up tree ends too.
        let lp = HashMap::from([(7, 8), (8, 7)]);
        assert_eq!(up_to_pane(7, &panes, |p| lp.get(&p).copied()), None);
    }

    #[test]
    fn a_record_needs_a_socket_and_start_time() {
        let text = r#"{"pid":7,"procStart":"99","messagingSocketPath":"/s","name":"x\u001b[31m","cwd":"/w","status":"busy"}"#;
        let rec = parse_record(text, 1000, Path::new("/h/.claude/sessions/7.json")).unwrap();
        assert_eq!(rec.name, "x[31m");
        assert!(rec.busy);
        assert_eq!(parse_record(r#"{"pid":7,"procStart":"99"}"#, 0, Path::new("/x")), None);
    }

    #[test]
    fn panes_map_to_their_sessions() {
        let p = parse_panes("427843\t1292830\tnb_a\n427843\t1672302\tnb_b\n").unwrap();
        assert_eq!(p.server_pid, 427843);
        assert_eq!(p.session_of[&1672302], "nb_b");
        assert_eq!(p.session_of[&1292830], "nb_a");
        assert!(parse_panes("").is_none());
    }

    #[test]
    fn a_nameless_session_goes_by_its_folder() {
        assert_eq!(base_name("/workspace/artisland/"), "artisland");
    }
}
