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
    /// The scan found the server speaks only the old SSH algorithms.
    #[serde(default)]
    pub legacy: bool,
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
    server.host_trust = key.clone();
    server.legacy_ssh = key.is_some() && body.legacy;
    server.host_trust_version = uuid::Uuid::new_v4().to_string();
    server.status = "unknown".into();
    server.last_checked.clear();
    let response = crate::remote_servers::public(server.clone());
    if let Err(e) = crate::remote_servers::save_servers(&mut servers) {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e));
    }
    HttpResponse::Ok().json(ApiResponse::ok(response))
}

/// The key the server itself presents, read with ssh-keyscan, to fill in the
/// form: nothing is trusted until someone compares the fingerprint and
/// registers it. A member's server is asked at its checked public address.
pub async fn scan(
    req: HttpRequest,
    path: web::Path<String>,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let actor = match crate::membership::actor(&req, &users) {
        Ok(actor) => actor,
        Err(r) => return r,
    };
    let server = match crate::remote_servers::find_server(&path) {
        Ok(Some(s)) if crate::member_servers::manageable(&actor, &s) => s,
        Ok(_) => return HttpResponse::NotFound().json(ApiResponse::<()>::error("Server not found")),
        Err(e) => return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    };
    let found = web::block(move || {
        let host = crate::member_servers::dial_host(&server, server.port)?;
        let port = server.port.to_string();
        // Modern path first: ssh-keyscan, which refuses the old algorithms.
        if let Some(found) = keyscan(&host, &port) {
            return Ok((found.0, found.1, false));
        }
        // Nothing — the server may speak only the old algorithms. Fetch its
        // key with `ssh` itself, which can be told to allow them.
        legacy_keyscan(&host, &port)
            .map(|(k, f)| (k, f, true))
            .ok_or_else(|| format!("{}:{} 에서 SSH 호스트 키를 받지 못했습니다", server.host, server.port))
    })
    .await;
    match found {
        Ok(Ok((public_key, fingerprint, legacy))) => HttpResponse::Ok()
            .json(ApiResponse::ok(serde_json::json!({ "public_key": public_key, "fingerprint": fingerprint, "legacy": legacy }))),
        Ok(Err(e)) => HttpResponse::Ok().json(ApiResponse::<()>::error(&e)),
        Err(_) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error("The scan stopped unexpectedly")),
    }
}

/// The host key via `ssh-keyscan` (strong algorithms only), strongest kind first.
fn keyscan(host: &str, port: &str) -> Option<(String, String)> {
    let out = std::process::Command::new("ssh-keyscan")
        .args(["-T", "8", "-t", "ed25519,ecdsa,rsa", "-p", port, "--", host])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let keys: Vec<_> = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| parse_key(&l.split_whitespace().skip(1).collect::<Vec<_>>().join(" ")).ok())
        .collect();
    ["ssh-ed25519", "ecdsa", "rsa"]
        .iter()
        .find_map(|kind| keys.iter().find(|(k, _)| k.contains(kind)).cloned())
}

/// The host key from an old server, read with `ssh` allowing the legacy
/// algorithms. ssh records the host key during the handshake; the login then
/// fails (no credentials) but the key has already been written out.
fn legacy_keyscan(host: &str, port: &str) -> Option<(String, String)> {
    let dir = std::env::temp_dir().join(format!("nabiman-hk-{}", uuid::Uuid::new_v4()));
    let known = dir.join("known_hosts");
    std::fs::create_dir_all(&dir).ok()?;
    let mut cmd = std::process::Command::new("ssh");
    cmd.args(["-p", port, "-o", "BatchMode=yes", "-o", "ConnectTimeout=8",
        "-o", "StrictHostKeyChecking=accept-new",
        "-o", &format!("UserKnownHostsFile={}", known.display()),
        "-o", "GlobalKnownHostsFile=/dev/null"]);
    cmd.args(legacy_ssh_opts());
    cmd.args(["-o", "PubkeyAuthentication=no", "-o", "PasswordAuthentication=no",
        "-o", "KbdInteractiveAuthentication=no",
        &format!("nabiman-probe@{host}"), "true"]);
    let _ = cmd.output();
    let line = std::fs::read_to_string(&known).ok();
    let _ = std::fs::remove_dir_all(&dir);
    line?
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .find_map(|l| parse_key(&l.split_whitespace().skip(1).collect::<Vec<_>>().join(" ")).ok())
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
    let mut options = vec![
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
    ];
    // Old servers (e.g. OpenSSH 4.x) offer only legacy algorithms. These are
    // appended with `+`, so a modern server still negotiates strong ones
    // first and only a server with nothing better falls back to these. The
    // host key is pinned above, so this does not weaken modern connections.
    options.extend(legacy_ssh_opts());
    Ok(options)
}

/// `-o` flags that let `ssh` accept the old algorithms as a last resort. Only
/// the tokens this `ssh` build still knows are included (filtered with
/// `ssh -Q`), so an upgrade that drops one never turns a flag into an error
/// that breaks every connection. Computed once.
pub fn legacy_ssh_opts() -> Vec<String> {
    static OPTS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    OPTS.get_or_init(build_legacy_opts).clone()
}

fn build_legacy_opts() -> Vec<String> {
    let known = |query: &str| -> std::collections::HashSet<String> {
        std::process::Command::new("ssh")
            .args(["-Q", query])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).collect())
            .unwrap_or_default()
    };
    let (kex, cipher, mac, key) = (known("kex"), known("cipher"), known("mac"), known("key"));
    let mut out = Vec::new();
    let mut add = |name: &str, want: &[&str], have: &std::collections::HashSet<String>| {
        let got: Vec<&str> = want.iter().copied().filter(|t| have.contains(*t)).collect();
        if !got.is_empty() {
            out.push("-o".to_string());
            out.push(format!("{name}=+{}", got.join(",")));
        }
    };
    add("KexAlgorithms", &["diffie-hellman-group-exchange-sha1", "diffie-hellman-group14-sha1", "diffie-hellman-group1-sha1"], &kex);
    add("HostKeyAlgorithms", &["ssh-rsa", "ssh-dss"], &key);
    add("PubkeyAcceptedAlgorithms", &["ssh-rsa", "ssh-dss"], &key);
    add("Ciphers", &["aes256-cbc", "aes192-cbc", "aes128-cbc", "3des-cbc"], &cipher);
    add("MACs", &["hmac-sha1", "hmac-sha1-96", "hmac-md5"], &mac);
    out
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
