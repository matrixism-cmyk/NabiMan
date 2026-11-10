//! Who hands work to whom: one Claude Code session messaging others is
//! orchestrating them, and the session list draws it.
//!
//! A message from another session lands in the receiver's transcript
//! (`~/.claude/projects/<folder>/<session id>.jsonl`) as a user entry whose
//! `origin` is `{"kind": "peer", "from": "uds:<socket>", "verifiedPeerPid": …}`:
//! Claude Code checked the sender itself. Only that is read — never what the
//! message said. Transcripts grow to hundreds of megabytes: each is read
//! through once (a quick byte search, parsing only the lines it finds) and
//! afterwards only what was added since.
//!
//! A message comes in two forms: a user entry when the receiver was waiting,
//! or a `queued_command` attachment when it arrived mid-turn.
//!
//! Two sessions are linked while a message passed between them within the
//! hour. The one whose work reached the other first leads: directly, or
//! through others it handed work to — art asks or-kr, or-kr passes it to
//! mpcf, mpcf answers art: art still leads mpcf, though mpcf wrote to art
//! before art wrote to it. Answers never turn a link round. That needs the
//! whole history; read only near its end, a transcript can begin with an
//! answer, which reversed links and made loops. A session that leads others
//! and is led by none is the orchestrator.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Messages older than this no longer tie two sessions together.
pub(crate) const WINDOW_SECS: u64 = 60 * 60;
/// A pair silent this long starts afresh: whoever writes first then leads.
pub(crate) const EPISODE_GAP_SECS: u64 = 6 * 60 * 60;
/// The most of a transcript ever read: its last gigabyte.
const MAX_READ: u64 = 1024 * 1024 * 1024;
/// Read in pieces this big.
const PIECE: usize = 8 * 1024 * 1024;
/// Messages kept per transcript (the first ones and the latest).
const MAX_KEPT: usize = 20_000;

/// One message received: the sender (`uds:<socket>` or `bridge:<session>`),
/// when (unix seconds), and the first line of what it said (see `gist`).
pub(crate) type Received = (String, u64, String);

/// How much of a message is kept, for the tooltip on its arrow.
pub(crate) const GIST_LEN: usize = 140;

/// The first line a peer message said, without the envelope Claude Code puts
/// around it.
pub(crate) fn gist(text: &str) -> String {
    let body = match text.find("<cross-session-message") {
        Some(i) => match text[i..].find('>') {
            Some(j) => &text[i + j + 1..],
            None => return String::new(),
        },
        None => text,
    };
    let body = body.split("</cross-session-message>").next().unwrap_or("");
    body.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.chars().filter(|c| !c.is_control()).take(GIST_LEN).collect())
        .unwrap_or_default()
}

#[derive(Default, Clone)]
struct Tail {
    /// Where the next look starts: just after the last whole line read.
    offset: u64,
    got: Vec<Received>,
    /// Message ids seen, so one recorded twice counts once.
    ids: HashSet<String>,
}

const NEEDLE: &[u8] = br#""kind":"peer""#;

static TAILS: Mutex<Option<HashMap<PathBuf, Tail>>> = Mutex::new(None);
/// A reader is going through the transcripts.
static READING: AtomicBool = AtomicBool::new(false);

/// The transcript of a session, under the `.claude` folder that holds its record.
pub(crate) fn transcript_of(claude_dir: &Path, cwd: &str, session_id: &str) -> Option<PathBuf> {
    if session_id.is_empty() || !session_id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return None;
    }
    let projects = claude_dir.join("projects");
    let file = format!("{session_id}.jsonl");
    let guess = projects.join(folder_name(cwd)).join(&file);
    if guess.is_file() {
        return Some(guess);
    }
    // A long folder name is shortened: look in each.
    fs::read_dir(&projects).ok()?.flatten().map(|d| d.path().join(&file)).find(|p| p.is_file())
}

