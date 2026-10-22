//! Verified host keys shared by every saved-server SSH path.
use crate::models::{ApiResponse, HostKeyTrust, RemoteServer};
use actix_web::{web, HttpRequest, HttpResponse};
use russh::keys::PublicKeyBase64;
use sha2::{Digest, Sha256};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustRequest {
    pub host: String,
    pub port: u16,
    pub public_key: String,
    pub fingerprint: String,
    pub previous_fingerprint: String,
    #[serde(default)]
    pub previous_version: String,
    pub reason: String,
}

pub fn parse_key(input: &str) -> Result<(String, String), String> {
    if input.len() > 8192 || input.contains(['\n', '\r']) {
        return Err("Provide one OpenSSH host public key line".into());
    }
    let parts: Vec<_> = input.split_whitespace().collect();
    if parts.len() < 2 {
        return Err("Provide the host public key, not only its fingerprint".into());
    }
    let key = russh::keys::parse_public_key_base64(parts[1])
        .map_err(|_| "Invalid SSH host public key")?;
    let algorithm = key.algorithm();
    let kind = algorithm.as_str();
    if parts[0] != kind {
        return Err("SSH public key type does not match its data".into());
    }
    Ok((
        format!("{} {}", kind, key.public_key_base64()),
        key.fingerprint(russh::keys::HashAlg::Sha256).to_string(),
    ))
}

pub async fn update(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<TrustRequest>,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let actor = match crate::membership::actor(&req, &users) {
        Ok(actor) => actor,
        Err(r) => return r,
    };
    // A remote-only member looks after the servers they registered.
    let own = crate::member_servers::is_member(&actor)
        && crate::remote_servers::find_server(&path)
            .ok()
            .flatten()
            .is_some_and(|s| s.owner_id == actor.id);
    if !actor.super_admin() && !own && !actor.manages(&crate::models::ScopeKind::Server, &path) {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error(
            "Host trust requires a server administrator",
        ));
    }
    if body.reason.trim().is_empty() || body.reason.len() > 2000 {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Independent verification reason is required",
        ));
    }
    let key = if body.public_key.trim().is_empty() && body.fingerprint.is_empty() {
        None
    } else {
        match parse_key(&body.public_key) {
            Ok((public_key, fingerprint)) if fingerprint == body.fingerprint => {
                Some(HostKeyTrust {
                    public_key,
                    fingerprint,
                    verified_by: actor.username,
                    verified_at: crate::remote_servers::now_iso(),
                    reason: body.reason.trim().to_owned(),
                })
            }
            _ => {
                return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                    "Public key must match the independently verified SHA256 fingerprint",
                ))
            }
        }
    };
    let _guard = crate::remote_servers::SERVER_WRITES
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut servers = match crate::remote_servers::load_servers() {
        Ok(s) => s,
        Err(e) => return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    };
    let Some(server) = servers.iter_mut().find(|s| s.id == path.as_str()) else {
        return HttpResponse::NotFound().json(ApiResponse::<()>::error("Server not found"));
    };
    if server.host != body.host || server.port != body.port {
        return HttpResponse::Conflict().json(ApiResponse::<()>::error(
            "Server endpoint changed; independently verify the current target",
        ));
    }
    let previous = server
        .host_trust
        .as_ref()
        .map(|k| k.fingerprint.as_str())
        .unwrap_or("");
    if previous != body.previous_fingerprint || server.host_trust_version != body.previous_version {
        return HttpResponse::Conflict().json(ApiResponse::<()>::error(
            "Host trust changed; reload before approving replacement",
        ));
    }
    server.host_trust = key;
    server.host_trust_version = uuid::Uuid::new_v4().to_string();
    server.status = "unknown".into();
    server.last_checked.clear();
    let response = crate::remote_servers::public(server.clone());
    if let Err(e) = crate::remote_servers::save_servers(&mut servers) {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e));
    }
    HttpResponse::Ok().json(ApiResponse::ok(response))
}

/// Each immutable trust version has its own known_hosts file. Existing sessions
/// retain their version; live revocation must separately bind the approval generation.
/// New sessions use current trust. No global TOFU fallback.
pub fn options(server: &RemoteServer) -> Result<Vec<String>, String> {
    options_at(server, &nabiman_server::data_dir())
}

fn options_at(server: &RemoteServer, directory: &std::path::Path) -> Result<Vec<String>, String> {
    let trust = server.host_trust.as_ref().ok_or(
        "Host key is not verified; register the independently verified key in server settings",
    )?;
    let (key, fingerprint) = parse_key(&trust.public_key)?;
    if fingerprint != trust.fingerprint {
        return Err("Stored host trust is inconsistent".into());
    }
    let identity = hex::encode(Sha256::digest(format!(
        "{}:{}:{}:{}",
        server.id, server.host, server.port, key
    )));
    let alias = format!("nabiman-{identity}");
    let path = directory
        .join("ssh-host-trust")
        .join(format!("{identity}.known_hosts"));
    crate::private_storage::write(
        &path,
        format!("{alias},[{alias}]:{} {key}\n", server.port).as_bytes(),
    )
    .map_err(|_| "Cannot write private SSH host trust file")?;
    Ok(vec![
        "-o".into(),
        "StrictHostKeyChecking=yes".into(),
        "-o".into(),
        format!("UserKnownHostsFile={}", path.display()),
        "-o".into(),
        "GlobalKnownHostsFile=/dev/null".into(),
        "-o".into(),
        "KnownHostsCommand=none".into(),
        "-o".into(),
        "VerifyHostKeyDNS=no".into(),
        "-o".into(),
        "UpdateHostKeys=no".into(),
        "-o".into(),
        format!("HostKeyAlias={alias}"),
    ])
}

pub fn shell_options(server: &RemoteServer) -> Result<String, String> {
    Ok(options(server)?
        .iter()
        .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" "))
}

#[cfg(test)]
#[path = "remote_host_trust_tests.rs"]
mod tests;
