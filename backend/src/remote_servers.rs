use actix_web::{web, HttpResponse};
use std::fs;
use std::process::Command;
use crate::models::{
    ApiResponse, RemoteServer, RemoteServerStatus, SshKeyInfo, DeployKeyRequest,
    AddRemoteServerRequest, UpdateRemoteServerRequest, RemoteExecRequest,
};

const DATA_FILE: &str = "/var/lib/nabiman/remote_servers.json";
const DATA_DIR: &str = "/var/lib/nabiman";

fn data_file_path() -> String {
    std::env::var("NABIMAN_DATA_DIR")
        .map(|d| format!("{}/remote_servers.json", d))
        .unwrap_or_else(|_| DATA_FILE.to_string())
}

fn data_dir_path() -> String {
    std::env::var("NABIMAN_DATA_DIR")
        .unwrap_or_else(|_| DATA_DIR.to_string())
}

fn load_servers() -> Vec<RemoteServer> {
    let path = data_file_path();
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save_servers(servers: &[RemoteServer]) -> Result<(), String> {
    let dir = data_dir_path();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    let json = serde_json::to_string_pretty(servers)
        .map_err(|e| format!("Serialize error: {}", e))?;
    fs::write(&data_file_path(), json).map_err(|e| format!("Write error: {}", e))
}

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("srv_{:x}_{:x}", ts.as_secs(), ts.subsec_nanos())
}

fn now_iso() -> String {
    let output = Command::new("date").args(["-u", "+%Y-%m-%dT%H:%M:%SZ"]).output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "unknown".into(),
    }
}

fn validate_host(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 256
        && s.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == ':')
}

fn validate_user(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 64
        && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// SSH probe: connect to remote and gather system info
fn ssh_probe(server: &RemoteServer) -> RemoteServerStatus {
    let checked_at = now_iso();

    // Combined command to get all info in one SSH call
    let remote_cmd = concat!(
        "hostname 2>/dev/null || echo unknown;",
        "cat /etc/os-release 2>/dev/null | grep PRETTY_NAME | head -1 | cut -d= -f2 | tr -d '\"' || uname -s;",
        "uptime -p 2>/dev/null || uptime | sed 's/.*up/up/';",
        "top -bn1 2>/dev/null | grep 'Cpu(s)' | awk '{print $2}' || echo 0;",
        "free -m 2>/dev/null | awk '/^Mem:/{printf \"%d/%dMB (%.1f%%)\", $3, $2, $3/$2*100}' || echo unknown;",
        "df -h / 2>/dev/null | awk 'NR==2{printf \"%s/%s (%s)\", $3, $2, $5}' || echo unknown;",
        "cat /proc/loadavg 2>/dev/null | awk '{print $1, $2, $3}' || echo unknown"
    );

    let port_str = server.port.to_string();
    let output = Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=accept-new",
            "-o", "ConnectTimeout=5",
            "-o", "BatchMode=yes",
            "-o", "ServerAliveInterval=5",
            "-p", &port_str,
            &format!("{}@{}", server.user, server.host),
            remote_cmd,
        ])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            let lines: Vec<&str> = stdout.lines().collect();

            RemoteServerStatus {
                id: server.id.clone(),
                name: server.name.clone(),
                host: server.host.clone(),
                status: "online".into(),
                hostname: lines.first().unwrap_or(&"unknown").to_string(),
                os: lines.get(1).unwrap_or(&"unknown").to_string(),
                uptime: lines.get(2).unwrap_or(&"unknown").to_string(),
                cpu_usage: format!("{}%", lines.get(3).unwrap_or(&"0").trim()),
                memory: lines.get(4).unwrap_or(&"unknown").to_string(),
                disk: lines.get(5).unwrap_or(&"unknown").to_string(),
                load: lines.get(6).unwrap_or(&"unknown").to_string(),
                checked_at,
            }
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();
            let reason = if stderr.contains("Connection refused") {
                "Connection refused"
            } else if stderr.contains("Connection timed out") || stderr.contains("timed out") {
                "Connection timed out"
            } else if stderr.contains("Permission denied") {
                "Auth failed (Permission denied)"
            } else if stderr.contains("No route to host") {
                "No route to host"
            } else if stderr.contains("Host key verification") {
                "Host key verification failed"
            } else {
                "Connection failed"
            };
            RemoteServerStatus {
                id: server.id.clone(),
                name: server.name.clone(),
                host: server.host.clone(),
                status: "offline".into(),
                hostname: reason.into(),
                os: String::new(), uptime: String::new(), cpu_usage: String::new(),
                memory: String::new(), disk: String::new(), load: String::new(),
                checked_at,
            }
        }
        Err(e) => {
            RemoteServerStatus {
                id: server.id.clone(),
                name: server.name.clone(),
                host: server.host.clone(),
                status: "offline".into(),
                hostname: format!("SSH error: {}", e),
                os: String::new(), uptime: String::new(), cpu_usage: String::new(),
                memory: String::new(), disk: String::new(), load: String::new(),
                checked_at,
            }
        }
    }
}