/// How Claude Code names a project's folder: the working folder with every
/// character but letters and digits turned into `-`.
pub(crate) fn folder_name(cwd: &str) -> String {
    cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// The messages from other sessions in these transcripts, by transcript, as
/// far as they have been read. Reading goes on in the background — the first
/// time through takes seconds — and each later look reads only what is new.
pub(crate) fn received(transcripts: &[PathBuf]) -> HashMap<PathBuf, Vec<Received>> {
    let out = {
        let mut guard = TAILS.lock().unwrap_or_else(|e| e.into_inner());
        let tails = guard.get_or_insert_with(HashMap::new);
        // Transcripts no session writes any more are forgotten.
        let wanted: HashSet<&PathBuf> = transcripts.iter().collect();
        tails.retain(|p, _| wanted.contains(p));
        transcripts.iter().filter_map(|p| Some((p.clone(), tails.get(p)?.got.clone()))).collect()
    };
    if !READING.swap(true, Ordering::AcqRel) {
        let list = transcripts.to_vec();
        let spawned = std::thread::Builder::new().name("claude-links".into()).spawn(move || {
            for path in list {
                read_more(&path);
            }
            READING.store(false, Ordering::Release);
        });
        if spawned.is_err() {
            READING.store(false, Ordering::Release);
        }
    }
    out
}

/// Reads what was added to one transcript, without holding the lock meanwhile.
fn read_more(path: &Path) {
    let mut tail = {
        let guard = TAILS.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().and_then(|t| t.get(path).cloned()).unwrap_or_default()
    };
    let _ = catch_up(path, &mut tail);
    trim(&mut tail);
    let mut guard = TAILS.lock().unwrap_or_else(|e| e.into_inner());
    guard.get_or_insert_with(HashMap::new).insert(path.to_path_buf(), tail);
}

/// Keeps a transcript's messages within `MAX_KEPT`: the first from each
/// sender (who wrote first) and the latest.
fn trim(tail: &mut Tail) {
    if tail.got.len() <= MAX_KEPT {
        return;
    }
    let mut firsts = HashSet::new();
    let cut = tail.got.len() - MAX_KEPT / 2;
    let got = std::mem::take(&mut tail.got);
    tail.got = got
        .into_iter()
        .enumerate()
        .filter(|(i, (s, _, _))| *i >= cut || firsts.insert(s.clone()))
        .map(|(_, g)| g)
        .collect();
}

fn catch_up(path: &Path, tail: &mut Tail) -> std::io::Result<()> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len < tail.offset {
        // Written over: start again.
        *tail = Tail::default();
    }
    let mut start = tail.offset;
    let mut skip_partial = false;
    if start == 0 && len > MAX_READ {
        start = len - MAX_READ;
        skip_partial = true;
    }
    file.seek(SeekFrom::Start(start))?;
    // Whole lines only; what follows the last newline waits for the next piece.
    let mut carry: Vec<u8> = Vec::new();
    let mut piece = vec![0u8; PIECE];
    let mut pos = start;
    let mut left = len - start;
    while left > 0 {
        let n = file.read(&mut piece[..PIECE.min(left as usize)])?;
        if n == 0 {
            break;
        }
        left -= n as u64;
        pos += n as u64;
        carry.extend_from_slice(&piece[..n]);
        let Some(last) = memchr::memrchr(b'\n', &carry) else { continue };
        let whole = &carry[..last];
        // Begun mid-line: that first piece of a line is not one.
        let from = if skip_partial { memchr::memchr(b'\n', whole).map_or(whole.len(), |i| i + 1) } else { 0 };
        skip_partial = false;
        let mut done_to = from;
        for hit in memchr::memmem::find_iter(&whole[from..], NEEDLE).map(|h| h + from) {
            if hit < done_to {
                continue; // another hit on a line already read
            }
            let begin = memchr::memrchr(b'\n', &whole[..hit]).map_or(0, |i| i + 1).max(from);
            let end = memchr::memchr(b'\n', &whole[hit..]).map_or(whole.len(), |i| hit + i);
            done_to = end;
            if let Some((sock, at, id, said)) = peer_message(&whole[begin..end]) {
                if id.is_empty() || tail.ids.insert(id) {
                    tail.got.push((sock, at, said));
                }
            }
        }
        carry.drain(..=last);
        tail.offset = pos - carry.len() as u64;
    }
    Ok(())
}

