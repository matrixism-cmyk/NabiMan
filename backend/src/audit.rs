use crate::models::ApiResponse;
use actix_web::{web, HttpResponse};
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
    #[serde(default)]
    pub hash: Option<String>,
}

#[path = "audit_inspection.rs"]
mod inspection;
#[path = "audit_storage.rs"]
mod storage;
pub type AuditLog = Arc<Mutex<storage::State>>;
pub fn new_audit_log() -> AuditLog {
    Arc::new(Mutex::new(storage::State::load(audit_file_path())))
}
fn audit_file_path() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("audit.jsonl")
}

fn compute_hash(prev_hash: &str, entry: &AuditEntry) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    if entry.hash.as_deref().is_some_and(|h| h.starts_with("v2:")) {
        hasher.update(b"nabiman-audit-v2\0");
        hasher.update(
            serde_json::to_vec(&(
                prev_hash,
                &entry.timestamp,
                &entry.user,
                &entry.method,
                &entry.path,
                entry.status,
                &entry.ip,
            ))
            .expect("string tuple serialization"),
        );
        return format!("v2:{}", hex::encode(hasher.finalize()));
    }
    hasher.update(prev_hash);
    hasher.update(&entry.timestamp);
    hasher.update(&entry.user);
    hasher.update(&entry.method);
    hasher.update(&entry.path);
    hasher.update(entry.status.to_string());
    hasher.update(&entry.ip);
    hex::encode(hasher.finalize())
}

pub fn append_entry(store: &AuditLog, entry: AuditEntry) -> Option<AuditEntry> {
    store.lock().unwrap().append(entry).ok()
}
#[cfg(test)]
#[path = "audit_append_tests.rs"]
mod append_tests;

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
    let filtered: Vec<AuditEntry> = log
        .entries
        .iter()
        .rev()
        .filter(|e| {
            query
                .user
                .as_ref()
                .map_or(true, |u| e.user.contains(u.as_str()))
                && query.method.as_ref().map_or(true, |m| &e.method == m)
                && query
                    .path_filter
                    .as_ref()
                    .map_or(true, |p| e.path.contains(p.as_str()))
        })
        .take(limit)
        .cloned()
        .collect();
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(filtered))
}

async fn clear_audit_log() -> HttpResponse {
    HttpResponse::Gone().json(ApiResponse::<()>::error(
        "감사 원본은 웹에서 삭제할 수 없습니다. 보존 정책에 따른 오프라인 관리가 필요합니다.",
    ))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/audit")
            .route("", web::get().to(get_audit_log))
            .route("/clear", web::post().to(clear_audit_log))
            .route("/verify", web::get().to(inspection::verify))
            .route("/health", web::get().to(inspection::health))
            .route("/export", web::post().to(inspection::export)),
    );
}