// --- SSH Key helpers ---

fn nabiman_key_dir() -> String {
    let data_dir = data_dir_path();
    format!("{}/ssh", data_dir)
}

fn nabiman_key_path() -> String {
    format!("{}/nabiman_ed25519", nabiman_key_dir())
}

fn nabiman_pubkey_path() -> String {
    format!("{}/nabiman_ed25519.pub", nabiman_key_dir())
}

fn ensure_ssh_key() -> Result<String, String> {
    let pubkey_path = nabiman_pubkey_path();
    let key_path = nabiman_key_path();

    // Return existing public key if present
    if let Ok(pubkey) = fs::read_to_string(&pubkey_path) {
        if !pubkey.trim().is_empty() {
            return Ok(pubkey.trim().to_string());
        }
    }

    // Generate new key pair
    let key_dir = nabiman_key_dir();
    fs::create_dir_all(&key_dir).map_err(|e| format!("Cannot create dir: {}", e))?;

    let output = Command::new("ssh-keygen")
        .args([
            "-t", "ed25519",
            "-f", &key_path,
            "-N", "",         // no passphrase
            "-C", "nabiman@server-manager",
        ])
        .output()
        .map_err(|e| format!("ssh-keygen failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ssh-keygen error: {}", stderr));
    }

    // Set permissions
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
        Command::new("ssh-keygen")
            .args(["-lf", &pubkey_path])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        None
    };

    SshKeyInfo {
        exists,
        key_path: key_path.clone(),
        public_key,
        fingerprint,
    }
}

/// Deploy public key to a remote server using sshpass + ssh-copy-id,
/// or manual append if sshpass is not available.
fn deploy_key_to_server(server: &RemoteServer, password: &str) -> Result<String, String> {
    let pubkey = ensure_ssh_key()?;
    let port_str = server.port.to_string();
    let target = format!("{}@{}", server.user, server.host);

    // Try sshpass + ssh-copy-id first (cleanest)
    let has_sshpass = Command::new("which").arg("sshpass").output()
        .map(|o| o.status.success()).unwrap_or(false);

    if has_sshpass {
        let key_path = nabiman_pubkey_path();
        let output = Command::new("sshpass")
            .args([
                "-p", password,
                "ssh-copy-id",
                "-i", &key_path,
                "-o", "StrictHostKeyChecking=accept-new",
                "-p", &port_str,
                &target,
            ])
            .output()
            .map_err(|e| format!("sshpass failed: {}", e))?;

        if output.status.success() {
            return Ok("Public key deployed via ssh-copy-id".into());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Permission denied") {
            return Err("Password authentication failed".into());
        }
        // Fall through to manual method
    }

    // Manual method: pipe password to ssh and append to authorized_keys
    // Use printf to pipe password, avoiding shell escaping issues
    let remote_cmd = format!(
        "mkdir -p ~/.ssh && chmod 700 ~/.ssh && \
         echo '{}' >> ~/.ssh/authorized_keys && \
         chmod 600 ~/.ssh/authorized_keys && \
         sort -u -o ~/.ssh/authorized_keys ~/.ssh/authorized_keys",
        pubkey.replace('\'', "'\\''")
    );

    let output = Command::new("sshpass")
        .args([
            "-p", password,
            "ssh",
            "-o", "StrictHostKeyChecking=accept-new",
            "-p", &port_str,
            &target,
            &remote_cmd,
        ])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            Ok("Public key deployed via manual append".into())
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            if !has_sshpass {
                // No sshpass available, give instructions
                Err(format!(
                    "sshpass not installed. Install it first:\n  apt install sshpass  (or)  yum install sshpass\n\n\
                     Or manually copy the key:\n  ssh-copy-id -i {} -p {} {}",
                    nabiman_pubkey_path(), port_str, target
                ))
            } else if stderr.contains("Permission denied") {
                Err("Password authentication failed".into())
            } else {
                Err(format!("Deploy failed: {}", stderr.trim()))
            }
        }
        Err(e) => {
            if !has_sshpass {
                Err(format!(
                    "sshpass not installed. Install it first:\n  apt install sshpass  (or)  yum install sshpass\n\n\
                     Or manually copy the key:\n  ssh-copy-id -i {} -p {} {}",
                    nabiman_pubkey_path(), port_str, target
                ))
            } else {
                Err(format!("Failed: {}", e))
            }
        }
    }
}

