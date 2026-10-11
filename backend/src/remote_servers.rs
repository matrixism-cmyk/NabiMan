#[path = "remote_server_operations.rs"]
mod operations;
use operations::*;

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

const DATA_FILE: &str = "/var/lib/nabiman/remote_servers.json";
pub(crate) static SERVER_WRITES: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn data_file_path() -> String {
    std::env::var("NABIMAN_DATA_DIR")
        .map(|d| format!("{}/remote_servers.json", d))
        .unwrap_or_else(|_| DATA_FILE.to_string())
}

pub fn load_servers() -> Result<Vec<RemoteServer>, String> {
    load_servers_at(std::path::Path::new(&data_file_path()))
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
    crate::private_storage::write(std::path::Path::new(&data_file_path()), json.as_bytes())
        .map_err(|e| format!("Write error: {}", e))
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

fn validate_user(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 64
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
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
                actor.super_admin()
                    || !actor.restricted()
                    || actor
                        .scope_role(&crate::models::ScopeKind::Server, &s.id)
                        .is_some()
            })
            .map(public)
            .collect::<Vec<_>>(),
        Err(error) => return storage_error(error),
    };
    HttpResponse::Ok().json(ApiResponse::ok(servers))
}

async fn add_server(body: web::Json<AddRemoteServerRequest>) -> HttpResponse {
    if !validate_host(&body.host) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid hostname"));
    }
    let user = body.user.as_deref().unwrap_or("root");
    if !validate_user(user) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid username"));
    }
    let port = body.port.unwrap_or(22);
    if port == 0 {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid port"));
    }

    // A pasted password can carry a stray newline; it is never part of the
    // secret and would be rejected by the server.
    let password = body
        .password
        .as_deref()
        .unwrap_or("")
        .trim_matches(['\r', '\n']);
    if password.len() > 512 {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Password too long"));
    }
    let password_enc = if password.is_empty() {
        String::new()
    } else {
        match secret_store::encrypt(password) {
            Ok(enc) => enc,
            Err(e) => return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(&e)),
        }
    };
    // A saved password implies password auth unless the caller says otherwise.
    let auth_method = body.auth_method.clone().unwrap_or_else(|| {
        if password.is_empty() {
            "key".into()
        } else {
            "password".into()
        }
    });

    let server = RemoteServer {
        id: generate_id(),
        name: body.name.clone(),
        host: body.host.clone(),
        port,
        user: user.to_string(),
        auth_method,
        tags: body.tags.clone().unwrap_or_default(),
        memo: body.memo.clone().unwrap_or_default(),
        created_at: now_iso(),
        last_checked: String::new(),
        status: "unknown".into(),
        password_enc,
        has_password: !password.is_empty(),
        host_trust_version: uuid::Uuid::new_v4().to_string(),
        host_trust: None,
    };

    let _guard = SERVER_WRITES.lock().unwrap_or_else(|e| e.into_inner());
    let mut servers = match load_servers() {
        Ok(servers) => servers,
        Err(error) => return storage_error(error),
    };
    servers.push(server.clone());
    if let Err(e) = save_servers(&mut servers) {
        return storage_error(e);
    }
    HttpResponse::Ok().json(ApiResponse::ok(public(server)))
}

async fn update_server(
    path: web::Path<String>,
    body: web::Json<UpdateRemoteServerRequest>,
) -> HttpResponse {
    let server_id = path.into_inner();
    let _guard = SERVER_WRITES.lock().unwrap_or_else(|e| e.into_inner());
    let mut servers = match load_servers() {
        Ok(servers) => servers,
        Err(error) => return storage_error(error),
    };
    let srv = match servers.iter_mut().find(|s| s.id == server_id) {
        Some(s) => s,
        None => {
            return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Server not found"))
        }
    };

    if let Some(ref name) = body.name {
        srv.name = name.clone();
    }
    if let Some(ref host) = body.host {
        if !validate_host(host) {
            return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid hostname"));
        }
        if srv.host != *host {
            srv.host_trust = None;
            srv.host_trust_version = uuid::Uuid::new_v4().to_string();
            srv.status = "unknown".into();
            srv.last_checked.clear();
        }
        srv.host = host.clone();
    }
    if let Some(port) = body.port {
        if port == 0 {
            return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid port"));
        }
        if srv.port != port {
            srv.host_trust = None;
            srv.host_trust_version = uuid::Uuid::new_v4().to_string();
            srv.status = "unknown".into();
            srv.last_checked.clear();
        }
        srv.port = port;
    }
    if let Some(ref user) = body.user {
        if !validate_user(user) {
            return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid username"));
        }
        srv.user = user.clone();
    }
    if let Some(ref method) = body.auth_method {
        srv.auth_method = method.clone();
    }
    if let Some(ref tags) = body.tags {
        srv.tags = tags.clone();
    }
    if let Some(ref memo) = body.memo {
        srv.memo = memo.clone();
    }
    if let Some(password) = body
        .password
        .as_deref()
        .map(|p| p.trim_matches(['\r', '\n']))
    {
        if password.is_empty() {
            srv.password_enc.clear();
        } else if password.len() > 512 {
            return HttpResponse::Ok()
                .json(ApiResponse::<RemoteServer>::error("Password too long"));
        } else {
            match secret_store::encrypt(password) {
                Ok(enc) => srv.password_enc = enc,
                Err(e) => return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(&e)),
            }
        }
        srv.has_password = !srv.password_enc.is_empty();
    }

    let updated = public(srv.clone());
    if let Err(e) = save_servers(&mut servers) {
        return storage_error(e);
    }
    HttpResponse::Ok().json(ApiResponse::ok(updated))
}

async fn delete_server(path: web::Path<String>) -> HttpResponse {
    let server_id = path.into_inner();
    let _guard = SERVER_WRITES.lock().unwrap_or_else(|e| e.into_inner());
    let mut servers = match load_servers() {
        Ok(servers) => servers,
        Err(error) => return storage_error(error),
    };
    let before = servers.len();
    servers.retain(|s| s.id != server_id);
    if servers.len() == before {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Server not found"));
    }
    if let Err(e) = save_servers(&mut servers) {
        return storage_error(e);
    }
    HttpResponse::Ok().json(ApiResponse::ok("Server deleted".to_string()))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/remote-servers")
            .route("", web::get().to(list_servers))
            .route("", web::post().to(add_server))
            .route("/check-all", web::post().to(check_all_servers))
            .route("/ssh-key", web::get().to(ssh_keys::get_ssh_key_info))
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
            .route("/{id}/check", web::post().to(check_server))
            .route("/{id}/exec", web::post().to(exec_on_server))
            .route("/{id}/deploy-key", web::post().to(ssh_keys::deploy_key))
            .route("/{id}/test-key", web::post().to(ssh_keys::test_key_handler)),
    );
}

#[cfg(test)]
#[path = "remote_servers_tests.rs"]
mod tests;
