use actix_web::{web, HttpRequest, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub timestamp: String,
    pub user: String,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub ip: String,
    #[serde(default)]
    pub hash: Option<String>,
}

pub type AuditLog = Arc<Mutex<Vec<AuditEntry>>>;

pub fn new_audit_log() -> AuditLog {
    let entries = load_from_file();
    Arc::new(Mutex::new(entries))
}

fn audit_file_path() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("audit.jsonl")
}

fn load_from_file() -> Vec<AuditEntry> {
    let path = audit_file_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    content.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn compute_hash(prev_hash: &str, entry: &AuditEntry) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(prev_hash);
    hasher.update(&entry.timestamp);
    hasher.update(&entry.user);
    hasher.update(&entry.method);
    hasher.update(&entry.path);
    hasher.update(entry.status.to_string());
    hasher.update(&entry.ip);
    hex::encode(hasher.finalize())
}

fn get_last_hash(fpath: &std::path::Path) -> String {
    let content = std::fs::read_to_string(fpath).unwrap_or_default();
    content.lines().rev()
        .filter_map(|l| serde_json::from_str::<AuditEntry>(l).ok())
        .find_map(|e| e.hash)
        .unwrap_or_else(|| "0".to_string())
}

// --- External Log Forwarding ---

#[derive(Serialize, Deserialize, Clone)]
pub struct LogForwardConfig {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub forward_type: String,  // "syslog_udp", "syslog_tcp", "http"
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub path: String,          // URL path for HTTP
    #[serde(default)]
    pub auth_header: String,   // Optional auth for HTTP
}

fn forwarding_file() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    std::path::PathBuf::from(dir).join("audit_forwarding.json")
}

fn load_forwarding() -> Vec<LogForwardConfig> {
    std::fs::read_to_string(forwarding_file()).ok()
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_forwarding(configs: &[LogForwardConfig]) {
    let _ = std::fs::write(forwarding_file(), serde_json::to_string_pretty(configs).unwrap_or_default());
}

fn forward_entry(entry: &AuditEntry) {
    let configs = load_forwarding();
    for cfg in configs.iter().filter(|c| c.enabled) {
        match cfg.forward_type.as_str() {
            "syslog_udp" => forward_syslog_udp(cfg, entry),
            "syslog_tcp" => forward_syslog_tcp(cfg, entry),
            "http" => forward_http(cfg, entry),
            _ => {}
        }
    }
}

fn syslog_message(entry: &AuditEntry) -> String {
    // RFC 3164 format
    format!("<134>nabiman: user={} method={} path={} status={} ip={}",
        entry.user, entry.method, entry.path, entry.status, entry.ip)
}

fn forward_syslog_udp(cfg: &LogForwardConfig, entry: &AuditEntry) {
    let msg = syslog_message(entry);
    let addr = format!("{}:{}", cfg.host, cfg.port);
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        let _ = sock.send_to(msg.as_bytes(), &addr);
    }
}

fn forward_syslog_tcp(cfg: &LogForwardConfig, entry: &AuditEntry) {
    let msg = syslog_message(entry);
    let addr = format!("{}:{}", cfg.host, cfg.port);
    if let Ok(mut stream) = std::net::TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| std::net::SocketAddr::from(([127,0,0,1], cfg.port))),
        std::time::Duration::from_secs(3),
    ) {
        use std::io::Write;
        let _ = stream.write_all(format!("{}\n", msg).as_bytes());
    }
}

fn forward_http(cfg: &LogForwardConfig, entry: &AuditEntry) {
    let url = format!("http://{}:{}{}", cfg.host, cfg.port, cfg.path);
    let json = serde_json::to_string(entry).unwrap_or_default();
    let mut args = vec![
        "-s".to_string(), "-X".to_string(), "POST".to_string(),
        "-H".to_string(), "Content-Type: application/json".to_string(),
        "-d".to_string(), json, "--max-time".to_string(), "5".to_string(),
    ];
    if !cfg.auth_header.is_empty() {
        args.extend(["-H".to_string(), format!("Authorization: {}", cfg.auth_header)]);
    }
    args.push(url);
    let _ = std::process::Command::new("curl").args(&args).output();
}

pub fn append_entry(store: &AuditLog, mut entry: AuditEntry) {
    let fpath = audit_file_path();
    if let Some(p) = fpath.parent() { let _ = std::fs::create_dir_all(p); }

    let prev_hash = get_last_hash(&fpath);
    entry.hash = Some(compute_hash(&prev_hash, &entry));

    if let Ok(line) = serde_json::to_string(&entry) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&fpath) {
            let _ = writeln!(f, "{}", line);
        }
        if fpath.metadata().map(|m| m.len() > 10_000_000).unwrap_or(false) {
            rotate_log(&fpath);
        }
    }

    // Forward to external destinations (fire-and-forget)
    let entry_clone = entry.clone();
    std::thread::spawn(move || forward_entry(&entry_clone));

    let mut log = store.lock().unwrap();
    log.push(entry);
    let len = log.len();
    if len > 1000 { log.drain(0..len - 1000); }
}

fn rotate_log(path: &std::path::Path) {
    for i in (1..5).rev() {
        let from = path.with_extension(format!("jsonl.{}", i));
        let to = path.with_extension(format!("jsonl.{}", i + 1));
        let _ = std::fs::rename(&from, &to);
    }
    let backup = path.with_extension("jsonl.1");
    let _ = std::fs::rename(path, &backup);
}

#[derive(Deserialize)]
struct AuditQuery {
    user: Option<String>,
    method: Option<String>,
    path_filter: Option<String>,
    limit: Option<usize>,
}