/// Test if key auth already works for a server
fn test_key_auth(server: &RemoteServer) -> bool {
    let port_str = server.port.to_string();
    let key_path = nabiman_key_path();

    if fs::metadata(&key_path).is_err() {
        return false;
    }

    Command::new("ssh")
        .args([
            "-i", &key_path,
            "-o", "StrictHostKeyChecking=accept-new",
            "-o", "ConnectTimeout=5",
            "-o", "BatchMode=yes",
            "-o", "PasswordAuthentication=no",
            "-p", &port_str,
            &format!("{}@{}", server.user, server.host),
            "echo ok",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- Handlers ---

async fn get_ssh_key_info() -> HttpResponse {
    let info = get_key_info();
    HttpResponse::Ok().json(ApiResponse::ok(info))
}

async fn generate_ssh_key() -> HttpResponse {
    match ensure_ssh_key() {
        Ok(pubkey) => HttpResponse::Ok().json(ApiResponse::ok(SshKeyInfo {
            exists: true,
            key_path: nabiman_key_path(),
            public_key: Some(pubkey),
            fingerprint: Command::new("ssh-keygen")
                .args(["-lf", &nabiman_pubkey_path()])
                .output().ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()),
        })),
        Err(e) => HttpResponse::Ok().json(ApiResponse::<SshKeyInfo>::error(&e)),
    }
}

async fn deploy_key(
    path: web::Path<String>,
    body: web::Json<DeployKeyRequest>,
) -> HttpResponse {
    let server_id = path.into_inner();
    let servers = load_servers();
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

async fn test_key(path: web::Path<String>) -> HttpResponse {
    let server_id = path.into_inner();
    let servers = load_servers();
    let srv = match servers.iter().find(|s| s.id == server_id) {
        Some(s) => s,
        None => return HttpResponse::Ok().json(ApiResponse::<bool>::error("Server not found")),
    };

    let works = test_key_auth(srv);
    HttpResponse::Ok().json(ApiResponse::ok(works))
}

async fn list_servers() -> HttpResponse {
    let servers = load_servers();
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

    let server = RemoteServer {
        id: generate_id(),
        name: body.name.clone(),
        host: body.host.clone(),
        port,
        user: user.to_string(),
        auth_method: body.auth_method.as_deref().unwrap_or("key").to_string(),
        tags: body.tags.clone().unwrap_or_default(),
        memo: body.memo.clone().unwrap_or_default(),
        created_at: now_iso(),
        last_checked: String::new(),
        status: "unknown".into(),
    };

    let mut servers = load_servers();
    servers.push(server.clone());
    if let Err(e) = save_servers(&servers) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(&e));
    }

    HttpResponse::Ok().json(ApiResponse::ok(server))
}

async fn update_server(
    path: web::Path<String>,
    body: web::Json<UpdateRemoteServerRequest>,
) -> HttpResponse {
    let server_id = path.into_inner();
    let mut servers = load_servers();

    let srv = match servers.iter_mut().find(|s| s.id == server_id) {
        Some(s) => s,
        None => return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Server not found")),
    };

    if let Some(ref name) = body.name { srv.name = name.clone(); }
    if let Some(ref host) = body.host {
        if !validate_host(host) {
            return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid hostname"));
        }
        srv.host = host.clone();
    }
    if let Some(port) = body.port { srv.port = port; }
    if let Some(ref user) = body.user {
        if !validate_user(user) {
            return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid username"));
        }
        srv.user = user.clone();
    }
    if let Some(ref method) = body.auth_method { srv.auth_method = method.clone(); }
    if let Some(ref tags) = body.tags { srv.tags = tags.clone(); }
    if let Some(ref memo) = body.memo { srv.memo = memo.clone(); }

    let updated = srv.clone();
    if let Err(e) = save_servers(&servers) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(&e));
    }
    HttpResponse::Ok().json(ApiResponse::ok(updated))
}

