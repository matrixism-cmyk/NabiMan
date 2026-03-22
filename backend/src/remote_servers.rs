use actix_web::{web, HttpResponse};
use std::fs;
use std::process::Command;
use crate::models::{
    ApiResponse, RemoteServer, RemoteServerStatus,
    AddRemoteServerRequest, UpdateRemoteServerRequest, RemoteExecRequest,
};
use crate::ssh_keys;

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

    let port_str = server.port.to_string();
    let output = Command::new("ssh")
        .args(["-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=5",
               "-o", "BatchMode=yes", "-o", "ServerAliveInterval=5",
               "-p", &port_str, &format!("{}@{}", server.user, server.host), remote_cmd])
        .output();

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
            let reason = if stderr.contains("Connection refused") { "Connection refused" }
                else if stderr.contains("timed out") { "Connection timed out" }
                else if stderr.contains("Permission denied") { "Auth failed (Permission denied)" }
                else if stderr.contains("No route to host") { "No route to host" }
                else if stderr.contains("Host key verification") { "Host key verification failed" }
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
    HttpResponse::Ok().json(ApiResponse::ok(load_servers()))
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
        id: generate_id(), name: body.name.clone(), host: body.host.clone(),
        port, user: user.to_string(),
        auth_method: body.auth_method.as_deref().unwrap_or("key").to_string(),
        tags: body.tags.clone().unwrap_or_default(),
        memo: body.memo.clone().unwrap_or_default(),
        created_at: now_iso(), last_checked: String::new(), status: "unknown".into(),
    };

    let mut servers = load_servers();
    servers.push(server.clone());
    if let Err(e) = save_servers(&servers) {
        return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error(&e));
    }
    HttpResponse::Ok().json(ApiResponse::ok(server))
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
    if let Some(port) = body.port { srv.port = port; }
    if let Some(ref user) = body.user {
        if !validate_user(user) { return HttpResponse::Ok().json(ApiResponse::<RemoteServer>::error("Invalid username")); }
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
    let srv = match servers.iter().find(|s| s.id == server_id) {
        Some(s) => s.clone(),
        None => return HttpResponse::Ok().json(ApiResponse::<RemoteServerStatus>::error("Server not found")),
    };
    let result = ssh_probe(&srv);
    if let Some(s) = servers.iter_mut().find(|s| s.id == server_id) {
        s.status = result.status.clone();
        s.last_checked = result.checked_at.clone();
    }
    let _ = save_servers(&servers);
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
    let _ = save_servers(&servers);
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

    let port_str = srv.port.to_string();
    let output = Command::new("ssh")
        .args(["-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=10",
               "-o", "BatchMode=yes", "-p", &port_str,
               &format!("{}@{}", srv.user, srv.host), cmd])
        .output();

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
