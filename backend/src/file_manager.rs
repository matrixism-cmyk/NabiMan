use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: String,
    pub permissions: String,
}

#[derive(Serialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub size: u64,
}

#[derive(Deserialize)]
pub struct BrowseRequest {
    pub path: Option<String>,
}

#[derive(Deserialize)]
pub struct ReadFileRequest {
    pub path: String,
}

#[derive(Deserialize)]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
}

fn sanitize_path(p: &str) -> Option<PathBuf> {
    let path = PathBuf::from(p);
    let canonical = std::fs::canonicalize(&path).ok()?;
    // Block /proc, /sys, /dev for safety
    let s = canonical.to_string_lossy();
    if s.starts_with("/proc") || s.starts_with("/sys") || s.starts_with("/dev") {
        return None;
    }
    Some(canonical)
}

async fn browse(query: web::Query<BrowseRequest>) -> HttpResponse {
    let dir_path = query.path.as_deref().unwrap_or("/");
    let dir = match sanitize_path(dir_path) {
        Some(p) if p.is_dir() => p,
        _ => return HttpResponse::Ok().json(
            ApiResponse::<Vec<FileEntry>>::error("Invalid or inaccessible directory")
        ),
    };

    let mut entries = Vec::new();
    if let Some(parent) = dir.parent() {
        entries.push(FileEntry {
            name: "..".into(),
            path: parent.to_string_lossy().to_string(),
            is_dir: true, size: 0, modified: String::new(), permissions: String::new(),
        });
    }

    match std::fs::read_dir(&dir) {
        Ok(rd) => {
            let mut items: Vec<FileEntry> = rd.filter_map(|e| {
                let e = e.ok()?;
                let meta = e.metadata().ok()?;
                let modified = meta.modified().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default())
                    .unwrap_or_default();
                let perms = format_permissions(&meta);
                Some(FileEntry {
                    name: e.file_name().to_string_lossy().to_string(),
                    path: e.path().to_string_lossy().to_string(),
                    is_dir: meta.is_dir(),
                    size: meta.len(),
                    modified, permissions: perms,
                })
            }).collect();
            items.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
            entries.extend(items);
        }
        Err(e) => return HttpResponse::Ok().json(
            ApiResponse::<Vec<FileEntry>>::error(&e.to_string())
        ),
    }
    HttpResponse::Ok().json(ApiResponse::ok(entries))
}

fn format_permissions(meta: &std::fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    let mode = meta.permissions().mode();
    format!("{:o}", mode & 0o7777)
}

async fn read_file(body: web::Json<ReadFileRequest>) -> HttpResponse {
    let path = match sanitize_path(&body.path) {
        Some(p) if p.is_file() => p,
        _ => return HttpResponse::Ok().json(ApiResponse::<FileContent>::error("Invalid file")),
    };
    let meta = match std::fs::metadata(&path) {
        Ok(m) => m, Err(e) => return HttpResponse::Ok().json(
            ApiResponse::<FileContent>::error(&e.to_string())),
    };
    if meta.len() > 2 * 1024 * 1024 {
        return HttpResponse::Ok().json(ApiResponse::<FileContent>::error("File too large (>2MB)"));
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => HttpResponse::Ok().json(ApiResponse::ok(FileContent {
            path: path.to_string_lossy().to_string(), content, size: meta.len(),
        })),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<FileContent>::error(&e.to_string())),
    }
}

async fn write_file(body: web::Json<WriteFileRequest>) -> HttpResponse {
    let path = match sanitize_path(&body.path) {
        Some(p) => p,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid path")),
    };
    match std::fs::write(&path, &body.content) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok("File saved".to_string())),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/files")
            .route("/browse", web::get().to(browse))
            .route("/read", web::post().to(read_file))
            .route("/write", web::post().to(write_file))
    );
}