async fn delete_server(path: web::Path<String>) -> HttpResponse {
    let server_id = path.into_inner();
    let mut servers = load_servers();
    let before = servers.len();
    servers.retain(|s| s.id != server_id);
    if servers.len() == before {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Server not found"));
    }
    if let Err(e) = save_servers(&servers) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(&e));
    }
    HttpResponse::Ok().json(ApiResponse::ok("Server deleted".to_string()))
}

async fn check_server(path: web::Path<String>) -> HttpResponse {
    let server_id = path.into_inner();
    let mut servers = load_servers();
    let srv = match servers.iter_mut().find(|s| s.id == server_id) {
        Some(s) => s.clone(),
        None => return HttpResponse::Ok().json(ApiResponse::<RemoteServerStatus>::error("Server not found")),
    };

    let result = ssh_probe(&srv);

    // Update status in storage
    if let Some(s) = servers.iter_mut().find(|s| s.id == server_id) {
        s.status = result.status.clone();
        s.last_checked = result.checked_at.clone();
    }
    let _ = save_servers(&servers);

    HttpResponse::Ok().json(ApiResponse::ok(result))
}

async fn check_all_servers() -> HttpResponse {
    let mut servers = load_servers();
    let mut results: Vec<RemoteServerStatus> = Vec::new();

    for srv in &servers {
        let result = ssh_probe(srv);
        results.push(result);
    }

    // Update all statuses
    for result in &results {
        if let Some(s) = servers.iter_mut().find(|s| s.id == result.id) {
            s.status = result.status.clone();
            s.last_checked = result.checked_at.clone();
        }
    }
    let _ = save_servers(&servers);

    HttpResponse::Ok().json(ApiResponse::ok(results))
}

async fn exec_on_server(
    path: web::Path<String>,
    body: web::Json<RemoteExecRequest>,
) -> HttpResponse {
    let server_id = path.into_inner();
    let servers = load_servers();
    let srv = match servers.iter().find(|s| s.id == server_id) {
        Some(s) => s,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Server not found")),
    };

    // Validate command: block dangerous patterns
    let cmd = body.command.trim();
    if cmd.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Empty command"));
    }
    if cmd.len() > 1024 {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Command too long"));
    }
    let dangerous = ["rm -rf /", "mkfs", "dd if=", "> /dev/sd", ":(){ :|:& };:"];
    for pattern in &dangerous {
        if cmd.contains(pattern) {
            return HttpResponse::Ok().json(ApiResponse::<String>::error("Dangerous command blocked"));
        }
    }

    let port_str = srv.port.to_string();
    let output = Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=accept-new",
            "-o", "ConnectTimeout=10",
            "-o", "BatchMode=yes",
            "-p", &port_str,
            &format!("{}@{}", srv.user, srv.host),
            cmd,
        ])
        .output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();
            let combined = if stderr.is_empty() {
                stdout
            } else if stdout.is_empty() {
                stderr
            } else {
                format!("{}\n--- stderr ---\n{}", stdout, stderr)
            };
            if result.status.success() {
                HttpResponse::Ok().json(ApiResponse::ok(combined))
            } else {
                HttpResponse::Ok().json(ApiResponse::<String>::error(&combined))
            }
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&format!("SSH error: {}", e))),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/remote-servers")
            .route("", web::get().to(list_servers))
            .route("", web::post().to(add_server))
            .route("/check-all", web::post().to(check_all_servers))
            .route("/ssh-key", web::get().to(get_ssh_key_info))
            .route("/ssh-key/generate", web::post().to(generate_ssh_key))
            .route("/{id}", web::put().to(update_server))
            .route("/{id}", web::delete().to(delete_server))
            .route("/{id}/check", web::post().to(check_server))
            .route("/{id}/exec", web::post().to(exec_on_server))
            .route("/{id}/deploy-key", web::post().to(deploy_key))
            .route("/{id}/test-key", web::post().to(test_key)),
    );
}