async fn get_audit_log(store: web::Data<AuditLog>, query: web::Query<AuditQuery>) -> HttpResponse {
    let log = store.lock().unwrap();
    let limit = query.limit.unwrap_or(200).min(500);
    let filtered: Vec<AuditEntry> = log.iter().rev()
        .filter(|e| {
            query.user.as_ref().map_or(true, |u| e.user.contains(u.as_str()))
            && query.method.as_ref().map_or(true, |m| &e.method == m)
            && query.path_filter.as_ref().map_or(true, |p| e.path.contains(p.as_str()))
        })
        .take(limit)
        .cloned()
        .collect();
    HttpResponse::Ok().json(ApiResponse::ok(filtered))
}

fn extract_role_from_request(req: &HttpRequest) -> Option<String> {
    let auth = req.headers().get("Authorization")?.to_str().ok()?;
    let token = auth.strip_prefix("Bearer ")?;
    // Decode JWT payload (base64 middle part) to extract role
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 { return None; }
    let payload = base64_decode(parts[1])?;
    let val: serde_json::Value = serde_json::from_str(&payload).ok()?;
    val.get("role").and_then(|r| r.as_str()).map(|s| s.to_string())
}

fn base64_decode(input: &str) -> Option<String> {
    // Standard base64url decode (JWT uses URL-safe base64 without padding)
    let padded = match input.len() % 4 {
        2 => format!("{}==", input),
        3 => format!("{}=", input),
        _ => input.to_string(),
    };
    let replaced = padded.replace('-', "+").replace('_', "/");
    // Use simple manual decode via a command or built-in
    let decoded = base64_decode_bytes(&replaced)?;
    String::from_utf8(decoded).ok()
}

fn base64_decode_bytes(input: &str) -> Option<Vec<u8>> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::new();
    let mut buf: u32 = 0;
    let mut bits = 0;
    for &b in input.as_bytes() {
        if b == b'=' { break; }
        let val = alphabet.iter().position(|&c| c == b)? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(output)
}

async fn clear_audit_log(store: web::Data<AuditLog>, req: HttpRequest) -> HttpResponse {
    let role = extract_role_from_request(&req).unwrap_or_default();
    if role != "admin" {
        return HttpResponse::Ok().json(
            ApiResponse::<String>::error("Only admin can clear audit log")
        );
    }
    store.lock().unwrap().clear();
    let _ = std::fs::write(audit_file_path(), "");
    HttpResponse::Ok().json(ApiResponse::ok("Audit log cleared"))
}

// --- Hash chain verification ---

#[derive(Serialize)]
struct VerifyResult {
    valid: bool,
    entries_checked: usize,
    error: Option<String>,
}

async fn verify_audit() -> HttpResponse {
    let fpath = audit_file_path();
    let content = std::fs::read_to_string(&fpath).unwrap_or_default();
    let mut prev_hash = "0".to_string();
    let mut count = 0usize;

    for line in content.lines() {
        if line.trim().is_empty() { continue; }
        let entry: AuditEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(e) => {
                return HttpResponse::Ok().json(ApiResponse::ok(VerifyResult {
                    valid: false, entries_checked: count,
                    error: Some(format!("Parse error at entry {}: {}", count, e)),
                }));
            }
        };
        let expected = compute_hash(&prev_hash, &entry);
        let actual = entry.hash.as_deref().unwrap_or("");
        if actual != expected {
            return HttpResponse::Ok().json(ApiResponse::ok(VerifyResult {
                valid: false, entries_checked: count,
                error: Some(format!("Hash mismatch at entry {}", count)),
            }));
        }
        prev_hash = expected;
        count += 1;
    }

    HttpResponse::Ok().json(ApiResponse::ok(VerifyResult {
        valid: true, entries_checked: count, error: None,
    }))
}

fn gen_id() -> String {
    use rand::Rng;
    format!("{:016x}", rand::thread_rng().gen::<u64>())
}

async fn list_forwarding() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(load_forwarding()))
}

async fn add_forwarding(body: web::Json<LogForwardConfig>) -> HttpResponse {
    let mut cfg = body.into_inner();
    cfg.id = gen_id();
    let mut configs = load_forwarding();
    configs.push(cfg);
    save_forwarding(&configs);
    HttpResponse::Ok().json(ApiResponse::ok("Forwarding config added"))
}

async fn delete_forwarding(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    let mut configs = load_forwarding();
    configs.retain(|c| c.id != id);
    save_forwarding(&configs);
    HttpResponse::Ok().json(ApiResponse::ok("Forwarding config deleted"))
}

async fn test_forwarding(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    let configs = load_forwarding();
    let cfg = match configs.iter().find(|c| c.id == id) {
        Some(c) => c,
        None => return HttpResponse::Ok().json(ApiResponse::<()>::error("Config not found")),
    };
    let test_entry = AuditEntry {
        timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        user: "test".into(), method: "GET".into(), path: "/api/audit/test".into(),
        status: 200, ip: "127.0.0.1".into(), hash: None,
    };
    match cfg.forward_type.as_str() {
        "syslog_udp" => forward_syslog_udp(cfg, &test_entry),
        "syslog_tcp" => forward_syslog_tcp(cfg, &test_entry),
        "http" => forward_http(cfg, &test_entry),
        _ => return HttpResponse::Ok().json(ApiResponse::<()>::error("Unknown type")),
    }
    HttpResponse::Ok().json(ApiResponse::ok("Test entry forwarded"))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/audit")
            .route("", web::get().to(get_audit_log))
            .route("/clear", web::post().to(clear_audit_log))
            .route("/verify", web::get().to(verify_audit))
            .route("/forwarding", web::get().to(list_forwarding))
            .route("/forwarding", web::post().to(add_forwarding))
            .route("/forwarding/{id}", web::delete().to(delete_forwarding))
            .route("/forwarding/{id}/test", web::post().to(test_forwarding))
    );
}
