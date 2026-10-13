//! Files next to a terminal: browse, upload, download, make folders, move,
//! rename and delete — on a saved server over SFTP, or on this machine for
//! the local shell.
//!
//! It lives under `/api/terminal/` on purpose. Reading and writing a machine's
//! files is the same power as a shell on it, so it is granted to exactly the
//! accounts the terminal is, and a saved server needs the same host-key
//! approval. The local panel works as the NabiMan process, as the local shell
//! does.

mod fs;
mod handlers;
pub(crate) mod links;
mod paths;
mod pool;

use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};

use crate::models::{ApiResponse, RemoteServer};
use fs::{Fs, FsError};

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
    user_id: String,
    scope: String,
    server: Option<RemoteServer>,
}

/// Refused the way a terminal would be.
fn target(req: &HttpRequest, scope: &str) -> Result<Target, HttpResponse> {
    let user_id = crate::auth::verified_user_id(req)
        .ok_or_else(|| fail(StatusCode::UNAUTHORIZED, "Sign in again"))?;
    if scope == LOCAL {
        return Ok(Target { user_id, scope: scope.into(), server: None });
    }
    let server = match crate::remote_servers::find_server(scope) {
        Ok(Some(server)) => server,
        Ok(None) => return Err(fail(StatusCode::NOT_FOUND, "Saved server not found")),
        Err(e) => return Err(fail(StatusCode::SERVICE_UNAVAILABLE, &e)),
    };
    super::remote_access::RemoteBinding::from_server(&server).map_err(|e| bad(&e))?;
    Ok(Target { user_id, scope: scope.into(), server: Some(server) })
}

impl Target {
    async fn fs(&self) -> Result<Fs, HttpResponse> {
        match &self.server {
            None => Ok(Fs::Local),
            Some(server) => pool::session(&self.user_id, server)
                .await
                .map(Fs::Remote)
                .map_err(|e| fail(StatusCode::BAD_GATEWAY, &e)),
        }
    }
    /// A failure as an answer. A broken connection is dropped, so the next
    /// request starts a fresh one instead of failing the same way.
    fn error(&self, e: FsError) -> HttpResponse {
        if e.broken {
            if let Some(server) = &self.server {
                pool::forget(&self.user_id, server);
            }
        }
        fail(e.status, &e.message)
    }
}

pub(crate) fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/terminal/sftp/{id}")
            .route("/list", web::get().to(handlers::list))
            .route("/download", web::get().to(handlers::download))
            .route("/link", web::post().to(handlers::link))
            .route("/upload", web::post().to(handlers::upload))
            .route("/mkdir", web::post().to(handlers::mkdir))
            .route("/rename", web::post().to(handlers::rename))
            .route("/delete", web::post().to(handlers::delete)),
    );
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
