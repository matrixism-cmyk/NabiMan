//! Files next to a terminal: browse, upload, download, make folders, move,
//! rename and delete — on a saved server over SFTP or FTP/FTPS, or on this
//! machine for the local shell.
//!
//! It lives under `/api/terminal/` on purpose. Reading and writing a machine's
//! files is the same power as a shell on it, so it is granted to exactly the
//! accounts the terminal is, and a saved server needs the same host-key
//! approval. The local panel works as the NabiMan process, as the local shell
//! does.

mod folder;
mod fs;
pub(crate) mod ftp;
mod handlers;
pub(crate) mod links;
mod parts;
mod paths;
mod pool;
pub(crate) mod share_keys;
mod zip;

use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};

use crate::models::{ApiResponse, RemoteServer};
use fs::{Fs, FsError};
pub(crate) use pool::check as sftp_check;
use sha2::Digest;

/// The scope that means "this machine" rather than a saved server.
const LOCAL: &str = "local";

fn fail(status: StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status).json(ApiResponse::<()>::error(message))
}
fn bad(message: &str) -> HttpResponse {
    fail(StatusCode::BAD_REQUEST, message)
}

/// Who is asking, and where: a saved server or this machine.
struct Target {
    /// Whose connection pool entry to use.
    pool_user: String,
    /// A saved server id, `local`, or for a visitor the share token.
    scope: String,
    server: Option<RemoteServer>,
    /// The route prefix downloads are fetched from.
    base: String,
    /// The share token, when the request comes from a share link's visitor.
    share: Option<String>,
}

/// The target of a file request, refused the way a terminal would be. Account
/// routes carry `{id}`, share routes `{token}`; the operations that change
/// files need a link that allows writing.
fn resolve(req: &HttpRequest) -> Result<Target, HttpResponse> {
    let (target, writable) = match req.match_info().get("token") {
        Some(token) => share_target(req, token)?,
        None => (account_target(req, req.match_info().get("id").unwrap_or(""))?, true),
    };
    let op = req.path().rsplit('/').next().unwrap_or("");
    let writes = ["upload", "upload-part", "upload-finish", "upload-abort", "mkdir", "rename", "delete"];
    if writes.contains(&op) && !writable {
        return Err(fail(StatusCode::FORBIDDEN, "This link allows browsing and downloading only"));
    }
    Ok(target)
}

fn account_target(req: &HttpRequest, scope: &str) -> Result<Target, HttpResponse> {
    let user_id = crate::auth::verified_user_id(req)
        .ok_or_else(|| fail(StatusCode::UNAUTHORIZED, "Sign in again"))?;
    let base = format!("/api/terminal/sftp/{scope}");
    if scope == LOCAL {
        return Ok(Target { pool_user: user_id, scope: scope.into(), server: None, base, share: None });
    }
    let server = match crate::remote_servers::find_server(scope) {
        Ok(Some(server)) => server,
        Ok(None) => return Err(fail(StatusCode::NOT_FOUND, "Saved server not found")),
        Err(e) => return Err(fail(StatusCode::SERVICE_UNAVAILABLE, &e)),
    };
    files_allowed(&server).map_err(|e| bad(&e))?;
    Ok(Target { pool_user: user_id, scope: scope.into(), server: Some(server), base, share: None })
}

