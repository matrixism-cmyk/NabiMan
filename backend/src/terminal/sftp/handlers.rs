//! The requests a file panel makes.

use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use futures::StreamExt;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::paths::{self, entry, join, parent, sort, valid_name, valid_path, Entry};
use super::{bad, fail, links, resolve};
use crate::models::ApiResponse;

/// A folder listing stops here; the rest is reported as left out.
const MAX_ENTRIES: usize = 5000;
/// A recursive delete gives up rather than walk an enormous tree.
const MAX_DELETE: usize = 20_000;
const CHUNK: usize = 256 * 1024;

macro_rules! tryr {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(r) => return r,
        }
    };
}

#[derive(Deserialize)]
pub(super) struct PathQuery {
    #[serde(default)]
    path: String,
}

pub(super) async fn list(req: HttpRequest, q: web::Query<PathQuery>) -> HttpResponse {
    let t = tryr!(resolve(&req));
    let asked = if q.path.is_empty() { "." } else { q.path.as_str() };
    if !valid_path(asked) {
        return bad("Invalid path");
    }
    let fs = tryr!(t.fs().await);
    let path = tryr!(fs.canonicalize(asked).await.map_err(|e| t.error(e)));
    let items = tryr!(fs.read_dir(&path).await.map_err(|e| t.error(e)));
    let truncated = items.len() > MAX_ENTRIES;
    let mut entries: Vec<Entry> = Vec::new();
    for (name, meta) in items.into_iter().take(MAX_ENTRIES) {
        let mut row = entry(name, &meta);
        if row.kind == "link" {
            // Following a link costs a round trip, so only links are followed.
            row.to_dir = fs.stat(&join(&path, &row.name)).await.is_ok_and(|m| m.is_dir());
        }
        entries.push(row);
    }
    sort(&mut entries);
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "path": path, "parent": parent(&path), "entries": entries, "truncated": truncated,
    })))
}

pub(super) async fn download(req: HttpRequest, q: web::Query<PathQuery>) -> HttpResponse {
    let t = tryr!(resolve(&req));
    if !valid_path(&q.path) {
        return bad("Invalid path");
    }
    let fs = tryr!(t.fs().await);
    let meta = tryr!(fs.stat(&q.path).await.map_err(|e| t.error(e)));
    if meta.is_dir() {
        return bad("Folders cannot be downloaded; open it and download the files");
    }
    let file = tryr!(fs.open(&q.path).await.map_err(|e| t.error(e)));
    // The connection travels with the stream so it stays open until the end.
    let state = Some((file, fs, vec![0u8; CHUNK]));
    let body = futures::stream::unfold(state, |state| async move {
        let (mut file, fs, mut buf) = state?;
        match file.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => Some((Ok(web::Bytes::copy_from_slice(&buf[..n])), Some((file, fs, buf)))),
            Err(e) => Some((Err(actix_web::error::ErrorBadGateway(e)), None)),
        }
    });
    let mut response = HttpResponse::Ok();
    response
        .content_type("application/octet-stream")
        .insert_header(("Content-Disposition", paths::disposition(paths::base_name(&q.path))))
        .insert_header(("Cache-Control", "no-store"))
        .no_chunking(meta.size);
    response.streaming(body)
}

#[derive(Deserialize)]
pub(super) struct PathBody {
    path: String,
}

/// A single-use URL for `path`, for a drag onto the desktop.
pub(super) async fn link(req: HttpRequest, b: web::Json<PathBody>) -> HttpResponse {
    let t = tryr!(resolve(&req));
    if !valid_path(&b.path) {
        return bad("Invalid path");
    }
    let owner = match &t.share {
        Some(token) => links::Owner::Share(token.clone()),
        None => match crate::auth::verified_claims(&req) {
            Some(claims) => links::Owner::Session(claims),
            None => return fail(StatusCode::UNAUTHORIZED, "Sign in again"),
        },
    };
    // A folder is fetched as a ZIP archive of everything in it.
    let fs = tryr!(t.fs().await);
    let folder = fs.stat(&b.path).await.is_ok_and(|m| m.is_dir());
    let route = if folder { "download-folder" } else { "download" };
    match links::issue(owner, &t.scope, &b.path) {
        Ok(ticket) => {
            let url = format!("{}/{route}?path={}&ticket={ticket}", t.base, paths::query_escape(&b.path));
            HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
                "url": url, "folder": folder, "expires_in_seconds": links::ttl_secs(),
            })))
        }
        Err(e) => fail(StatusCode::SERVICE_UNAVAILABLE, &e),
    }
}

#[derive(Deserialize)]
pub(super) struct UploadQuery {
    dir: String,
    name: String,
    #[serde(default)]
    overwrite: bool,
}

