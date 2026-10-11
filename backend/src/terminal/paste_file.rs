//! Files pasted into the web terminal from the clipboard.
//!
//! An image on the clipboard cannot travel down a terminal connection — only
//! characters do, which is why a CLI that pastes images reads the clipboard of
//! the machine it runs on and finds nothing (or no X display at all) when that
//! machine is the server. So the browser sends the bytes here, the server keeps
//! them in a per-user folder, and the terminal is given the path instead. That
//! is something a shell, an editor or a CLI can actually open.

use actix_web::{web, HttpRequest, HttpResponse};

use super::handlers::extract_username_header;
use super::session::data_dir;

/// Largest clipboard file accepted, in bytes.
pub(crate) const MAX_BYTES: usize = 16 * 1024 * 1024;
/// Pasted files older than this are swept away when the next one arrives.
const KEEP_SECS: u64 = 7 * 24 * 3600;

/// Clipboard types worth writing to disk, with the extension each one gets.
/// Deliberately short: pasted content lands in a folder shells read from, so
/// nothing executable belongs here.
const ALLOWED: &[(&str, &str)] = &[
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/bmp", "bmp"),
    ("image/tiff", "tif"),
    ("image/svg+xml", "svg"),
    ("image/avif", "avif"),
    ("application/pdf", "pdf"),
];

/// The extension for a clipboard type, or `None` when we do not take it.
pub(crate) fn extension_for(mime: &str) -> Option<&'static str> {
    let mime = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    ALLOWED
        .iter()
        .find(|(m, _)| *m == mime)
        .map(|(_, ext)| *ext)
}

/// A username reduced to something safe to use as a folder name: nothing that
/// could walk out of the paste directory survives.
pub(crate) fn safe_user(username: &str) -> String {
    let mut cleaned: String = username
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .take(32)
        .collect();
    while cleaned.contains("..") {
        cleaned = cleaned.replace("..", ".");
    }
    let cleaned = cleaned.trim_matches('.');
    if cleaned.is_empty() {
        "user".to_string()
    } else {
        cleaned.to_string()
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Drop pasted files nobody came back for, so the folder cannot grow forever.
fn sweep_old(dir: &str) {
    let now = now_secs();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| now.saturating_sub(d.as_secs()) > KEEP_SECS)
            .unwrap_or(false);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

async fn upload_paste(body: web::Bytes, req: HttpRequest) -> HttpResponse {
    let username = extract_username_header(&req);
    if username.is_empty() {
        return HttpResponse::Unauthorized()
            .json(crate::models::ApiResponse::<()>::error("Unauthorized"));
    }
    let mime = req
        .headers()
        .get("X-Paste-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream");
    let Some(ext) = extension_for(mime) else {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(&format!(
            "Clipboard type not supported: {}",
            mime
        )));
    };
    if body.is_empty() {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "Empty clipboard file",
        ));
    }
    if body.len() > MAX_BYTES {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "Clipboard file too large",
        ));
    }

    let dir = format!("{}/pastes/{}", data_dir(), safe_user(&username));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(&format!(
            "mkdir: {}",
            e
        )));
    }
    sweep_old(&dir);

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let tag: String = (0..4)
        .map(|_| {
            const HEX: &[u8] = b"0123456789abcdef";
            HEX[rand::random::<usize>() % 16] as char
        })
        .collect();
    let path = format!("{}/paste-{}-{}.{}", dir, stamp, tag, ext);
    match std::fs::write(&path, &body) {
        Ok(()) => HttpResponse::Ok().json(crate::models::ApiResponse::ok(path)),
        Err(e) => HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(&format!(
            "write: {}",
            e
        ))),
    }
}

pub(crate) fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/api/terminal/paste-file")
            // The default extractor limit is a quarter of a megabyte; a
            // screenshot is bigger than that.
            .app_data(web::PayloadConfig::new(MAX_BYTES))
            .route(web::post().to(upload_paste)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_images_and_pdfs_only() {
        assert_eq!(extension_for("image/png"), Some("png"));
        assert_eq!(extension_for("image/jpeg; charset=binary"), Some("jpg"));
        assert_eq!(extension_for("IMAGE/PNG"), Some("png"));
        assert_eq!(extension_for("application/pdf"), Some("pdf"));
        assert_eq!(extension_for("application/x-sh"), None);
        assert_eq!(extension_for("text/html"), None);
    }

    #[test]
    fn folder_name_cannot_escape_the_paste_directory() {
        assert_eq!(safe_user("admin"), "admin");
        assert_eq!(safe_user("first.last"), "first.last");
        assert_eq!(safe_user("../../etc"), "etc");
        assert_eq!(safe_user(".."), "user");
        assert_eq!(safe_user("a/b"), "ab");
        assert_eq!(safe_user(""), "user");
    }
}