/// A share link's visitor: the link must be live and grant files, the key (or,
/// for one download, a drag-out ticket) must be the link's own, and the shared
/// session must still be running. The files are those of the session's target.
fn share_target(req: &HttpRequest, token: &str) -> Result<(Target, bool), HttpResponse> {
    use super::share::{live_link, ShareFiles};
    let ended = || fail(StatusCode::UNAUTHORIZED, "File access for this link has ended; reconnect");
    let unavailable = || fail(StatusCode::SERVICE_UNAVAILABLE, "Share registry unavailable");
    let shares = req.app_data::<web::Data<super::share_store::ShareStore>>().ok_or_else(unavailable)?;
    let users = req.app_data::<web::Data<crate::users::UserStore>>().ok_or_else(unavailable)?;
    let sessions = req.app_data::<web::Data<super::session::PtyStore>>().ok_or_else(unavailable)?;
    let link = live_link(shares, users, token).ok_or_else(ended)?;
    if link.files == ShareFiles::None {
        return Err(ended());
    }
    let admitted = match req.headers().get("X-Share-Files").and_then(|v| v.to_str().ok()) {
        Some(key) => share_keys::check(key, token).is_some_and(|snapshot| link.same_authorization(&snapshot)),
        None => {
            let q = web::Query::<std::collections::HashMap<String, String>>::from_query(req.query_string())
                .map(|q| q.into_inner())
                .unwrap_or_default();
            (req.path().ends_with("/download") || req.path().ends_with("/download-folder"))
                && matches!((q.get("path"), q.get("ticket")), (Some(p), Some(t)) if links::redeem_share(token, p, t))
        }
    };
    if !admitted {
        return Err(ended());
    }
    let session = sessions
        .lock()
        .map_err(|_| unavailable())?
        .get(&link.session_id)
        .filter(|s| s.owned_by(&link.owner_id) && super::remote_access::allowed(s))
        .cloned()
        .ok_or_else(ended)?;
    if !super::session::tmux_session_exists(&session.tmux_name) {
        return Err(fail(StatusCode::GONE, "The shared session has ended"));
    }
    let server = match &session.remote_binding {
        None => None,
        Some(binding) => match crate::remote_servers::find_server(binding.server_id()) {
            Ok(Some(server)) if files_allowed(&server).is_ok() => Some(server),
            _ => return Err(ended()),
        },
    };
    let pool_user = format!("share:{}", &hex::encode(sha2::Sha256::digest(token.as_bytes()))[..16]);
    let target = Target {
        pool_user,
        scope: token.into(),
        server,
        base: format!("/api/share/{token}/files"),
        share: Some(token.into()),
    };
    Ok((target, link.files == ShareFiles::Write))
}

/// Whether a saved server's files can be opened at all: it has a way to
/// reach them, and SFTP's host key has been approved.
pub(crate) fn files_allowed(server: &RemoteServer) -> Result<(), String> {
    match server.files.as_str() {
        "sftp" => super::remote_access::ssh_approved(server),
        "ftp" | "ftps" => Ok(()),
        _ => Err("This server has no file access set up".into()),
    }
}

impl Target {
    async fn fs(&self) -> Result<Fs, HttpResponse> {
        let unreachable = |e: String| fail(StatusCode::BAD_GATEWAY, &e);
        match &self.server {
            None => Ok(Fs::Local),
            Some(server) if server.uses_ftp() => {
                ftp::session(&self.pool_user, server).await.map(Fs::Ftp).map_err(unreachable)
            }
            Some(server) => pool::session(&self.pool_user, server).await.map(Fs::Remote).map_err(unreachable),
        }
    }
    /// A failure as an answer. A broken connection is dropped, so the next
    /// request starts a fresh one instead of failing the same way.
    fn error(&self, e: FsError) -> HttpResponse {
        if e.broken {
            if let Some(server) = &self.server {
                pool::forget(&self.pool_user, server);
                ftp::forget(&self.pool_user, server);
            }
        }
        fail(e.status, &e.message)
    }
}

fn routes(prefix: &str) -> actix_web::Scope {
    web::scope(prefix)
        .route("/list", web::get().to(handlers::list))
        .route("/download", web::get().to(handlers::download))
        .route("/download-folder", web::get().to(folder::download))
        .route("/link", web::post().to(handlers::link))
        .route("/upload", web::post().to(handlers::upload))
        .route("/upload-part", web::post().to(parts::chunk))
        .route("/upload-finish", web::post().to(parts::finish))
        .route("/upload-abort", web::post().to(parts::abort))
        .route("/mkdir", web::post().to(handlers::mkdir))
        .route("/rename", web::post().to(handlers::rename))
        .route("/delete", web::post().to(handlers::delete))
}

pub(crate) fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(routes("/api/terminal/sftp/{id}"))
        // Public: a share link's visitor, checked in `share_target`.
        .service(routes("/api/share/{token}/files"));
}

#[cfg(test)]
mod tests {
    use super::pool::failure_message;

    #[test]
    fn failures_name_the_cause() {
        let said = "Warning: Permanently added 'x' (ED25519) to the list of known hosts.\r\nuser@host: Permission denied (publickey,password).\n";
        assert_eq!(failure_message(said, false), "SFTP connection failed: user@host: Permission denied (publickey,password).");
        assert_eq!(failure_message("", true), "SFTP connection timed out");
        assert_eq!(failure_message("", false), "SFTP is not available on this server");
    }
}
