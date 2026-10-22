//! Uploads that arrive in pieces.
//!
//! A file of several gigabytes in one request is fragile: one dropped
//! connection loses all of it, and a proxy that buffers request bodies holds
//! the whole file on its own disk before passing any of it on, while the
//! browser already shows 100%. So large files come in pieces of a few dozen
//! megabytes, written into a hidden part file next to the target; a piece
//! that fails is sent again from where the part file ends; and only when the
//! last piece is in does the part file take the real name. An upload that is
//! cancelled removes its part file.

use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use super::paths::{join, valid_name, valid_path};
use super::{bad, fail, resolve};
use crate::models::ApiResponse;

macro_rules! tryr {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(r) => return r,
        }
    };
}

#[derive(Deserialize)]
pub(super) struct PartQuery {
    dir: String,
    name: String,
    /// Chosen by the browser for this upload: 16 to 64 hex digits.
    id: String,
    #[serde(default)]
    offset: u64,
    /// For finishing: the size the whole file must have.
    #[serde(default)]
    size: u64,
    #[serde(default)]
    overwrite: bool,
}

/// The hidden file an upload grows in, beside where it will end up.
pub(super) fn part_name(name: &str, id: &str) -> Option<String> {
    let id_ok = (16..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit());
    if !id_ok || !valid_name(name) {
        return None;
    }
    let mut stem = name;
    while stem.len() > 180 {
        let cut = stem.char_indices().last().map(|(i, _)| i).unwrap_or(0);
        stem = &stem[..cut];
    }
    let part = format!(".{stem}.nbup-{}", id.to_ascii_lowercase());
    valid_name(&part).then_some(part)
}

fn paths(q: &PartQuery) -> Result<(String, String), HttpResponse> {
    if !valid_path(&q.dir) {
        return Err(bad("Invalid path"));
    }
    let part = part_name(&q.name, &q.id).ok_or_else(|| bad("Invalid file name"))?;
    Ok((join(&q.dir, &part), join(&q.dir, &q.name)))
}

fn conflict(message: &str, data: serde_json::Value) -> HttpResponse {
    HttpResponse::Conflict().json(serde_json::json!({ "success": false, "data": data, "message": message }))
}

/// One piece, written at `offset`. The first piece starts the part file (and
/// is where an existing file is noticed); later ones must continue exactly
/// where it ends, and are told where that is when they do not.
pub(super) async fn chunk(req: HttpRequest, q: web::Query<PartQuery>, mut body: web::Payload) -> HttpResponse {
    let t = tryr!(resolve(&req));
    let limit = crate::member_servers::upload_limit(&req);
    if limit.is_some_and(|max| q.offset > max) {
        return super::too_big(limit);
    }
    let (part, target) = tryr!(paths(&q));
    let fs = tryr!(t.fs().await);
    let mut file = if q.offset == 0 {
        if !q.overwrite && fs.exists(&target).await {
            return conflict("A file with that name already exists", serde_json::json!({ "exists": true }));
        }
        tryr!(fs.create(&part).await.map_err(|e| t.error(e)))
    } else {
        let have = fs.stat(&part).await.map(|m| m.size).unwrap_or(0);
        if have != q.offset {
            return conflict("The upload continues from a different point", serde_json::json!({ "size": have }));
        }
        tryr!(fs.open_at(&part, q.offset).await.map_err(|e| t.error(e)))
    };
    let mut written: u64 = 0;
    while let Some(piece) = body.next().await {
        let Ok(piece) = piece else {
            return fail(StatusCode::BAD_GATEWAY, "The upload was interrupted");
        };
        if limit.is_some_and(|max| q.offset + written + piece.len() as u64 > max) {
            drop(file);
            let _ = fs.remove_file(&part).await;
            return super::too_big(limit);
        }
        if let Err(e) = file.write_all(&piece).await {
            return fail(StatusCode::BAD_GATEWAY, &format!("Write failed: {e}"));
        }
        written += piece.len() as u64;
    }
    if let Err(e) = file.shutdown().await {
        return fail(StatusCode::BAD_GATEWAY, &format!("Write failed: {e}"));
    }
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({ "size": q.offset + written })))
}

/// The last piece is in: the part file takes the real name, once it is
/// exactly as large as the file the browser sent.
pub(super) async fn finish(req: HttpRequest, q: web::Query<PartQuery>) -> HttpResponse {
    let t = tryr!(resolve(&req));
    if crate::member_servers::upload_limit(&req).is_some_and(|max| q.size > max) {
        return super::too_big(crate::member_servers::upload_limit(&req));
    }
    let (part, target) = tryr!(paths(&q));
    let fs = tryr!(t.fs().await);
    let have = tryr!(fs.stat(&part).await.map_err(|e| t.error(e))).size;
    if have != q.size {
        return conflict("The upload is incomplete", serde_json::json!({ "size": have }));
    }
    if let Ok(existing) = fs.lstat(&target).await {
        if !q.overwrite {
            return conflict("A file with that name already exists", serde_json::json!({ "exists": true }));
        }
        if existing.is_dir() {
            return conflict("A folder has that name", serde_json::json!({}));
        }
        // SFTP's rename does not replace, so the old file goes first.
        if let Err(e) = fs.remove_file(&target).await {
            return t.error(e);
        }
    }
    match fs.rename(&part, &target).await {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({ "path": target, "size": have }))),
        Err(e) => t.error(e),
    }
}

/// A cancelled or failed upload leaves nothing behind.
pub(super) async fn abort(req: HttpRequest, q: web::Query<PartQuery>) -> HttpResponse {
    let t = tryr!(resolve(&req));
    let (part, _) = tryr!(paths(&q));
    let fs = tryr!(t.fs().await);
    let _ = fs.remove_file(&part).await;
    HttpResponse::Ok().json(ApiResponse::ok(true))
}

#[cfg(test)]
mod tests {
    use super::part_name;

    #[test]
    fn part_files_are_hidden_siblings() {
        let id = "0123456789abcdef";
        assert_eq!(part_name("big.iso", id).as_deref(), Some(".big.iso.nbup-0123456789abcdef"));
        // A long name is shortened on a character boundary, never split.
        // 240 bytes: a valid name that would pass 255 with the part suffix.
        let long = "가".repeat(80);
        let part = part_name(&long, id).unwrap();
        assert!(part.len() <= 255 && part.ends_with(".nbup-0123456789abcdef"));
    }

    #[test]
    fn ids_and_names_cannot_escape() {
        assert!(part_name("x", "short").is_none());
        assert!(part_name("x", "../../0123456789abcdef").is_none());
        assert!(part_name("../x", "0123456789abcdef").is_none());
        assert!(part_name("a/b", "0123456789abcdef").is_none());
    }
}
