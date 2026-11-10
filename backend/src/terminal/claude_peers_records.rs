//! The records Claude Code keeps of its running sessions
//! (`~/.claude/sessions/<pid>.json`), and whether each is still the process
//! that wrote it.

use std::fs;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::procs::{stat_fields, up_to_pane, Panes};

const MAX_FILES: usize = 400;
const MAX_FILE: u64 = 16 * 1024;

/// Every account's `~/.claude/sessions/*.json`, at most `MAX_FILES` of them.
pub(super) fn session_files() -> Vec<PathBuf> {
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
pub(super) struct Record {
    pub(super) pid: u32,
    pub(super) proc_start: String,
    pub(super) sock: String,
    pub(super) name: String,
    pub(super) cwd: String,
    pub(super) busy: bool,
    /// When `busy` last changed (seconds since 1970).
    pub(super) since: Option<u64>,
    /// Claude Code's version.
    pub(super) version: String,
    pub(super) session_id: String,
    /// Its Remote Control session, when that is on.
    pub(super) bridge: Option<String>,
    /// A Claude Code background session (`kind: bg`), run by its daemon rather than a terminal.
    pub(super) background: bool,
    /// The id `claude attach` takes for a background session (its `jobId`).
    pub(super) job: String,
    /// Who wrote the file.
    pub(super) uid: u32,
    /// The record's own path, under the account's `.claude` folder.
    pub(super) file: PathBuf,
}

pub(super) fn read_record(path: &Path) -> Option<Record> {
    let file = fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE {
        return None;
    }
    let mut text = String::new();
    file.take(MAX_FILE).read_to_string(&mut text).ok()?;
    parse_record(&text, meta.uid(), path)
}

pub(super) fn parse_record(text: &str, uid: u32, file: &Path) -> Option<Record> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let rec = Record {
        pid: u32::try_from(v.get("pid")?.as_u64()?).ok()?,
        proc_start: s("procStart"),
        sock: s("messagingSocketPath"),
        name: clip(&s("name"), 80),
        cwd: clip(&s("cwd"), 300),
        busy: s("status") == "busy",
        since: v.get("statusUpdatedAt").and_then(serde_json::Value::as_u64).map(|ms| ms / 1000),
        version: clip(&s("version"), 20),
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
pub(super) fn alive(rec: &Record) -> bool {
    let proc_dir = PathBuf::from(format!("/proc/{}", rec.pid));
    let owner = fs::metadata(&proc_dir).map(|m| m.uid()).ok();
    let start = fs::read_to_string(proc_dir.join("stat")).ok().and_then(|s| stat_fields(&s)).map(|(start, _)| start);
    owner == Some(rec.uid) && start.as_deref() == Some(rec.proc_start.as_str())
}

/// The tmux session the record's process runs in, if it is still that
/// process, the file's owner runs it, and a pane of this tmux server's is
/// among its parents.
pub(super) fn session_of(rec: &Record, panes: &Panes) -> Option<String> {
    if !alive(rec) {
        return None;
    }
    up_to_pane(rec.pid, panes, |pid| {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        stat_fields(&stat).map(|(_, ppid)| ppid)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_needs_a_socket_and_start_time() {
        let text = r#"{"pid":7,"procStart":"99","messagingSocketPath":"/s","name":"x\u001b[31m","cwd":"/w","status":"busy"}"#;
        let rec = parse_record(text, 1000, Path::new("/h/.claude/sessions/7.json")).unwrap();
        assert_eq!(rec.name, "x[31m");
        assert!(rec.busy);
        assert_eq!(parse_record(r#"{"pid":7,"procStart":"99"}"#, 0, Path::new("/x")), None);
    }
}
