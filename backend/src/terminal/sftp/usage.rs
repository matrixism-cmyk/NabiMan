//! Properties: what the chosen entries are, and how much is inside them —
//! folders, files and bytes, counted all the way down.
//!
//! Counting a tree can take long (a home folder full of `node_modules`), so
//! the answer is a stream of JSON lines: first what the entries are, then the
//! running totals a few times a second, then the totals with `done`. The
//! count stops at a time and an entry limit and says which one it hit; closing
//! the request stops it too. Links are counted, never followed, and
//! `/proc`, `/sys` and `/dev` are not gone into.
//!
//! This machine is counted directly, and a server with an SSH shell by its own
//! `find` in one go; an SFTP-only account and FTP walk the folders over the
//! file connection (see `usage_walk`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::fs::Fs;
use super::paths::{base_name, entry, valid_path, Entry};
use super::usage_walk;
use super::{bad, resolve, Target};

/// At most this many chosen paths per request.
const MAX_PATHS: usize = 1000;
const TICK: Duration = Duration::from_millis(300);

/// How far a count goes: an account's, and a share link visitor's.
pub(super) struct Limits {
    pub time: Duration,
    pub entries: u64,
}
const ACCOUNT: Limits = Limits { time: Duration::from_secs(120), entries: 2_000_000 };
const VISITOR: Limits = Limits { time: Duration::from_secs(30), entries: 200_000 };

/// Never gone into: they are not files, and `/proc` alone is endless.
pub(super) const SYSTEM: [&str; 3] = ["/proc", "/sys", "/dev"];

#[derive(Default, Clone, Copy, Serialize, PartialEq, Debug)]
pub(super) struct Count {
    pub dirs: u64,
    pub files: u64,
    pub links: u64,
    pub other: u64,
    /// The files' sizes added up (not what they take on disk).
    pub bytes: u64,
    /// Folders that could not be read.
    pub unreadable: u64,
}

impl Count {
    pub fn add(&mut self, kind: &str, size: u64) {
        match kind {
            "dir" => self.dirs += 1,
            "file" => {
                self.files += 1;
                self.bytes += size;
            }
            "link" => self.links += 1,
            _ => self.other += 1,
        }
    }
    pub fn merge(&mut self, o: &Count) {
        self.dirs += o.dirs;
        self.files += o.files;
        self.links += o.links;
        self.other += o.other;
        self.bytes += o.bytes;
        self.unreadable += o.unreadable;
    }
    pub fn entries(&self) -> u64 {
        self.dirs + self.files + self.links + self.other
    }
}

/// One count in progress, shared by whoever counts and whoever reports.
pub(super) struct Job {
    pub count: Mutex<Count>,
    stop: AtomicBool,
    capped: Mutex<Option<&'static str>>,
    deadline: Instant,
    entries: u64,
}