pub(super) async fn upload(
    req: HttpRequest,
    q: web::Query<UploadQuery>,
    mut body: web::Payload,
) -> HttpResponse {
    let t = tryr!(resolve(&req));
    if !valid_path(&q.dir) || !valid_name(&q.name) {
        return bad("Invalid file name");
    }
    let path = join(&q.dir, &q.name);
    let fs = tryr!(t.fs().await);
    let existed = fs.exists(&path).await;
    if existed && !q.overwrite {
        return HttpResponse::Conflict().json(serde_json::json!({
            "success": false, "data": { "exists": true }, "message": "A file with that name already exists",
        }));
    }
    let mut file = tryr!(fs.create(&path).await.map_err(|e| t.error(e)));
    let mut written: u64 = 0;
    let mut problem: Option<String> = None;
    while let Some(chunk) = body.next().await {
        let Ok(chunk) = chunk else {
            problem = Some("The upload was interrupted".into());
            break;
        };
        if let Err(e) = file.write_all(&chunk).await {
            problem = Some(format!("Write failed: {e}"));
            break;
        }
        written += chunk.len() as u64;
    }
    if problem.is_none() {
        if let Err(e) = file.shutdown().await {
            problem = Some(format!("Write failed: {e}"));
        }
    }
    drop(file);
    if let Some(message) = problem {
        // A half-written new file is worse than none; an overwritten one is
        // already gone, so it is left for the operator to see.
        if !existed {
            let _ = fs.remove_file(&path).await;
        }
        return fail(StatusCode::BAD_GATEWAY, &message);
    }
    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({ "path": path, "size": written })))
}

pub(super) async fn mkdir(req: HttpRequest, b: web::Json<PathBody>) -> HttpResponse {
    let t = tryr!(resolve(&req));
    if !valid_path(&b.path) {
        return bad("Invalid path");
    }
    let fs = tryr!(t.fs().await);
    match fs.mkdir(&b.path).await {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::ok(b.path.clone())),
        Err(e) => t.error(e),
    }
}

#[derive(Deserialize)]
pub(super) struct MoveBody {
    from: String,
    to: String,
}

/// Renames or moves. Never replaces something already at the destination.
pub(super) async fn rename(req: HttpRequest, b: web::Json<MoveBody>) -> HttpResponse {
    let t = tryr!(resolve(&req));
    if !valid_path(&b.from) || !valid_path(&b.to) {
        return bad("Invalid path");
    }
    if b.to == b.from || b.to.starts_with(&format!("{}/", b.from.trim_end_matches('/'))) {
        return bad("A folder cannot be moved into itself");
    }
    let fs = tryr!(t.fs().await);
    if fs.exists(&b.to).await {
        return fail(StatusCode::CONFLICT, "Something with that name is already there");
    }
    match fs.rename(&b.from, &b.to).await {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::ok(b.to.clone())),
        Err(e) => t.error(e),
    }
}

/// Deletes a file, a link (not what it points to) or a folder with everything
/// in it.
pub(super) async fn delete(req: HttpRequest, b: web::Json<PathBody>) -> HttpResponse {
    let t = tryr!(resolve(&req));
    if !valid_path(&b.path) || parent(&b.path).is_none() {
        return bad("Invalid path");
    }
    let fs = tryr!(t.fs().await);
    let meta = tryr!(fs.lstat(&b.path).await.map_err(|e| t.error(e)));
    if !meta.is_dir() {
        return match fs.remove_file(&b.path).await {
            Ok(()) => HttpResponse::Ok().json(ApiResponse::ok(1)),
            Err(e) => t.error(e),
        };
    }
    // Walk the tree first, then remove files and the emptied folders deepest
    // first. Links inside are removed as links, never followed.
    let mut folders = vec![b.path.clone()];
    let mut files = Vec::new();
    let mut at = 0;
    while at < folders.len() {
        let items = tryr!(fs.read_dir(&folders[at]).await.map_err(|e| t.error(e)));
        for (name, meta) in items {
            let path = join(&folders[at], &name);
            if meta.is_dir() { folders.push(path) } else { files.push(path) }
        }
        if folders.len() + files.len() > MAX_DELETE {
            return bad("Too many files to delete from here; use the terminal");
        }
        at += 1;
    }
    for path in &files {
        if let Err(e) = fs.remove_file(path).await {
            return t.error(e);
        }
    }
    for path in folders.iter().rev() {
        if let Err(e) = fs.remove_dir(path).await {
            return t.error(e);
        }
    }
    HttpResponse::Ok().json(ApiResponse::ok(files.len() + folders.len()))
}
