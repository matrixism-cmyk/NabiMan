use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::Serialize;
use std::sync::{Arc, Mutex};

#[derive(Serialize, serde::Deserialize, Clone)]
pub struct AuditEntry {
    pub timestamp: String,
    pub action: String,
    pub path: String,
    pub detail: String,
}

pub type AuditLog = Arc<Mutex<Vec<AuditEntry>>>;

pub fn new_audit_log() -> AuditLog {
    let log = Arc::new(Mutex::new(Vec::new()));
    // Load existing entries from file
    let entries = load_from_file();
    *log.lock().unwrap() = entries;
    log
}

fn audit_file_path() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("audit.jsonl")
}

fn load_from_file() -> Vec<AuditEntry> {
    let path = audit_file_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    content.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

pub fn log_action(store: &AuditLog, action: &str, path: &str, detail: &str) {
    let entry = AuditEntry {
        timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        action: action.to_string(),
        path: path.to_string(),
        detail: detail.to_string(),
    };

    // Append to file
    if let Ok(line) = serde_json::to_string(&entry) {
        let fpath = audit_file_path();
        if let Some(parent) = fpath.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&fpath) {
            let _ = writeln!(f, "{}", line);
        }
    }

    let mut log = store.lock().unwrap();
    log.push(entry);
    // Keep last 1000 entries in memory
    let len = log.len();
    if len > 1000 { log.drain(0..len - 1000); }
}

async fn get_audit_log(store: web::Data<AuditLog>) -> HttpResponse {
    let log = store.lock().unwrap();
    let mut entries: Vec<AuditEntry> = log.iter().rev().take(200).cloned().collect();
    entries.reverse();
    HttpResponse::Ok().json(ApiResponse::ok(entries))
}

async fn clear_audit_log(store: web::Data<AuditLog>) -> HttpResponse {
    store.lock().unwrap().clear();
    let _ = std::fs::write(audit_file_path(), "");
    HttpResponse::Ok().json(ApiResponse::ok("Audit log cleared".to_string()))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/audit")
            .route("", web::get().to(get_audit_log))
            .route("/clear", web::post().to(clear_audit_log))
    );
}
