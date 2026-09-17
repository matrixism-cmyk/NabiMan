use actix_web::{web, HttpResponse};
use std::fs;
use std::process::Command;
use crate::models::{
    ApiResponse, RemoteServer, RemoteServerStatus,
    AddRemoteServerRequest, UpdateRemoteServerRequest, RemoteExecRequest,
};
use crate::ssh_keys;
use crate::secret_store;
use crate::ssh_auth;

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

pub fn load_servers() -> Vec<RemoteServer> {
    let path = data_file_path();
    let mut servers: Vec<RemoteServer> = match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    for s in servers.iter_mut() {
        s.has_password = !s.password_enc.is_empty();
    }
    servers
}

/// Strip the encrypted password before a server record goes out over the API.
fn public(mut server: RemoteServer) -> RemoteServer {
    server.has_password = !server.password_enc.is_empty();
    server.password_enc = String::new();
    server
}

/// Look up one saved server (used by the terminal WebSocket to resolve
/// `server_id` into host/user/port and, when stored, a password).
pub fn find_server(id: &str) -> Option<RemoteServer> {
    load_servers().into_iter().find(|s| s.id == id)
}

/// Decrypt the stored password of a server, if it has one.
pub fn server_password(srv: &RemoteServer) -> Option<String> {
    if srv.password_enc.is_empty() {
        return None;
    }
    match secret_store::decrypt(&srv.password_enc) {
        Ok(pw) => Some(pw),
        Err(e) => {
            eprintln!("remote_servers: cannot decrypt password for {}: {}", srv.id, e);
            None
        }
    }
}

/// Build the ssh invocation for a server: plain `ssh` with the NabiMan key, or
/// `sshpass -f <file> ssh` when a password is stored. The returned guard must
/// outlive the command — dropping it deletes the password file.
fn ssh_command(srv: &RemoteServer, connect_timeout: u32, remote_cmd: &str)
    -> Result<(Command, Option<ssh_auth::PwFile>), String>
{
    let port = srv.port.to_string();
    let target = format!("{}@{}", srv.user, srv.host);
    let timeout = format!("ConnectTimeout={}", connect_timeout);

    let password = server_password(srv);
    if let Some(pw) = password {
        let pw_file = ssh_auth::write_password_file(&pw)?;
        let mut cmd = Command::new("sshpass");
        cmd.args(["-f", pw_file.path(), "ssh",
                  "-o", "StrictHostKeyChecking=accept-new",
                  "-o", &timeout]);
        cmd.args(ssh_auth::password_auth_opts());
        cmd.args(["-p", &port, &target, remote_cmd]);
        return Ok((cmd, Some(pw_file)));
    }

    let mut cmd = Command::new("ssh");
    cmd.args(["-o", "StrictHostKeyChecking=accept-new",
              "-o", &timeout,
              "-o", "BatchMode=yes",
              "-o", "ServerAliveInterval=15",
              "-p", &port, &target, remote_cmd]);
    Ok((cmd, None))
}

fn save_servers(servers: &mut [RemoteServer]) -> Result<(), String> {
    let dir = data_dir_path();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    for s in servers.iter_mut() {
        s.has_password = !s.password_enc.is_empty();
    }
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
    !s.is_empty() && s.len() < 256
        && s.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == ':')
}

