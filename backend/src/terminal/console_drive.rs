//! The per-VM virtual drive. guacd redirects this host directory into the RDP
//! session as the "NabiMan" drive, so files the browser drags in and files the
//! VM copies onto that drive both land here — listed and downloaded server-side
//! (reliable, unlike reading the redirected filesystem back over RDPDR).

use std::path::{Path, PathBuf};

use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType};
use actix_web::{web, HttpRequest, HttpResponse};

use crate::models::ApiResponse;

/// The host directory redirected into the VM at `host` as its "NabiMan" drive.
/// Keyed by host so each VM has its own; must match the path rdp_tunnel sends.
pub fn drive_dir(host: &str) -> PathBuf {
    let root = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/workspace/var/lib/nabiman".into());
    let safe: String = host.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    Path::new(&root).join("guac-drive").join(safe)
}

fn host_of(target: &str) -> Option<String> {
    super::vnc_registry::rdp_login(target).map(|(host, _, _, _)| host)
}

/// `GET /api/remote/consoles/{target}/drive` — the files on the VM's drive.
#[actix_web::get("/api/remote/consoles/{target}/drive")]
async fn list_drive(req: HttpRequest, target: web::Path<String>) -> HttpResponse {
    if !super::vnc_console::may_open(&req, &target) {
        return HttpResponse::Forbidden().finish();
    }
    let Some(host) = host_of(&target) else {
        return HttpResponse::NotFound().finish();
    };
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(drive_dir(&host)) {
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let modified = meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            files.push(serde_json::json!({
                "name": entry.file_name().to_string_lossy(),
                "size": meta.len(),
                "modified": modified,
            }));
        }
    }
    HttpResponse::Ok().json(ApiResponse::ok(files))
}

/// `GET /api/remote/consoles/{target}/drive/{name}` — download one drive file.
#[actix_web::get("/api/remote/consoles/{target}/drive/{name}")]
async fn get_drive_file(req: HttpRequest, path: web::Path<(String, String)>) -> HttpResponse {
    let (target, name) = path.into_inner();
    if !super::vnc_console::may_open(&req, &target) {
        return HttpResponse::Forbidden().finish();
    }
    // A bare file name only — never a path out of the drive directory.
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return HttpResponse::BadRequest().finish();
    }
    let Some(host) = host_of(&target) else {
        return HttpResponse::NotFound().finish();
    };
    match actix_files::NamedFile::open_async(drive_dir(&host).join(&name)).await {
        Ok(file) => file
            .set_content_disposition(ContentDisposition {
                disposition: DispositionType::Attachment,
                parameters: vec![DispositionParam::Filename(name)],
            })
            .into_response(&req),
        Err(_) => HttpResponse::NotFound().finish(),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_drive).service(get_drive_file);
}