/// A message another session sent, as its transcript line records it: the
/// sender's socket, when, and the message's id.
fn peer_message(line: &[u8]) -> Option<(String, u64, String, String)> {
    let v: serde_json::Value = serde_json::from_slice(line).ok()?;
    let origin = match v.get("type")?.as_str()? {
        "user" => v.get("origin")?,
        // Arrived while the receiver was working.
        "attachment" => {
            let a = v.get("attachment")?;
            if a.get("type")?.as_str()? != "queued_command" {
                return None;
            }
            a.get("origin")?
        }
        _ => return None,
    };
    if origin.get("kind")?.as_str()? != "peer" {
        return None;
    }
    // A sender on this machine was checked by the kernel; one on another
    // machine came through Remote Control, under the same account.
    let from = origin.get("from")?.as_str()?;
    let checked = from.starts_with("uds:") && origin.get("verifiedPeerPid").is_some();
    if !checked && !from.starts_with("bridge:") {
        return None;
    }
    let sock = from.to_string();
    let at = chrono::DateTime::parse_from_rfc3339(v.get("timestamp")?.as_str()?).ok()?;
    let id = origin.get("msg_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    // What it said: the user turn's text, or a queued command's prompt.
    let text = v
        .pointer("/message/content")
        .map(|c| match c {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(parts) => parts.iter().filter_map(|p| p.get("text")?.as_str()).collect::<Vec<_>>().join("\n"),
            _ => String::new(),
        })
        .or_else(|| v.pointer("/attachment/prompt").and_then(|p| p.as_str()).map(str::to_string))
        .unwrap_or_default();
    Some((sock, u64::try_from(at.timestamp()).ok()?, id, gist(&text)))
}

/// Which sessions lead which, from messages `(from, to, when)` between them.
#[derive(Default, Debug, PartialEq)]
pub(crate) struct Links {
    pub leads: HashMap<String, Vec<String>>,
    pub orchestrators: HashSet<String>,
}

/// `lead`: the session the operator made the lead; a pair it is in runs from it.
pub(crate) fn links(messages: &[(String, String, u64)], now: u64, lead: Option<&str>) -> Links {
    let mut sorted: Vec<(&str, &str, u64)> = messages
        .iter()
        .filter(|(f, t, _)| f != t)
        .map(|(f, t, at)| (f.as_str(), t.as_str(), *at))
        .collect();
    sorted.sort_by_key(|m| m.2);
    // The pairs that spoke within the hour.
    let mut pairs: HashSet<(&str, &str)> = HashSet::new();
    for (f, t, at) in &sorted {
        if at + WINDOW_SECS >= now {
            pairs.insert(if f < t { (*f, *t) } else { (*t, *f) });
        }
    }
    // Who started each pair's current exchange: the sender of its first
    // message after a quiet spell of EPISODE_GAP_SECS. Answers and reports
    // that follow do not turn it round, and an old exchange (yesterday's) does
    // not decide today's.
    let a_started = |a: &str, b: &str| -> bool {
        let between: Vec<&(&str, &str, u64)> = sorted.iter().filter(|(f, t, _)| (*f == a && *t == b) || (*f == b && *t == a)).collect();
        let mut start = between.len().saturating_sub(1);
        while start > 0 && between[start].2.saturating_sub(between[start - 1].2) <= EPISODE_GAP_SECS {
            start -= 1;
        }
        between.get(start).is_none_or(|m| m.0 == a)
    };
    let mut out = Links::default();
    let mut led: HashSet<&str> = HashSet::new();
    for &(a, b) in &pairs {
        let (from, to) = if lead == Some(a) {
            (a, b)
        } else if lead == Some(b) {
            (b, a)
        } else if a_started(a, b) {
            (a, b)
        } else {
            (b, a)
        };
        out.leads.entry(from.to_string()).or_default().push(to.to_string());
        led.insert(to);
    }
    for (from, to) in out.leads.iter_mut() {
        to.sort();
        if !led.contains(from.as_str()) {
            out.orchestrators.insert(from.clone());
        }
    }
    out
}

#[cfg(test)]
#[path = "claude_links_tests.rs"]
mod tests;