fn validate_user(s: &str) -> bool {
    !s.is_empty() && s.len() < 64
        && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn ssh_probe(server: &RemoteServer) -> RemoteServerStatus {
    let checked_at = now_iso();
    let remote_cmd = concat!(
        "hostname 2>/dev/null || echo unknown;",
        "cat /etc/os-release 2>/dev/null | grep PRETTY_NAME | head -1 | cut -d= -f2 | tr -d '\"' || uname -s;",
        "uptime -p 2>/dev/null || uptime | sed 's/.*up/up/';",
        "top -bn1 2>/dev/null | grep 'Cpu(s)' | awk '{print $2}' || echo 0;",
        "free -m 2>/dev/null | awk '/^Mem:/{printf \"%d/%dMB (%.1f%%)\", $3, $2, $3/$2*100}' || echo unknown;",
        "df -h / 2>/dev/null | awk 'NR==2{printf \"%s/%s (%s)\", $3, $2, $5}' || echo unknown;",
        "cat /proc/loadavg 2>/dev/null | awk '{print $1, $2, $3}' || echo unknown"
    );

    let (mut cmd, _pw_guard) = match ssh_command(server, 5, remote_cmd) {
        Ok(c) => c,
        Err(e) => return RemoteServerStatus {
            id: server.id.clone(), name: server.name.clone(), host: server.host.clone(),
            status: "offline".into(), hostname: format!("SSH setup error: {}", e),
            os: String::new(), uptime: String::new(), cpu_usage: String::new(),
            memory: String::new(), disk: String::new(), load: String::new(), checked_at,
        },
    };
    let output = cmd.output();

    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            let lines: Vec<&str> = stdout.lines().collect();
            RemoteServerStatus {
                id: server.id.clone(), name: server.name.clone(), host: server.host.clone(),
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
            // sshpass swallows ssh's own message, so fall back to its exit code:
            // 5 = password rejected, 6/7 = host key unknown/changed.
            let code_reason = match result.status.code() {
                Some(5) => Some("Auth failed (password rejected)"),
                Some(6) => Some("Host key unknown"),
                Some(7) => Some("Host key changed"),
                _ => None,
            };
            let reason = if stderr.contains("Connection refused") { "Connection refused" }
                else if stderr.contains("timed out") { "Connection timed out" }
                else if stderr.contains("Permission denied") { "Auth failed (Permission denied)" }
                else if stderr.contains("No route to host") { "No route to host" }
                else if stderr.contains("Host key verification") { "Host key verification failed" }
                else if let Some(r) = code_reason { r }
                else { "Connection failed" };
            RemoteServerStatus {
                id: server.id.clone(), name: server.name.clone(), host: server.host.clone(),
                status: "offline".into(), hostname: reason.into(),
                os: String::new(), uptime: String::new(), cpu_usage: String::new(),
                memory: String::new(), disk: String::new(), load: String::new(), checked_at,
            }
        }
        Err(e) => RemoteServerStatus {
            id: server.id.clone(), name: server.name.clone(), host: server.host.clone(),
            status: "offline".into(), hostname: format!("SSH error: {}", e),
            os: String::new(), uptime: String::new(), cpu_usage: String::new(),
            memory: String::new(), disk: String::new(), load: String::new(), checked_at,
        },
    }
}

// --- Handlers ---

async fn list_servers() -> HttpResponse {
    let servers: Vec<RemoteServer> = load_servers().into_iter().map(public).collect();
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
    let password = body.password.as_deref().unwrap_or("").trim_matches(['\r', '\n']);
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
    let auth_method = body.auth_method.clone()
        .unwrap_or_else(|| if password.is_empty() { "key".into() } else { "password".into() });

    let server = RemoteServer {
        id: generate_id(), name: body.name.clone(), host: body.host.clone(),
        port, user: user.to_string(),
        auth_method,
        tags: body.tags.clone().unwrap_or_default(),
        memo: body.memo.clone().unwrap_or_default(),
        created_at: now_iso(), last_checked: String::new(), status: "unknown".into(),
        password_enc,
        has_password: !password.is_empty(),
    };

    let mut servers = load_servers();
    servers.push(server.clone());
    if let Err(e) = save_servers(&mut servers) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(&e));
    }
    HttpResponse::Ok().json(ApiResponse::ok(public(server)))
}

