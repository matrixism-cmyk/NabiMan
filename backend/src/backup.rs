#[path = "backup_encrypted.rs"]
mod encrypted;
use encrypted::{create_encrypted, restore_encrypted};

use crate::models::ApiResponse;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
pub struct BackupEntry {
    pub id: String,
    pub source: String,
    pub filename: String,
    pub size: u64,
    pub created: String,
}

#[path = "backup_archive.rs"]
mod archive;
#[path = "backup_schedule.rs"]
mod schedule;
pub use schedule::{new_schedule_store, start_backup_scheduler};

fn backup_dir() -> std::path::PathBuf {
    nabiman_server::data_dir().join("backups")
}

async fn list_backups() -> HttpResponse {
    let dir = backup_dir();
    if !dir.exists() {
        return HttpResponse::Ok().json(ApiResponse::ok(Vec::<BackupEntry>::new()));
    }
    let mut entries = Vec::new();
    let rd = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => {
            return HttpResponse::ServiceUnavailable()
                .json(ApiResponse::<()>::error("Backup storage unavailable"))
        }
    };
    {
        for entry in rd.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !meta.is_file() || entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let created = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| {
                    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            let source = name.rsplitn(2, '_').last().unwrap_or("").replace("__", "/");
            entries.push(BackupEntry {
                id: name.clone(),
                source,
                filename: name,
                size: meta.len(),
                created,
            });
        }
    }
    entries.sort_by(|a, b| b.created.cmp(&a.created));
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(entries))
}

#[derive(Deserialize)]
pub struct BackupRequest {
    pub path: String,
}

async fn create_backup(body: web::Json<BackupRequest>) -> HttpResponse {
    let src = std::path::Path::new(&body.path);
    if !src.exists() || !src.is_file() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("File not found"));
    }
    let dir = backup_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string()));
    }
    let safe_name = body
        .path
        .replace('/', "__")
        .trim_start_matches('_')
        .to_string();
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.bak", safe_name, ts);
    let dest = dir.join(&filename);

    match std::fs::copy(src, &dest) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Backup created: {}", filename))),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

#[derive(Deserialize)]
pub struct RestoreRequest {
    pub filename: String,
    pub target: String,
}

async fn restore_backup(body: web::Json<RestoreRequest>) -> HttpResponse {
    let fname = &body.filename;
    if fname.contains("..") || fname.contains('/') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid filename"));
    }
    let src = backup_dir().join(fname);
    if !src.exists() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Backup file not found"));
    }
    let target = std::path::Path::new(&body.target);
    match std::fs::copy(&src, target) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok("Restored".to_string())),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

#[derive(Deserialize)]
pub struct DeleteBackupRequest {
    pub filename: String,
}

async fn delete_backup(body: web::Json<DeleteBackupRequest>) -> HttpResponse {
    let fname = &body.filename;
    if fname.contains("..") || fname.contains('/') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid filename"));
    }
    let path = backup_dir().join(fname);
    match std::fs::remove_file(&path) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok("Deleted".to_string())),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

// --- Compressed backup ---

#[derive(Deserialize)]
struct CompressedBackupRequest {
    paths: Vec<String>,
    name: Option<String>,
}

async fn create_compressed(body: web::Json<CompressedBackupRequest>) -> HttpResponse {
    let body = body.into_inner();
    match web::block(move || {
        archive::create(
            &backup_dir(),
            body.name.as_deref().unwrap_or("archive"),
            &body.paths,
            true,
        )
    })
    .await
    {
        Ok(Ok(filename)) => HttpResponse::Ok().json(ApiResponse::ok(filename)),
        Ok(Err(error)) => HttpResponse::BadRequest().json(ApiResponse::<String>::error(&error)),
        Err(_) => HttpResponse::ServiceUnavailable()
            .json(ApiResponse::<String>::error("Archive worker unavailable")),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/backup")
            .route("", web::get().to(list_backups))
            .route("/create", web::post().to(create_backup))
            .route("/restore", web::post().to(restore_backup))
            .route("/delete", web::post().to(delete_backup))
            .route("/create-compressed", web::post().to(create_compressed))
            .route("/create-encrypted", web::post().to(create_encrypted))
            .route("/restore-encrypted", web::post().to(restore_encrypted))
            .route("/schedules", web::get().to(schedule::list))
            .route("/schedules", web::post().to(schedule::create))
            .route("/schedules/{id}", web::delete().to(schedule::delete))
            .route("/schedules/{id}/run", web::post().to(schedule::run)),
    );
}
