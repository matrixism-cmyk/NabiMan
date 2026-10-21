//! Copying files from one file panel's place to another's — between two
//! saved servers, or a server and this machine — relayed through NabiMan
//! (`copy_run`). Reaching either side is checked exactly as the panels'
//! own requests are; a copy is an account's alone (share links have one
//! panel, so they have nothing to copy between).
//!
//! A copy runs on its own after the request that starts it, so it survives
//! the browser closing; the panel asks how it is going.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use super::paths::{base_name, join, parent, valid_path};
use super::{account_target, bad, fail};
use crate::models::ApiResponse;

/// Copies one account may have running at once.
const RUNNING_PER_ACCOUNT: usize = 4;
/// Paths one request may name.
const MAX_PATHS: usize = 1000;
/// How long a finished copy stays to be seen.
const KEEP_FINISHED_SECS: u64 = 3600;

#[derive(Clone, Default, Serialize)]
pub(super) struct Progress {
    /// `copying`, then `done`, `failed` or `cancelled`.
    state: &'static str,
    bytes_total: u64,
    bytes_done: u64,
    files_total: usize,
    files_done: usize,
    /// The file being copied, below what was dropped.
    current: String,
    message: String,
    /// What was left out, and why (the first few).
    skipped: Vec<String>,
    skipped_count: usize,
    /// Whether the size of everything is known yet.
    planned: bool,
    finished_unix: Option<u64>,
}

pub(super) struct Job {
    pub id: String,
    owner: String,
    from: (String, String),
    to: (String, String),
    dir: String,
    names: Vec<String>,
    started_unix: u64,
    cancel: AtomicBool,
    progress: Mutex<Progress>,
}

impl Job {
    fn update(&self, f: impl FnOnce(&mut Progress)) {
        f(&mut self.progress.lock().unwrap_or_else(|e| e.into_inner()));
    }
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    pub fn note(&self, note: String) {
        self.update(|p| {
            if p.skipped.len() < super::copy_run::max_notes() {
                p.skipped.push(note);
            }
            p.skipped_count += 1;
        });
    }
    pub fn planned(&self, bytes: u64, files: usize) {
        self.update(|p| (p.bytes_total, p.files_total, p.planned) = (bytes, files, true));
    }
    pub fn working_on(&self, label: &str) {
        self.update(|p| p.current = label.to_string());
    }
    pub fn moved(&self, bytes: u64) {
        self.update(|p| p.bytes_done += bytes);
    }
    pub fn file_done(&self) {
        self.update(|p| p.files_done += 1);
    }
    pub fn finish(&self, state: &'static str, message: String) {
        self.update(|p| {
            p.state = state;
            p.message = message;
            p.current.clear();
            p.finished_unix = Some(now());
        });
    }
    fn running(&self) -> bool {
        self.progress.lock().map(|p| p.finished_unix.is_none()).unwrap_or(false)
    }
    fn view(&self) -> serde_json::Value {
        let p = self.progress.lock().map(|p| p.clone()).unwrap_or_default();
        serde_json::json!({
            "id": self.id, "from": { "scope": self.from.0, "name": self.from.1 },
            "to": { "scope": self.to.0, "name": self.to.1 }, "dir": self.dir, "names": self.names,
            "started_unix": self.started_unix, "progress": p,
        })
    }
}

fn now() -> u64 {
    super::super::session::now_unix()
}

fn jobs() -> &'static Mutex<HashMap<String, Arc<Job>>> {
    static JOBS: OnceLock<Mutex<HashMap<String, Arc<Job>>>> = OnceLock::new();
    JOBS.get_or_init(Default::default)
}

fn prune(map: &mut HashMap<String, Arc<Job>>) {
    let now = now();
    map.retain(|_, j| {
        j.progress.lock().map(|p| p.finished_unix.is_none_or(|t| now.saturating_sub(t) < KEEP_FINISHED_SECS)).unwrap_or(false)
    });
}

#[derive(Deserialize)]
pub(super) struct Start {
    /// Where the files are: a saved server id, or `local`.
    from: String,
    paths: Vec<String>,
    to: String,
    /// The folder they go into.
    dir: String,
    /// Replace what is already there by the same name (asked first).
    #[serde(default)]
    overwrite: bool,
}

fn label(t: &super::Target) -> String {
    t.server.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| t.scope.clone())
}

