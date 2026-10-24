#[path = "remote_server_operations.rs"]
mod operations;
use operations::*;
#[path = "remote_server_records.rs"]
mod records;
use records::*;
#[path = "remote_protocols.rs"]
pub(crate) mod protocols;
#[path = "remote_check.rs"]
pub(crate) mod check;
#[path = "remote_servers_cache.rs"]
mod cache;

use crate::models::{
    AddRemoteServerRequest, ApiResponse, RemoteExecRequest, RemoteServer, RemoteServerStatus,
    UpdateRemoteServerRequest,
};
use crate::remote_probe::{ssh_command, ssh_probe};
use crate::secret_store;
use crate::ssh_keys;
use actix_web::{web, HttpResponse};
use std::fs;
use std::process::Command;

pub(crate) static SERVER_WRITES: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn data_file_path() -> String {
    nabiman_server::data_dir().join("remote_servers.json").to_string_lossy().into_owned()
}

pub fn load_servers() -> Result<Vec<RemoteServer>, String> {
    cache::cached(std::path::Path::new(&data_file_path()), load_servers_at)
}

fn load_servers_at(path: &std::path::Path) -> Result<Vec<RemoteServer>, String> {
    let mut servers: Vec<RemoteServer> = match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map_err(|e| format!("Invalid remote server database: {e}"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(format!("Cannot read remote server database: {e}")),
    };
    for s in servers.iter_mut() {
        s.has_password = !s.password_enc.is_empty();
    }
    Ok(servers)
}

fn storage_error(error: String) -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error))
}

/// Strip the encrypted password before a server record goes out over the API.
pub(crate) fn public(mut server: RemoteServer) -> RemoteServer {
    server.has_password = !server.password_enc.is_empty();
    server.password_enc = String::new();
    server
}

/// Look up one saved server (used by the terminal WebSocket to resolve
/// `server_id` into host/user/port and, when stored, a password).
pub fn find_server(id: &str) -> Result<Option<RemoteServer>, String> {
    Ok(load_servers()?.into_iter().find(|s| s.id == id))
}

/// Decrypt the stored password of a server, if it has one.
pub fn server_password(srv: &RemoteServer) -> Option<String> {
    if srv.password_enc.is_empty() {
        return None;
    }
    match secret_store::decrypt(&srv.password_enc) {
        Ok(pw) => Some(pw),
        Err(e) => {
            eprintln!(
                "remote_servers: cannot decrypt password for {}: {}",
                srv.id, e
            );
            None
        }
    }
}

/// Call while holding SERVER_WRITES to serialize read-modify-write operations.
pub(crate) fn save_servers(servers: &mut [RemoteServer]) -> Result<(), String> {
    for s in servers.iter_mut() {
        s.has_password = !s.password_enc.is_empty();
    }
    let json =
        serde_json::to_string_pretty(servers).map_err(|e| format!("Serialize error: {}", e))?;
    let written = crate::private_storage::write(std::path::Path::new(&data_file_path()), json.as_bytes())
        .map_err(|e| format!("Write error: {}", e));
    cache::forget();
    written
}

fn generate_id() -> String {
    format!("srv_{}", uuid::Uuid::new_v4().simple())
}

pub(crate) fn now_iso() -> String {
    let output = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "unknown".into(),
    }
}

fn validate_host(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 256
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == ':')
}

// --- Handlers ---

async fn list_servers(
    req: actix_web::HttpRequest,
    store: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let actor = match crate::membership::actor(&req, &store) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let servers = match load_servers() {
        Ok(servers) => servers
            .into_iter()
            .filter(|s| {
                crate::member_servers::listed(&actor, s)
                    && (actor.super_admin()
                        || !actor.restricted()
                        || crate::member_servers::is_member(&actor)
                        || actor
                            .scope_role(&crate::models::ScopeKind::Server, &s.id)
                            .is_some())
            })
            .map(|s| {
                // The administrators' notes on a server they assigned stay theirs.
                let assigned = crate::member_servers::assigned(&actor, &s);
                let mut s = public(s);
                if crate::member_servers::is_member(&actor) && assigned {
                    s.memo.clear();
                }
                s
            })
            .collect::<Vec<_>>(),
        Err(error) => return storage_error(error),
    };
    HttpResponse::Ok().json(ApiResponse::ok(servers))
}

/// NabiMan's SSH CA public key, for installing on servers that will trust
/// certificate authentication. Public by nature; no secret leaves here.
async fn ssh_ca() -> HttpResponse {
    match crate::ssh_cert::ca_public_key() {
        Ok(key) => HttpResponse::Ok().json(ApiResponse::ok(key)),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/remote-servers")
            .route("", web::get().to(list_servers))
            .route("", web::post().to(add_server))
            .route("/check-all", web::post().to(check_all_servers))
            .route("/ssh-key", web::get().to(ssh_keys::get_ssh_key_info))
            .route("/ssh-ca", web::get().to(ssh_ca))
            .route(
                "/ssh-key/generate",
                web::post().to(ssh_keys::generate_ssh_key),
            )
            .route("/{id}", web::put().to(update_server))
            .route("/{id}", web::delete().to(delete_server))
            .route(
                "/{id}/host-trust",
                web::put().to(crate::remote_host_trust::update),
            )
            .route("/{id}/host-key", web::post().to(crate::remote_host_trust::scan))
            .route("/{id}/check", web::post().to(check_server))
            .route("/{id}/detect", web::post().to(check::detect))
            .route("/{id}/exec", web::post().to(exec_on_server))
            .route("/{id}/deploy-key", web::post().to(ssh_keys::deploy_key))
            .route("/{id}/test-key", web::post().to(ssh_keys::test_key_handler)),
    );
}

#[cfg(test)]
#[path = "remote_servers_tests.rs"]
mod tests;
