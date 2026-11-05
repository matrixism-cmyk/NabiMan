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
}

/// What is known about one tmux session.
#[derive(Default, Clone, Debug)]
pub(crate) struct Seen {
    pub claude: Vec<ClaudePeer>,
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
    let found: Vec<(Record, String)> = session_files()
        .into_iter()
        .filter_map(|path| {
            let rec = read_record(&path)?;
            let session = session_of(&rec, &panes)?;
            Some((rec, session))
        })
        .collect();
    let refs = refs(&found.iter().map(|(r, _)| r.sock.as_str()).collect::<Vec<_>>());
    for ((rec, tmux_name), reference) in found.into_iter().zip(refs) {
        let name = if rec.name.is_empty() { base_name(&rec.cwd) } else { rec.name.clone() };
        let peer = ClaudePeer {
            address: format!("{name} [{reference}]"),
            name,
            reference,
            cwd: rec.cwd,
            busy: rec.busy,
        };
        let entry = seen.entry(tmux_name).or_default();
        entry.claude.push(peer);
        entry.claude.sort_by(|a, b| a.address.cmp(&b.address));
    }
    seen
}

/// Every account's `~/.claude/sessions/*.json`, at most `MAX_FILES` of them.
fn session_files() -> Vec<PathBuf> {
    let mut homes: Vec<PathBuf> = vec![PathBuf::from("/root")];
    if let Ok(passwd) = fs::read_to_string("/etc/passwd") {
        for line in passwd.lines() {
            let f: Vec<&str> = line.split(':').collect();
            if f.len() >= 6 && f[2].parse::<u32>().is_ok_and(|uid| uid >= 1000 && uid < 65534) {
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
    /// Who wrote the file.
    uid: u32,
}

fn read_record(path: &Path) -> Option<Record> {
    let file = fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE {
        return None;
    }
    let mut text = String::new();
    file.take(MAX_FILE).read_to_string(&mut text).ok()?;
    parse_record(&text, meta.uid())
}

fn parse_record(text: &str, uid: u32) -> Option<Record> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let rec = Record {
        pid: u32::try_from(v.get("pid")?.as_u64()?).ok()?,
        proc_start: s("procStart"),
        sock: s("messagingSocketPath"),
        name: clip(&s("name"), 80),
        cwd: clip(&s("cwd"), 300),
        busy: s("status") == "busy",
        uid,
    };
    (!rec.sock.is_empty() && !rec.proc_start.is_empty()).then_some(rec)
}

fn clip(s: &str, n: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(n).collect()
}

/// The tmux session the record's process runs in, if it is still that
/// process, the file's owner runs it, and a pane of this tmux server's is
/// among its parents.
fn session_of(rec: &Record, panes: &Panes) -> Option<String> {
    let proc_dir = PathBuf::from(format!("/proc/{}", rec.pid));
    if fs::metadata(&proc_dir).ok()?.uid() != rec.uid {
        return None;
    }
    let (start, _) = stat_fields(&fs::read_to_string(proc_dir.join("stat")).ok()?)?;
    if start != rec.proc_start {
        return None;
    }
    up_to_pane(rec.pid, panes, |pid| {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
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
fn stat_fields(stat: &str) -> Option<(String, u32)> {
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
        let rec = parse_record(text, 1000).unwrap();
        assert_eq!(rec.name, "x[31m");
        assert!(rec.busy);
        assert_eq!(parse_record(r#"{"pid":7,"procStart":"99"}"#, 0), None);
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
