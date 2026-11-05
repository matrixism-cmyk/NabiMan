//! Who hands work to whom: one Claude Code session messaging others is
//! orchestrating them, and the session list draws it.
//!
//! A message from another session lands in the receiver's transcript
//! (`~/.claude/projects/<folder>/<session id>.jsonl`) as a user entry whose
//! `origin` is `{"kind": "peer", "from": "uds:<socket>", "verifiedPeerPid": …}`:
//! Claude Code checked the sender itself. Only that is read — never what the
//! message said. Transcripts grow to hundreds of megabytes, so each is read
//! once from near its end and afterwards only what was added since.
//!
//! Two sessions are linked while a message passed between them within the
//! hour. Of the two, the one linked to more sessions leads — the one handing
//! out work talks to several, each worker to it alone; between equals, the
//! one that wrote first (over the last day) leads, so answers do not turn it
//! round. A session that leads others and is led by none is the orchestrator.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Messages older than this no longer tie two sessions together.
pub(crate) const WINDOW_SECS: u64 = 60 * 60;
/// How long messages are kept, to tell who wrote first.
const HISTORY_SECS: u64 = 24 * 60 * 60;
/// Where a transcript seen for the first time is read from: this far before its end.
const FIRST_TAIL: u64 = 4 * 1024 * 1024;
/// The most read in one look; a transcript that grew more is caught up from near its end.
const MAX_STEP: u64 = 8 * 1024 * 1024;

/// One message received: the sender's socket and when (unix seconds).
pub(crate) type Received = (String, u64);

#[derive(Default)]
struct Tail {
    /// Where the next look starts: just after the last whole line read.
    offset: u64,
    got: Vec<Received>,
}

static TAILS: Mutex<Option<HashMap<PathBuf, Tail>>> = Mutex::new(None);

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
fn folder_name(cwd: &str) -> String {
    cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// The messages from other sessions in these transcripts over the last day,
/// by transcript; what was read before is kept, so each look reads only what
/// is new.
pub(crate) fn received(transcripts: &[PathBuf], now: u64) -> HashMap<PathBuf, Vec<Received>> {
    let mut guard = TAILS.lock().unwrap_or_else(|e| e.into_inner());
    let tails = guard.get_or_insert_with(HashMap::new);
    // Transcripts no session writes any more are forgotten.
    let wanted: HashSet<&PathBuf> = transcripts.iter().collect();
    tails.retain(|p, _| wanted.contains(p));
    let mut out = HashMap::new();
    for path in transcripts {
        let tail = tails.entry(path.clone()).or_default();
        let _ = catch_up(path, tail);
        tail.got.retain(|(_, at)| at + HISTORY_SECS >= now);
        out.insert(path.clone(), tail.got.clone());
    }
    out
}

fn catch_up(path: &Path, tail: &mut Tail) -> std::io::Result<()> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let mut start = tail.offset;
    if len < start || start == 0 {
        // New to us, or written over: begin near the end.
        tail.got.clear();
        start = len.saturating_sub(FIRST_TAIL);
    }
    if len - start > MAX_STEP {
        start = len - MAX_STEP;
    }
    let skip_partial = start > 0 && start != tail.offset;
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity((len - start) as usize);
    file.take(len - start).read_to_end(&mut buf)?;
    let Some(last_newline) = buf.iter().rposition(|b| *b == b'\n') else { return Ok(()) };
    let whole = &buf[..last_newline];
    let mut lines = whole.split(|b| *b == b'\n');
    if skip_partial {
        lines.next(); // begun mid-line
    }
    tail.got.extend(lines.filter_map(peer_message));
    tail.offset = start + last_newline as u64 + 1;
    Ok(())
}