pub(super) async fn start(req: HttpRequest, b: web::Json<Start>) -> HttpResponse {
    if b.paths.is_empty() || b.paths.len() > MAX_PATHS {
        return bad("Choose what to copy");
    }
    if !valid_path(&b.dir) || b.paths.iter().any(|p| !valid_path(p) || parent(p).is_none()) {
        return bad("Invalid path");
    }
    if b.from == b.to {
        let dir = if b.dir.trim_end_matches('/').is_empty() { "/" } else { b.dir.trim_end_matches('/') };
        for p in &b.paths {
            if parent(p).as_deref() == Some(dir) {
                return bad("It is already in that folder");
            }
            if b.dir == *p || b.dir.starts_with(&format!("{}/", p.trim_end_matches('/'))) {
                return bad("A folder cannot be copied into itself");
            }
        }
    }
    let owner = match crate::auth::verified_user_id(&req) {
        Some(id) => id,
        None => return fail(StatusCode::UNAUTHORIZED, "Sign in again"),
    };
    let from = match account_target(&req, &b.from) { Ok(t) => t, Err(r) => return r };
    let to = match account_target(&req, &b.to) { Ok(t) => t, Err(r) => return r };
    let src = match from.fs().await { Ok(fs) => fs, Err(r) => return r };
    let dst = match to.fs().await { Ok(fs) => fs, Err(r) => return r };
    match dst.stat(&b.dir).await {
        Ok(m) if m.is_dir() => {}
        Ok(_) => return bad("Copy into a folder"),
        Err(e) => return to.error(e),
    }
    let names: Vec<String> = b.paths.iter().map(|p| base_name(p).to_string()).collect();
    let mut conflicts = Vec::new();
    for name in &names {
        if dst.lstat(&join(&b.dir, name)).await.is_ok() {
            conflicts.push(name.clone());
        }
    }
    if !conflicts.is_empty() && !b.overwrite {
        return HttpResponse::Conflict().json(serde_json::json!({
            "success": false, "data": { "conflicts": conflicts }, "message": "Something by that name is already there",
        }));
    }
    let mut id = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut id);
    let job = Arc::new(Job {
        id: hex::encode(id),
        owner: owner.clone(),
        from: (b.from.clone(), label(&from)),
        to: (b.to.clone(), label(&to)),
        dir: b.dir.clone(),
        names,
        started_unix: now(),
        cancel: AtomicBool::new(false),
        progress: Mutex::new(Progress { state: "copying", ..Progress::default() }),
    });
    {
        let mut map = jobs().lock().unwrap_or_else(|e| e.into_inner());
        prune(&mut map);
        if map.values().filter(|j| j.owner == owner && j.running()).count() >= RUNNING_PER_ACCOUNT {
            return fail(StatusCode::TOO_MANY_REQUESTS, "Wait for a copy to finish before starting another");
        }
        map.insert(job.id.clone(), job.clone());
    }
    let view = job.view();
    actix_web::rt::spawn(super::copy_run::run(job, src, dst, b.paths.clone(), b.dir.clone()));
    HttpResponse::Ok().json(ApiResponse::ok(view))
}

/// The account's copies, running and lately finished, oldest first.
pub(super) async fn list(req: HttpRequest) -> HttpResponse {
    let Some(owner) = crate::auth::verified_user_id(&req) else {
        return fail(StatusCode::UNAUTHORIZED, "Sign in again");
    };
    let mut map = jobs().lock().unwrap_or_else(|e| e.into_inner());
    prune(&mut map);
    let mut mine: Vec<&Arc<Job>> = map.values().filter(|j| j.owner == owner).collect();
    mine.sort_by_key(|j| j.started_unix);
    let out: Vec<serde_json::Value> = mine.iter().map(|j| j.view()).collect();
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(out))
}

pub(super) async fn cancel(req: HttpRequest, path: web::Path<String>) -> HttpResponse {
    let Some(owner) = crate::auth::verified_user_id(&req) else {
        return fail(StatusCode::UNAUTHORIZED, "Sign in again");
    };
    let job = jobs().lock().unwrap_or_else(|e| e.into_inner()).get(path.as_str()).cloned();
    match job {
        Some(job) if job.owner == owner => {
            job.cancel.store(true, Ordering::Relaxed);
            HttpResponse::Ok().json(ApiResponse::ok(job.view()))
        }
        _ => fail(StatusCode::NOT_FOUND, "No such copy"),
    }
}

pub(super) fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/copies", web::get().to(list))
        .route("/api/terminal/copies", web::post().to(start))
        .route("/api/terminal/copies/{job}/cancel", web::post().to(cancel));
}

#[cfg(test)]
impl Job {
    pub(super) fn for_test() -> Arc<Job> {
        Arc::new(Job {
            id: "0123456789abcdef0123456789abcdef".into(),
            owner: "test".into(),
            from: ("local".into(), "local".into()),
            to: ("local".into(), "local".into()),
            dir: "/".into(),
            names: Vec::new(),
            started_unix: 0,
            cancel: AtomicBool::new(false),
            progress: Mutex::new(Progress { state: "copying", ..Progress::default() }),
        })
    }
    pub(super) fn test_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub(super) fn test_progress(&self) -> serde_json::Value {
        self.view()["progress"].clone()
    }
}
