use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub timestamp: String,
    pub user: String,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub ip: String,
}

pub type AuditLog = Arc<Mutex<Vec<AuditEntry>>>;

pub fn new_audit_log() -> AuditLog {
    let entries = load_from_file();
    Arc::new(Mutex::new(entries))
}

fn audit_file_path() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("audit.jsonl")
}

fn load_from_file() -> Vec<AuditEntry> {
    let path = audit_file_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    content.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

pub fn append_entry(store: &AuditLog, entry: AuditEntry) {
    if let Ok(line) = serde_json::to_string(&entry) {
        let fpath = audit_file_path();
        if let Some(p) = fpath.parent() { let _ = std::fs::create_dir_all(p); }
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&fpath) {
            let _ = writeln!(f, "{}", line);
        }
        // Rotate if > 10MB
        if fpath.metadata().map(|m| m.len() > 10_000_000).unwrap_or(false) {
            rotate_log(&fpath);
        }
    }
    let mut log = store.lock().unwrap();
    log.push(entry);
    let len = log.len();
    if len > 1000 { log.drain(0..len - 1000); }
}

fn rotate_log(path: &std::path::Path) {
    for i in (1..5).rev() {
        let from = path.with_extension(format!("jsonl.{}", i));
        let to = path.with_extension(format!("jsonl.{}", i + 1));
        let _ = std::fs::rename(&from, &to);
    }
    let backup = path.with_extension("jsonl.1");
    let _ = std::fs::rename(path, &backup);
}

#[derive(Deserialize)]
struct AuditQuery {
    user: Option<String>,
    method: Option<String>,
    path_filter: Option<String>,
    limit: Option<usize>,
}

async fn get_audit_log(store: web::Data<AuditLog>, query: web::Query<AuditQuery>) -> HttpResponse {
    let log = store.lock().unwrap();
    let limit = query.limit.unwrap_or(200).min(500);
    let filtered: Vec<AuditEntry> = log.iter().rev()
        .filter(|e| {
            query.user.as_ref().map_or(true, |u| e.user.contains(u.as_str()))
            && query.method.as_ref().map_or(true, |m| &e.method == m)
            && query.path_filter.as_ref().map_or(true, |p| e.path.contains(p.as_str()))
        })
        .take(limit)
        .cloned()
        .collect();
    HttpResponse::Ok().json(ApiResponse::ok(filtered))
}

async fn clear_audit_log(store: web::Data<AuditLog>) -> HttpResponse {
    store.lock().unwrap().clear();
    let _ = std::fs::write(audit_file_path(), "");
    HttpResponse::Ok().json(ApiResponse::ok("Audit log cleared"))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/audit")
            .route("", web::get().to(get_audit_log))
            .route("/clear", web::post().to(clear_audit_log))
    );
}