async fn update_server(path: web::Path<String>, body: web::Json<UpdateRemoteServerRequest>) -> HttpResponse {
    let server_id = path.into_inner();
    let mut servers = load_servers();
    let srv = match servers.iter_mut().find(|s| s.id == server_id) {
        Some(s) => s,
        None => return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Server not found")),
    };

    if let Some(ref name) = body.name { srv.name = name.clone(); }
    if let Some(ref host) = body.host {
        if !validate_host(host) { return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid hostname")); }
        srv.host = host.clone();
    }
    if let Some(port) = body.port {
        if port == 0 { return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid port")); }
        srv.port = port;
    }
    if let Some(ref user) = body.user {
        if !validate_user(user) { return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid username")); }
        srv.user = user.clone();
    }
    if let Some(ref method) = body.auth_method { srv.auth_method = method.clone(); }
    if let Some(ref tags) = body.tags { srv.tags = tags.clone(); }
    if let Some(ref memo) = body.memo { srv.memo = memo.clone(); }
    if let Some(password) = body.password.as_deref().map(|p| p.trim_matches(['\r', '\n'])) {
        if password.is_empty() {
            srv.password_enc.clear();
        } else if password.len() > 512 {
            return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Password too long"));
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
    if let Err(e) = save_servers(&mut servers) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error(&e));
    }
    HttpResponse::Ok().json(ApiResponse::ok("Server deleted".to_string()))
}

async fn check_server(path: web::Path<String>) -> HttpResponse {
    let server_id = path.into_inner();
    let mut servers = load_servers();
    let srv = match servers.iter().find(|s| s.id == server_id) {
        Some(s) => s.clone(),
        None => return HttpResponse::Ok().json(ApiResponse::<RemoteServerStatus>::error("Server not found")),
    };
    let result = ssh_probe(&srv);
    if let Some(s) = servers.iter_mut().find(|s| s.id == server_id) {
        s.status = result.status.clone();
        s.last_checked = result.checked_at.clone();
    }
    let _ = save_servers(&mut servers);
    HttpResponse::Ok().json(ApiResponse::ok(result))
}

async fn check_all_servers() -> HttpResponse {
    let mut servers = load_servers();
    let results: Vec<RemoteServerStatus> = servers.iter().map(|srv| ssh_probe(srv)).collect();
    for result in &results {
        if let Some(s) = servers.iter_mut().find(|s| s.id == result.id) {
            s.status = result.status.clone();
            s.last_checked = result.checked_at.clone();
        }
    }
    let _ = save_servers(&mut servers);
    HttpResponse::Ok().json(ApiResponse::ok(results))
}

async fn exec_on_server(path: web::Path<String>, body: web::Json<RemoteExecRequest>) -> HttpResponse {
    let server_id = path.into_inner();
    let servers = load_servers();
    let srv = match servers.iter().find(|s| s.id == server_id) {
        Some(s) => s,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Server not found")),
    };

    let cmd = body.command.trim();
    if cmd.is_empty() { return HttpResponse::Ok().json(ApiResponse::<String>::error("Empty command")); }
    if cmd.len() > 1024 { return HttpResponse::Ok().json(ApiResponse::<String>::error("Command too long")); }
    let dangerous = ["rm -rf /", "mkfs", "dd if=", "> /dev/sd", ":(){ :|:& };:"];
    for pattern in &dangerous {
        if cmd.contains(pattern) {
            return HttpResponse::Ok().json(ApiResponse::<String>::error("Dangerous command blocked"));
        }
    }

    let (mut ssh, _pw_guard) = match ssh_command(srv, 10, cmd) {
        Ok(c) => c,
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    };
    let output = ssh.output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();
            let combined = if stderr.is_empty() { stdout }
                else if stdout.is_empty() { stderr }
                else { format!("{}\n--- stderr ---\n{}", stdout, stderr) };
            if result.status.success() { HttpResponse::Ok().json(ApiResponse::ok(combined)) }
            else { HttpResponse::Ok().json(ApiResponse::<String>::error(&combined)) }
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
            .route("/ssh-key", web::get().to(ssh_keys::get_ssh_key_info))
            .route("/ssh-key/generate", web::post().to(ssh_keys::generate_ssh_key))
            .route("/{id}", web::put().to(update_server))
            .route("/{id}", web::delete().to(delete_server))
            .route("/{id}/check", web::post().to(check_server))
            .route("/{id}/exec", web::post().to(exec_on_server))
            .route("/{id}/deploy-key", web::post().to(ssh_keys::deploy_key))
            .route("/{id}/test-key", web::post().to(ssh_keys::test_key_handler)),
    );
}
