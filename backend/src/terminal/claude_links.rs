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
mod tests {
    use super::*;

    fn m(from: &str, to: &str, at: u64) -> (String, String, u64) {
        (from.into(), to.into(), at)
    }

    const NOW: u64 = 100_000;

    #[test]
    fn the_first_to_write_leads_and_answers_do_not_turn_it_round() {
        // art handed out work hours ago; the answers since are the latest.
        let l = links(&[m("art", "mpcf", NOW - 9000), m("mpcf", "art", NOW - 50), m("art", "nabi", NOW - 8000), m("nabi", "art", NOW - 20)], NOW, None);
        assert_eq!(l.leads["art"], vec!["mpcf", "nabi"]);
        assert_eq!(l.leads.len(), 1);
        assert_eq!(l.orchestrators, HashSet::from(["art".to_string()]));
    }

    #[test]
    fn reports_back_to_the_lead_do_not_make_the_reporters_lead() {
        // nabiman tells everyone; each reports back, and one asks another for data.
        let l = links(&[
            m("nabi", "a", NOW - 900), m("nabi", "b", NOW - 900), m("a", "nabi", NOW - 600), m("b", "nabi", NOW - 500),
            m("a", "b", NOW - 300), m("b", "a", NOW - 200), m("a", "nabi", NOW - 100),
        ], NOW, None);
        assert_eq!(l.leads["nabi"], vec!["a", "b"]);
        assert_eq!(l.leads["a"], vec!["b"]);
        assert_eq!(l.orchestrators, HashSet::from(["nabi".to_string()]));
    }

    #[test]
    fn the_lead_named_by_the_operator_leads_every_pair_it_is_in() {
        // Reports reach the lead first (an old exchange): still drawn from the lead.
        let l = links(&[m("a", "lead", NOW - 600), m("lead", "a", NOW - 500), m("b", "lead", NOW - 100), m("a", "b", NOW - 50)], NOW, Some("lead"));
        assert_eq!(l.leads["lead"], vec!["a", "b"]);
        assert_eq!(l.leads["a"], vec!["b"]);
        assert_eq!(l.orchestrators, HashSet::from(["lead".to_string()]));
    }

    #[test]
    fn yesterdays_exchange_does_not_decide_todays() {
        // b led a yesterday; today a starts afresh after a long quiet spell.
        let l = links(&[m("b", "a", NOW - 86_400), m("a", "b", NOW - 86_300), m("a", "b", NOW - 600), m("b", "a", NOW - 300)], NOW, None);
        assert_eq!(l.leads["a"], vec!["b"]);
        assert!(!l.leads.contains_key("b"));
    }

    #[test]
    fn a_pair_quiet_for_an_hour_is_no_longer_linked() {
        let l = links(&[m("a", "b", NOW - WINDOW_SECS - 1), m("b", "a", NOW - WINDOW_SECS - 1)], NOW, None);
        assert_eq!(l, Links::default());
    }

    #[test]
    fn one_led_by_another_is_not_the_orchestrator_though_it_leads_too() {
        let l = links(&[m("a", "b", NOW - 3), m("a", "c", NOW - 3), m("a", "d", NOW - 3), m("b", "e", NOW - 2)], NOW, None);
        assert_eq!(l.orchestrators, HashSet::from(["a".to_string()]));
        assert_eq!(l.leads["b"], vec!["e"]);
    }

    #[test]
    fn only_a_verified_peer_message_counts() {
        let real = br#"{"type":"user","timestamp":"2026-11-05T12:35:08.552Z","origin":{"kind":"peer","from":"uds:/tmp/cc-socks/1673538.sock","verifiedPeerPid":1673538,"body":"..."}}"#;
        assert_eq!(peer_message(real), Some(("uds:/tmp/cc-socks/1673538.sock".into(), 1_793_882_108, String::new(), String::new())));
        // Arrived mid-turn: an attachment, with the same checked origin.
        let queued = br#"{"type":"attachment","timestamp":"2026-11-05T12:53:54.219Z","attachment":{"type":"queued_command","origin":{"kind":"peer","from":"uds:/s","verifiedPeerPid":1,"msg_id":"m1"}}}"#;
        assert_eq!(peer_message(queued), Some(("uds:/s".into(), 1_793_883_234, "m1".into(), String::new())));
        // From another machine, over Remote Control.
        let bridged = br#"{"type":"user","timestamp":"2026-11-05T14:56:18.111Z","origin":{"kind":"peer","from":"bridge:session_01WD","msg_id":"m2"}}"#;
        assert_eq!(peer_message(bridged), Some(("bridge:session_01WD".into(), 1_793_890_578, "m2".into(), String::new())));
        // What it said, from either form, without the envelope.
        let said = br#"{"type":"attachment","timestamp":"2026-11-05T12:53:54.219Z","attachment":{"type":"queued_command","prompt":"<cross-session-message from=\"uds:/s\" from-name=\"a\">\n\n  Review the PR, then report.\nMore lines.\n</cross-session-message>","origin":{"kind":"peer","from":"uds:/s","verifiedPeerPid":1}}}"#;
        assert_eq!(peer_message(said).unwrap().3, "Review the PR, then report.");
        let parts = r#"{"type":"user","timestamp":"2026-11-05T12:53:54.219Z","message":{"content":[{"type":"text","text":"Another Claude session sent a message:\n<cross-session-message from=\"bridge:x\">\n작업 요청: 배포 \u0007확인\n</cross-session-message>"}]},"origin":{"kind":"peer","from":"bridge:x"}}"#;
        assert_eq!(peer_message(parts.as_bytes()).unwrap().3, "작업 요청: 배포 확인");
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
        let mut tail = Tail::default();
        catch_up(&path, &mut tail).unwrap();
        assert_eq!(tail.got.len(), 1);
        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        std::io::Write::write_all(&mut f, line("/b", "2026-11-05T12:00:30Z").as_bytes()).unwrap();
        std::io::Write::write_all(&mut f, b"{\"half a line").unwrap();
        catch_up(&path, &mut tail).unwrap();
        assert_eq!(tail.got.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), vec!["uds:/a", "uds:/b"]);
        // In the background: the first look may come back before the reading is done.
        let _ = received(std::slice::from_ref(&path));
        let mut got = Vec::new();
        for _ in 0..200 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            got = received(std::slice::from_ref(&path)).remove(&path).unwrap_or_default();
            if got.len() == 2 {
                break;
            }
        }
        assert_eq!(got.len(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn project_folders_are_named_the_way_claude_code_names_them() {
        assert_eq!(folder_name("/workspace/server/mpcf.or.kr"), "-workspace-server-mpcf-or-kr");
    }
}