/// A message another session sent, as its transcript line records it.
fn peer_message(line: &[u8]) -> Option<Received> {
    // Most lines are not: skip them before parsing.
    if !contains(line, br#""kind":"peer""#) {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(line).ok()?;
    if v.get("type")?.as_str()? != "user" {
        return None;
    }
    let origin = v.get("origin")?;
    if origin.get("kind")?.as_str()? != "peer" || origin.get("verifiedPeerPid").is_none() {
        return None;
    }
    let sock = origin.get("from")?.as_str()?.strip_prefix("uds:")?.to_string();
    let at = chrono::DateTime::parse_from_rfc3339(v.get("timestamp")?.as_str()?).ok()?;
    Some((sock, u64::try_from(at.timestamp()).ok()?))
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Which sessions lead which, from messages `(from, to, when)` between them.
#[derive(Default, Debug, PartialEq)]
pub(crate) struct Links {
    pub leads: HashMap<String, Vec<String>>,
    pub orchestrators: HashSet<String>,
}

pub(crate) fn links(messages: &[(String, String, u64)], now: u64) -> Links {
    // Each pair, with when it first and last spoke either way, and who first wrote.
    let mut pairs: HashMap<(&str, &str), (u64, u64, &str)> = HashMap::new();
    for (from, to, at) in messages {
        if from == to {
            continue;
        }
        let key = if from < to { (from.as_str(), to.as_str()) } else { (to.as_str(), from.as_str()) };
        let e = pairs.entry(key).or_insert((*at, *at, from.as_str()));
        if *at < e.0 || (*at == e.0 && from.as_str() < e.2) {
            e.0 = *at;
            e.2 = from.as_str();
        }
        e.1 = e.1.max(*at);
    }
    pairs.retain(|_, (_, last, _)| *last + WINDOW_SECS >= now);
    let mut degree: HashMap<&str, usize> = HashMap::new();
    for (a, b) in pairs.keys() {
        *degree.entry(a).or_default() += 1;
        *degree.entry(b).or_default() += 1;
    }
    let mut out = Links::default();
    let mut led: HashSet<&str> = HashSet::new();
    for (&(a, b), &(_, _, first)) in &pairs {
        let (from, to) = match degree[a].cmp(&degree[b]) {
            std::cmp::Ordering::Greater => (a, b),
            std::cmp::Ordering::Less => (b, a),
            std::cmp::Ordering::Equal if first == a => (a, b),
            std::cmp::Ordering::Equal => (b, a),
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
mod tests {
    use super::*;

    fn m(from: &str, to: &str, at: u64) -> (String, String, u64) {
        (from.into(), to.into(), at)
    }

    const NOW: u64 = 100_000;

    #[test]
    fn the_one_handing_out_work_leads_even_when_a_worker_spoke_first_lately() {
        // Seen live: mpcf's answer is the oldest message left, yet art hands out the work.
        let l = links(&[m("mpcf", "art", NOW - 50), m("art", "mpcf", NOW - 40), m("art", "nabi", NOW - 30), m("nabi", "art", NOW - 20)], NOW);
        assert_eq!(l.leads["art"], vec!["mpcf", "nabi"]);
        assert_eq!(l.leads.len(), 1);
        assert_eq!(l.orchestrators, HashSet::from(["art".to_string()]));
    }

    #[test]
    fn between_two_alone_the_first_to_write_leads() {
        let l = links(&[m("b", "a", NOW - 10), m("a", "b", NOW - 20)], NOW);
        assert_eq!(l.leads["a"], vec!["b"]);
        assert_eq!(l.orchestrators, HashSet::from(["a".to_string()]));
    }

    #[test]
    fn a_pair_quiet_for_an_hour_is_no_longer_linked() {
        let l = links(&[m("a", "b", NOW - WINDOW_SECS - 1)], NOW);
        assert_eq!(l, Links::default());
    }

    #[test]
    fn one_led_by_another_is_not_the_orchestrator_though_it_leads_too() {
        let l = links(&[m("a", "b", NOW - 3), m("a", "c", NOW - 3), m("a", "d", NOW - 3), m("b", "e", NOW - 2)], NOW);
        assert_eq!(l.orchestrators, HashSet::from(["a".to_string()]));
        assert_eq!(l.leads["b"], vec!["e"]);
    }

    #[test]
    fn only_a_verified_peer_message_counts() {
        let real = br#"{"type":"user","timestamp":"2026-11-05T12:35:08.552Z","origin":{"kind":"peer","from":"uds:/tmp/cc-socks/1673538.sock","verifiedPeerPid":1673538,"body":"..."}}"#;
        assert_eq!(peer_message(real), Some(("/tmp/cc-socks/1673538.sock".into(), 1_793_882_108)));
        // Quoted in a tool result, or claimed without the check: no.
        let quoted = br#"{"type":"user","timestamp":"2026-11-05T12:35:08.552Z","message":{"content":"\"kind\":\"peer\""}}"#;
        assert_eq!(peer_message(quoted), None);
        let unverified = br#"{"type":"user","timestamp":"2026-11-05T12:35:08.552Z","origin":{"kind":"peer","from":"uds:/s"}}"#;
        assert_eq!(peer_message(unverified), None);
    }

    #[test]
    fn a_transcript_is_read_once_and_then_only_what_was_added() {
        let dir = std::env::temp_dir().join(format!("nabiman-links-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.jsonl");
        let line = |sock: &str, ts: &str| format!(
            "{{\"type\":\"user\",\"timestamp\":\"{ts}\",\"origin\":{{\"kind\":\"peer\",\"from\":\"uds:{sock}\",\"verifiedPeerPid\":1}}}}\n");
        fs::write(&path, line("/a", "2026-11-05T12:00:00Z")).unwrap();
        let now = 1_793_880_000 + 60;
        let mut tail = Tail::default();
        catch_up(&path, &mut tail).unwrap();
        assert_eq!(tail.got.len(), 1);
        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        std::io::Write::write_all(&mut f, line("/b", "2026-11-05T12:00:30Z").as_bytes()).unwrap();
        std::io::Write::write_all(&mut f, b"{\"half a line").unwrap();
        catch_up(&path, &mut tail).unwrap();
        assert_eq!(tail.got.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), vec!["/a", "/b"]);
        let got = received(&[path.clone()], now);
        assert_eq!(got[&path].len(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn project_folders_are_named_the_way_claude_code_names_them() {
        assert_eq!(folder_name("/workspace/server/mpcf.or.kr"), "-workspace-server-mpcf-or-kr");
    }
}
