use actix_web::{web, HttpResponse};
use std::fs;
use std::process::Command;
use crate::models::{ApiResponse, RemoteServer, SshKeyInfo, DeployKeyRequest};

fn data_dir_path() -> String {
    std::env::var("NABIMAN_DATA_DIR")
        .unwrap_or_else(|_| "/var/lib/nabiman".to_string())
}

fn nabiman_key_dir() -> String {
    format!("{}/ssh", data_dir_path())
}

pub fn nabiman_key_path() -> String {
    format!("{}/nabiman_ed25519", nabiman_key_dir())
}

pub fn nabiman_pubkey_path() -> String {
    format!("{}/nabiman_ed25519.pub", nabiman_key_dir())
}

pub fn ensure_ssh_key() -> Result<String, String> {
    let pubkey_path = nabiman_pubkey_path();
    let key_path = nabiman_key_path();

    if let Ok(pubkey) = fs::read_to_string(&pubkey_path) {
        if !pubkey.trim().is_empty() {
            return Ok(pubkey.trim().to_string());
        }
    }

    let key_dir = nabiman_key_dir();
    fs::create_dir_all(&key_dir).map_err(|e| format!("Cannot create dir: {}", e))?;

    let output = Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-f", &key_path, "-N", "", "-C", "nabiman@server-manager"])
        .output()
        .map_err(|e| format!("ssh-keygen failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ssh-keygen error: {}", stderr));
    }

    let _ = Command::new("chmod").args(["600", &key_path]).output();
    let _ = Command::new("chmod").args(["644", &pubkey_path]).output();

    fs::read_to_string(&pubkey_path)
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("Cannot read pubkey: {}", e))
}

fn get_key_info() -> SshKeyInfo {
    let key_path = nabiman_key_path();
    let pubkey_path = nabiman_pubkey_path();
    let exists = fs::metadata(&key_path).is_ok();
    let public_key = fs::read_to_string(&pubkey_path).ok().map(|s| s.trim().to_string());
    let fingerprint = if exists {
        Command::new("ssh-keygen").args(["-lf", &pubkey_path]).output().ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else { None };

    SshKeyInfo { exists, key_path, public_key, fingerprint }
}

fn fingerprint_from_pubkey() -> Option<String> {
    Command::new("ssh-keygen").args(["-lf", &nabiman_pubkey_path()]).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn deploy_key_to_server(server: &RemoteServer, password: &str) -> Result<String, String> {
    let pubkey = ensure_ssh_key()?;
    let port_str = server.port.to_string();
    let target = format!("{}@{}", server.user, server.host);

    let has_sshpass = Command::new("which").arg("sshpass").output()
        .map(|o| o.status.success()).unwrap_or(false);

    if has_sshpass {
        let key_path = nabiman_pubkey_path();
        let output = Command::new("sshpass")
            .args(["-p", password, "ssh-copy-id", "-i", &key_path,
                   "-o", "StrictHostKeyChecking=accept-new", "-p", &port_str, &target])
            .output()
            .map_err(|e| format!("sshpass failed: {}", e))?;

        if output.status.success() {
            return Ok("Public key deployed via ssh-copy-id".into());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Permission denied") {
            return Err("Password authentication failed".into());
        }
    }

    let remote_cmd = format!(
        "mkdir -p ~/.ssh && chmod 700 ~/.ssh && \
         echo '{}' >> ~/.ssh/authorized_keys && \
         chmod 600 ~/.ssh/authorized_keys && \
         sort -u -o ~/.ssh/authorized_keys ~/.ssh/authorized_keys",
        pubkey.replace('\'', "'\\''")
    );

    let output = Command::new("sshpass")
        .args(["-p", password, "ssh", "-o", "StrictHostKeyChecking=accept-new",
               "-p", &port_str, &target, &remote_cmd])
        .output();

    match output {
        Ok(result) if result.status.success() => Ok("Public key deployed via manual append".into()),
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            if !has_sshpass {
                Err(format!("sshpass not installed. Install: apt install sshpass\n\
                     Or manually: ssh-copy-id -i {} -p {} {}", nabiman_pubkey_path(), port_str, target))
            } else if stderr.contains("Permission denied") {
                Err("Password authentication failed".into())
            } else {
                Err(format!("Deploy failed: {}", stderr.trim()))
            }
        }
        Err(e) => {
            if !has_sshpass {
                Err(format!("sshpass not installed. Install: apt install sshpass\n\
                     Or manually: ssh-copy-id -i {} -p {} {}", nabiman_pubkey_path(), port_str, target))
            } else {
                Err(format!("Failed: {}", e))
            }
        }
    }
}

pub fn test_key_auth(server: &RemoteServer) -> bool {
    let port_str = server.port.to_string();
    let key_path = nabiman_key_path();

    if fs::metadata(&key_path).is_err() { return false; }

    Command::new("ssh")
        .args(["-i", &key_path, "-o", "StrictHostKeyChecking=accept-new",
               "-o", "ConnectTimeout=5", "-o", "BatchMode=yes",
               "-o", "PasswordAuthentication=no", "-p", &port_str,
               &format!("{}@{}", server.user, server.host), "echo ok"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- Handlers ---

pub async fn get_ssh_key_info() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(get_key_info()))
}

pub async fn generate_ssh_key() -> HttpResponse {
    match ensure_ssh_key() {
        Ok(pubkey) => HttpResponse::Ok().json(ApiResponse::ok(SshKeyInfo {
            exists: true,
            key_path: nabiman_key_path(),
            public_key: Some(pubkey),
            fingerprint: fingerprint_from_pubkey(),
        })),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<SshKeyInfo>::error(&e)),
    }
}

pub async fn deploy_key(
    path: web::Path<String>,
    body: web::Json<DeployKeyRequest>,
) -> HttpResponse {
    let server_id = path.into_inner();
    let servers = super::remote_servers::load_servers();
    let srv = match servers.iter().find(|s| s.id == server_id) {
        Some(s) => s,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Server not found")),
    };
    if body.password.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Password required"));
    }
    match deploy_key_to_server(srv, &body.password) {
        Ok(msg) => HttpResponse::Ok().json(ApiResponse::ok(msg)),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    }
}

pub async fn test_key_handler(path: web::Path<String>) -> HttpResponse {
    let server_id = path.into_inner();
    let servers = super::remote_servers::load_servers();
    let srv = match servers.iter().find(|s| s.id == server_id) {
        Some(s) => s,
        None => return HttpResponse::Ok().json(ApiResponse::<bool>::error("Server not found")),
    };
    HttpResponse::Ok().json(ApiResponse::ok(test_key_auth(srv)))
}
