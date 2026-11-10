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
use crate::lock_safe::LockExt;

#[path = "claude_peers_link.rs"]
mod link;
#[path = "claude_peers_models.rs"]
mod models;
#[path = "claude_peers_procs.rs"]
mod procs;
#[path = "claude_peers_records.rs"]
mod records;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};

use link::link;
use models::last_model;
pub(crate) use models::model_ok;
use procs::{local_agents, panes};
pub(crate) use procs::{stat_fields, tmux_session_of_pid};
use records::{alive, read_record, session_files, session_of, Record};

/// How long one look is reused: the list and every open window ask often.
const FRESH: Duration = Duration::from_secs(2);
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
    /// When it last became busy or idle (seconds since 1970), if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
    /// The model that last answered in it (`claude-opus-5-5`…), from its transcript.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Its Claude Code version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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
    /// The AI agents running in it, by kind (`claude`, `codex`, `gemini`…), told by their processes.
    pub agents: Vec<String>,
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

/// The id a remote Claude session with no tile goes by among the tiles: its
/// server, conversation and process (one conversation may run in several).
pub(crate) fn ghost_id(server_id: &str, p: &super::claude_remote::RemoteClaude) -> String {
    format!("rc:{server_id}:{}", p.key())
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

/// Drop what was seen, so the next look scans again (the lead changed, say).
pub(crate) fn forget() {
    *CACHE.lock_safe() = None;
}

/// The Claude sessions in each tmux session, by tmux name.
pub(crate) fn look() -> Arc<HashMap<String, Seen>> {
    let mut cache = CACHE.lock_safe();
    if let Some((at, seen)) = cache.as_ref() {
        if at.elapsed() < FRESH {
            return seen.clone();
        }
    }
    let seen = Arc::new(scan());
    *cache = Some((Instant::now(), seen.clone()));
    seen
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
    // The agents in each tile, by their processes: here, and on the servers the tiles are connected to.
    for (tmux, kind) in local_agents(&panes) {
        seen.entry(tmux).or_default().agents.push(kind.to_string());
    }
    for (tmux, kinds) in &remote.agents {
        seen.entry(tmux.clone()).or_default().agents.extend(kinds.iter().cloned());
    }
    link(&found, &remote, &mut seen);
    add_remote(&remote, &mut seen);
    for ((rec, tmux_name), reference) in found.into_iter().zip(refs) {
        let name = if rec.name.is_empty() { base_name(&rec.cwd) } else { rec.name.clone() };
        let model = rec.file.parent().and_then(|p| p.parent()).and_then(|dir| super::claude_links::transcript_of(dir, &rec.cwd, &rec.session_id)).and_then(|t| last_model(&t));
        let peer = ClaudePeer {
            address: format!("{name} [{reference}]"),
            name,
            reference,
            cwd: rec.cwd,
            busy: rec.busy,
            since: rec.since,
            model,
            version: Some(rec.version.clone()).filter(|v| !v.is_empty()),
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
    for entry in seen.values_mut() {
        if !entry.claude.is_empty() {
            entry.agents.push("claude".to_string());
        }
        entry.agents = super::ai_agents::tidy(std::mem::take(&mut entry.agents));
    }
    seen
}

/// Remote Claude sessions: into their tile's entry, or one of their own.
fn add_remote(remote: &super::claude_remote::Snapshot, seen: &mut HashMap<String, Seen>) {
    for p in &remote.peers {
        let server = remote.servers.get(&p.server_id).cloned().unwrap_or_else(|| p.server_id.clone());
        let card = p.tile.clone().unwrap_or_else(|| ghost_id(&p.server_id, p));
        let name = if p.name.is_empty() { base_name(&p.cwd) } else { p.name.clone() };
        let peer = ClaudePeer {
            address: p.bridge.as_ref().map(|b| format!("bridge:{b}")).unwrap_or_else(|| name.clone()),
            name,
            reference: String::new(),
            cwd: p.cwd.clone(),
            busy: p.busy,
            since: p.since,
            model: p.model.clone(),
            version: Some(p.version.clone()).filter(|v| !v.is_empty()),
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
    use std::path::Path;

    use super::records::parse_record;
    use super::*;

    #[test]
    fn a_background_record_keeps_the_id_attach_takes() {
        let rec = parse_record(r#"{"pid":7,"procStart":"1","messagingSocketPath":"/s","sessionId":"cd2cae1c-62","kind":"bg","jobId":"cd2cae1c","cwd":"/w"}"#, 1000, Path::new("/x")).unwrap();
        assert!(rec.background);
        assert_eq!(rec.job, "cd2cae1c");
        assert!(safe_job("cd2cae1c") && !safe_job("a b") && !safe_job("x';rm") && !safe_job(""));
        assert_eq!(background("lc:cd2cae1c-62"), None, "a record whose process is not running opens nothing");
        assert_eq!(background("rc:x:y"), None);
    }

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
    fn a_nameless_session_goes_by_its_folder() {
        assert_eq!(base_name("/workspace/artisland/"), "artisland");
    }
}
