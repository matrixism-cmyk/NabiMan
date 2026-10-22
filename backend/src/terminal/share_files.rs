//! Share links to a saved server's files alone. A server reached only over
//! FTP, FTPS or SFTP has no terminal session to share, so its link names the
//! server instead, together with exactly how it was reached when the link was
//! made. Anything about that changing — address, user, protocol, password,
//! approved host key or certificate — ends the link, as revoking it would.

use super::management::{publish, CreateShareRequest, LinkTarget};
use super::*;
use crate::models::RemoteServer;
use sha2::{Digest, Sha256};

/// How a server's files are reached, as one value a link can be held to.
pub(crate) fn files_binding(server: &RemoteServer) -> String {
    let ports = format!("{}/{}/{}", server.port, server.ftp_port, server.ftps_implicit);
    let parts = [
        &server.id, &server.host, &ports, &server.user, &server.auth_method, &server.files,
        &server.tls_fingerprint, &server.host_trust_version, &server.password_enc,
    ];
    hex::encode(Sha256::digest(parts.map(|p| p.as_str()).join("\0")))
}

/// The server a files link opens, while it is still reached as it was.
pub(crate) fn files_link_server(link: &ShareLink) -> Option<RemoteServer> {
    if !link.files_only() || link.files == ShareFiles::None {
        return None;
    }
    let server = crate::remote_servers::find_server(&link.server_id).ok()??;
    (super::super::sftp::files_allowed(&server).is_ok() && files_binding(&server) == link.server_binding)
        .then_some(server)
}

pub(super) fn files_link_alive(link: &ShareLink) -> bool {
    files_link_server(link).is_some()
}

pub(super) async fn create_files_share(
    path: web::Path<String>,
    body: web::Json<CreateShareRequest>,
    shares: web::Data<ShareStore>,
    users: web::Data<UserStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    req: HttpRequest,
) -> HttpResponse {
    let Some(owner) = current_owner(&req, &users) else {
        return HttpResponse::Forbidden().finish();
    };
    if let Some(refused) = super::management::not_granted(&owner, false, true) {
        return refused;
    }
    if body.files == ShareFiles::None {
        return no_store(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "A files link needs read or write access",
        )));
    }
    let server = match crate::remote_servers::find_server(&path) {
        Ok(Some(server)) if crate::member_servers::usable(&owner, &server) => server,
        Ok(_) => return no_store(HttpResponse::Ok().json(ApiResponse::<()>::error("Saved server not found"))),
        Err(_) => return storage_error(),
    };
    if let Err(e) = super::super::sftp::files_allowed(&server) {
        return no_store(HttpResponse::Ok().json(ApiResponse::<()>::error(&e)));
    }
    let port = if server.uses_ftp() { server.ftp_port } else { server.port };
    let target = LinkTarget {
        label: format!("{} · {}:{}@{}:{}", server.name, server.files, server.user, server.host, port),
        server_binding: files_binding(&server),
        server_id: server.id,
        ..LinkTarget::default()
    };
    // There is no terminal: "read only" is simply what the file access says.
    let request = CreateShareRequest {
        expires_in_secs: body.expires_in_secs,
        permanent: body.permanent,
        password: body.password.clone(),
        read_only: body.files == ShareFiles::Read,
        files: body.files,
    };
    publish(&req, &request, owner, target, &users, &sessions, &shares)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> RemoteServer {
        serde_json::from_value(serde_json::json!({
            "id": "srv", "name": "files", "host": "h", "port": 22, "user": "web", "auth_method": "password",
            "tags": [], "memo": "", "created_at": "", "terminal": "none", "files": "ftps", "ftp_port": 990,
        }))
        .unwrap()
    }

    #[test]
    fn a_files_link_is_held_to_how_the_server_is_reached() {
        let base = server();
        let pinned = files_binding(&base);
        let mut renamed = base.clone();
        renamed.name = "renamed".into();
        renamed.memo = "memo".into();
        assert_eq!(files_binding(&renamed), pinned);
        let changes: [fn(&mut RemoteServer); 7] = [
            |s| s.host = "other".into(),
            |s| s.user = "admin".into(),
            |s| s.files = "ftp".into(),
            |s| s.ftp_port = 21,
            |s| s.ftps_implicit = true,
            |s| s.password_enc = "new".into(),
            |s| s.tls_fingerprint = "SHA256:00".into(),
        ];
        for change in changes {
            let mut s = base.clone();
            change(&mut s);
            assert_ne!(files_binding(&s), pinned);
        }
    }
}