impl Job {
    fn new(limits: &Limits) -> Self {
        Job {
            count: Mutex::default(),
            stop: AtomicBool::new(false),
            capped: Mutex::new(None),
            deadline: Instant::now() + limits.time,
            entries: limits.entries,
        }
    }
    pub fn tally(&self, f: impl FnOnce(&mut Count)) {
        f(&mut self.count.lock().unwrap_or_else(|e| e.into_inner()));
    }
    pub fn snapshot(&self) -> Count {
        *self.count.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Whether to stop now: asked to, out of time, or past the entry limit.
    pub fn over(&self) -> bool {
        if self.stop.load(Ordering::Relaxed) {
            return true;
        }
        let why = if Instant::now() > self.deadline {
            "time"
        } else if self.snapshot().entries() >= self.entries {
            "entries"
        } else {
            return false;
        };
        self.capped.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert(why);
        self.stop.store(true, Ordering::Relaxed);
        true
    }
    pub fn halt(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
    /// Stops for a reason of its own, such as a lost connection.
    pub fn give_up(&self, why: &'static str) {
        self.capped.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert(why);
        self.halt();
    }
    #[cfg(test)]
    pub fn for_test() -> Self {
        Job::new(&ACCOUNT)
    }
    fn capped(&self) -> Option<&'static str> {
        *self.capped.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Deserialize)]
pub(super) struct UsageBody {
    paths: Vec<String>,
}

#[derive(Serialize)]
struct Item {
    path: String,
    #[serde(flatten)]
    entry: Entry,
    /// Where a link points.
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
}

/// What is directly in one folder.
#[derive(Default, Serialize)]
struct Inside {
    dirs: u64,
    files: u64,
    links: u64,
    other: u64,
    hidden: u64,
}

type Tx = mpsc::Sender<Result<web::Bytes, std::io::Error>>;

async fn line(tx: &Tx, value: serde_json::Value) -> bool {
    tx.send(Ok(web::Bytes::from(format!("{value}\n")))).await.is_ok()
}

/// `POST …/usage` with `{"paths": [...]}`: JSON lines, as the module says.
pub(super) async fn usage(req: HttpRequest, b: web::Json<UsageBody>) -> HttpResponse {
    let t = match resolve(&req) {
        Ok(t) => t,
        Err(r) => return r,
    };
    // Whole paths only: a server's `find` must never take one for an option.
    if b.paths.is_empty() || b.paths.len() > MAX_PATHS || !b.paths.iter().all(|p| valid_path(p) && p.starts_with('/')) {
        return bad("Invalid selection");
    }
    let fs = match t.fs().await {
        Ok(fs) => fs,
        Err(r) => return r,
    };
    let mut items = Vec::new();
    for path in &b.paths {
        match fs.lstat(path).await {
            Ok(meta) => {
                let target = if meta.kind == "link" { fs.read_link(path).await } else { None };
                let mut row = entry(base_name(path).to_string(), &meta);
                if row.kind == "link" {
                    row.to_dir = fs.stat(path).await.is_ok_and(|m| m.is_dir());
                }
                items.push(Item { path: path.clone(), entry: row, target });
            }
            Err(e) => return t.error(e),
        }
    }
    let names = super::attrs::names(&t).await;
    if let Some(names) = &names {
        let mut rows: Vec<Entry> = items.iter().map(|i| i.entry.clone()).collect();
        super::attrs::name_entries(&mut rows, names);
        for (item, row) in items.iter_mut().zip(rows) {
            item.entry = row;
        }
    }
    // One folder: what is directly in it, before the long count.
    let inside = match items.as_slice() {
        [only] if only.entry.kind == "dir" => match fs.read_dir(&only.path).await {
            Ok(list) => Some(inside_of(&list)),
            Err(e) if e.broken => return t.error(e),
            Err(_) => None,
        },
        _ => None,
    };
    // The selected folders are counted inside; whatever else was chosen counts itself.
    let mut roots = Vec::new();
    let mut chosen = Count::default();
    for item in &items {
        if item.entry.kind == "dir" {
            roots.push(item.path.clone());
        } else if items.len() > 1 {
            chosen.add(item.entry.kind, item.entry.size);
        }
    }
    let limits = if t.share.is_some() { &VISITOR } else { &ACCOUNT };
    let job = Arc::new(Job::new(limits));
    job.tally(|c| *c = chosen);
    let first = serde_json::json!({ "items": items, "inside": inside });
    let (tx, rx) = mpsc::channel(8);
    actix_web::rt::spawn(async move {
        if !line(&tx, first).await {
            return;
        }
        let how = count(&t, fs, roots, job.clone(), &tx).await;
        let c = job.snapshot();
        line(&tx, serde_json::json!({ "count": c, "done": true, "capped": job.capped(), "how": how })).await;
    });
    HttpResponse::Ok()
        .content_type("application/x-ndjson")
        .insert_header(("Cache-Control", "no-store"))
        .streaming(tokio_stream::wrappers::ReceiverStream::new(rx))
}

fn inside_of(list: &[(String, super::fs::Meta)]) -> Inside {
    let mut n = Inside::default();
    for (name, meta) in list {
        match meta.kind {
            "dir" => n.dirs += 1,
            "file" => n.files += 1,
            "link" => n.links += 1,
            _ => n.other += 1,
        }
        if name.starts_with('.') {
            n.hidden += 1;
        }
    }
    n
}

/// Counts under `roots`, sending the running totals until done; says how it counted.
async fn count(t: &Target, fs: Fs, roots: Vec<String>, job: Arc<Job>, tx: &Tx) -> &'static str {
    if roots.is_empty() {
        return "none";
    }
    let shell = t.server.clone().filter(|s| s.terminal == "ssh" && s.files == "sftp");
    let worker = {
        let job = job.clone();
        actix_web::rt::spawn(async move {
            match (&fs, shell) {
                (Fs::Local, _) => {
                    let j = job.clone();
                    let _ = tokio::task::spawn_blocking(move || usage_walk::local(&roots, &j)).await;
                    "local"
                }
                (Fs::Remote(_), Some(server)) => {
                    let (j, r) = (job.clone(), roots.clone());
                    let found = tokio::task::spawn_blocking(move || usage_walk::find(&server, &r, &j)).await;
                    if matches!(found, Ok(true)) {
                        return "find";
                    }
                    // No GNU find, or no shell after all: walk over SFTP instead.
                    job.tally(|c| *c = Count::default());
                    usage_walk::walk(&fs, roots, &job).await;
                    "walk"
                }
                _ => {
                    usage_walk::walk(&fs, roots, &job).await;
                    "walk"
                }
            }
        })
    };
    let mut ticker = tokio::time::interval(TICK);
    tokio::pin!(worker);
    loop {
        tokio::select! {
            how = &mut worker => return how.unwrap_or("walk"),
            _ = ticker.tick() => {
                job.over();
                if !line(tx, serde_json::json!({ "count": job.snapshot() })).await {
                    // The panel closed the request: stop counting.
                    job.halt();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_add_up_by_kind_and_only_files_have_bytes() {
        let mut c = Count::default();
        c.add("dir", 4096);
        c.add("file", 10);
        c.add("file", 5);
        c.add("link", 7);
        c.add("other", 0);
        assert_eq!(c, Count { dirs: 1, files: 2, links: 1, other: 1, bytes: 15, unreadable: 0 });
        assert_eq!(c.entries(), 5);
    }

    #[test]
    fn a_job_says_which_limit_stopped_it() {
        let job = Job::new(&Limits { time: Duration::from_secs(60), entries: 3 });
        job.tally(|c| c.files = 2);
        assert!(!job.over());
        job.tally(|c| c.dirs = 1);
        assert!(job.over());
        assert_eq!(job.capped(), Some("entries"));
        let late = Job::new(&Limits { time: Duration::ZERO, entries: 10 });
        std::thread::sleep(Duration::from_millis(2));
        assert!(late.over());
        assert_eq!(late.capped(), Some("time"));
        // Asked to stop is not a limit.
        let asked = Job::new(&ACCOUNT);
        asked.halt();
        assert!(asked.over());
        assert_eq!(asked.capped(), None);
    }
}
