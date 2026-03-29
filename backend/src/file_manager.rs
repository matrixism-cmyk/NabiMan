use actix_web::{web, HttpRequest, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_UPLOAD_SIZE: u64 = 50 * 1024 * 1024; // 50MB

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

fn sanitize_parent_path(p: &str) -> Option<PathBuf> {
    let path = PathBuf::from(p);
    if let Some(parent) = path.parent() {
        let canonical = std::fs::canonicalize(parent).ok()?;
        let s = canonical.to_string_lossy();
        if s.starts_with("/proc") || s.starts_with("/sys") || s.starts_with("/dev") {
            return None;
        }
        Some(canonical.join(path.file_name()?))
    } else {
        None
    }
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

// --- New endpoints ---

#[derive(Deserialize)]
struct ChmodRequest {
    path: String,
    mode: String,
}

async fn chmod_file(body: web::Json<ChmodRequest>) -> HttpResponse {
    let path = match sanitize_path(&body.path) {
        Some(p) => p,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid path")),
    };
    // Validate mode is octal (3-4 digits)
    if !body.mode.chars().all(|c| c.is_ascii_digit() && c < '8')
        || body.mode.len() < 3 || body.mode.len() > 4
    {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid octal mode"));
    }
    match std::process::Command::new("chmod").arg(&body.mode).arg(&path).output() {
        Ok(o) if o.status.success() => {
            HttpResponse::Ok().json(ApiResponse::ok("Permissions changed".to_string()))
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).to_string();
            HttpResponse::Ok().json(ApiResponse::<String>::error(&err))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

#[derive(Deserialize)]
struct MkdirRequest {
    path: String,
}

async fn mkdir(body: web::Json<MkdirRequest>) -> HttpResponse {
    let target = match sanitize_parent_path(&body.path) {
        Some(p) => p,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid path")),
    };
    match std::fs::create_dir_all(&target) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok("Directory created".to_string())),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

#[derive(Deserialize)]
struct DeleteRequest {
    path: String,
}

async fn delete_file(body: web::Json<DeleteRequest>) -> HttpResponse {
    let path = match sanitize_path(&body.path) {
        Some(p) => p,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid path")),
    };
    let result = if path.is_dir() {
        std::fs::remove_dir(&path) // only removes empty dirs
    } else {
        std::fs::remove_file(&path)
    };
    match result {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok("Deleted".to_string())),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

#[derive(Deserialize)]
struct DownloadQuery {
    path: String,
}

async fn download_file(query: web::Query<DownloadQuery>) -> HttpResponse {
    let path = match sanitize_path(&query.path) {
        Some(p) if p.is_file() => p,
        _ => return HttpResponse::BadRequest().json(ApiResponse::<String>::error("Invalid file")),
    };
    let meta = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(e) => return HttpResponse::BadRequest().json(
            ApiResponse::<String>::error(&e.to_string())
        ),
    };
    if meta.len() > MAX_UPLOAD_SIZE {
        return HttpResponse::BadRequest().json(
            ApiResponse::<String>::error("File too large (>50MB)")
        );
    }
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return HttpResponse::InternalServerError().json(
            ApiResponse::<String>::error(&e.to_string())
        ),
    };
    let fname = path.file_name().map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".into());
    HttpResponse::Ok()
        .insert_header(("Content-Disposition", format!("attachment; filename=\"{}\"", fname)))
        .insert_header(("Content-Type", "application/octet-stream"))
        .body(bytes)
}

async fn upload_file(req: HttpRequest, body: web::Bytes) -> HttpResponse {
    let target_path = match req.headers().get("X-File-Path") {
        Some(v) => v.to_str().unwrap_or("").to_string(),
        None => return HttpResponse::BadRequest().json(
            ApiResponse::<String>::error("Missing X-File-Path header")
        ),
    };
    if body.len() as u64 > MAX_UPLOAD_SIZE {
        return HttpResponse::BadRequest().json(
            ApiResponse::<String>::error("File too large (>50MB)")
        );
    }
    let path = match sanitize_parent_path(&target_path) {
        Some(p) => p,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid path")),
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, &body) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::ok("File uploaded".to_string())),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/files")
            .route("/browse", web::get().to(browse))
            .route("/read", web::post().to(read_file))
            .route("/write", web::post().to(write_file))
            .route("/chmod", web::post().to(chmod_file))
            .route("/mkdir", web::post().to(mkdir))
            .route("/delete", web::post().to(delete_file))
            .route("/download", web::get().to(download_file))
            .route("/upload", web::post().to(upload_file))
    );
}
