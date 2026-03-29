use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()))
}

fn ldap_config_path() -> PathBuf {
    data_dir().join("ldap_config.json")
}

// --- Models ---

#[derive(Serialize, Deserialize, Clone)]
pub struct LdapConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub bind_dn: String,
    #[serde(default)]
    pub bind_password: String,
    #[serde(default)]
    pub search_base: String,
    #[serde(default = "default_user_filter")]
    pub user_filter: String,
    #[serde(default)]
    pub tls: bool,
    #[serde(default)]
    pub admin_group: String,
    #[serde(default)]
    pub operator_group: String,
}

fn default_user_filter() -> String {
    "(uid={username})".to_string()
}

impl Default for LdapConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: "ldap://localhost:389".to_string(),
            bind_dn: String::new(),
            bind_password: String::new(),
            search_base: "dc=example,dc=com".to_string(),
            user_filter: default_user_filter(),
            tls: false,
            admin_group: "cn=admins,ou=groups,dc=example,dc=com".to_string(),
            operator_group: "cn=operators,ou=groups,dc=example,dc=com".to_string(),
        }
    }
}

#[derive(Serialize)]
struct LdapConfigMasked {
    pub enabled: bool,
    pub server_url: String,
    pub bind_dn: String,
    pub bind_password: String,
    pub search_base: String,
    pub user_filter: String,
    pub tls: bool,
    pub admin_group: String,
    pub operator_group: String,
}

impl From<&LdapConfig> for LdapConfigMasked {
    fn from(cfg: &LdapConfig) -> Self {
        Self {
            enabled: cfg.enabled,
            server_url: cfg.server_url.clone(),
            bind_dn: cfg.bind_dn.clone(),
            bind_password: if cfg.bind_password.is_empty() { String::new() } else { "***".into() },
            search_base: cfg.search_base.clone(),
            user_filter: cfg.user_filter.clone(),
            tls: cfg.tls,
            admin_group: cfg.admin_group.clone(),
            operator_group: cfg.operator_group.clone(),
        }
    }
}

#[derive(Serialize)]
struct TestResult {
    pub success: bool,
    pub message: String,
}

// --- Storage ---

fn load_config() -> LdapConfig {
    let path = ldap_config_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        LdapConfig::default()
    }
}

fn save_config(cfg: &LdapConfig) {
    let path = ldap_config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::to_string_pretty(cfg).unwrap_or_default());
}

// --- URL parsing helper ---

fn parse_ldap_host_port(server_url: &str) -> Option<(String, u16)> {
    // Formats: ldap://host:port, ldaps://host:port, host:port
    let stripped = server_url
        .trim_start_matches("ldaps://")
        .trim_start_matches("ldap://");
    let is_ssl = server_url.starts_with("ldaps://");
    let default_port: u16 = if is_ssl { 636 } else { 389 };

    let (host, port) = if let Some(idx) = stripped.rfind(':') {
        let h = &stripped[..idx];
        let p = stripped[idx + 1..].trim_end_matches('/').parse::<u16>().unwrap_or(default_port);
        (h.to_string(), p)
    } else {
        (stripped.trim_end_matches('/').to_string(), default_port)
    };

    if host.is_empty() {
        None
    } else {
        Some((host, port))
    }
}

// --- Handlers ---

async fn get_config() -> HttpResponse {
    let cfg = load_config();
    let masked = LdapConfigMasked::from(&cfg);
    HttpResponse::Ok().json(ApiResponse::ok(masked))
}

async fn save_ldap_config(body: web::Json<LdapConfig>) -> HttpResponse {
    let mut cfg = body.into_inner();

    // If password is masked, preserve existing password
    if cfg.bind_password == "***" {
        let existing = load_config();
        cfg.bind_password = existing.bind_password;
    }

    if cfg.server_url.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Server URL is required"));
    }

    save_config(&cfg);
    let masked = LdapConfigMasked::from(&cfg);
    HttpResponse::Ok().json(ApiResponse::ok(masked))
}

async fn test_connection() -> HttpResponse {
    let cfg = load_config();

    if cfg.server_url.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::ok(TestResult {
            success: false,
            message: "LDAP server URL is not configured".into(),
        }));
    }

    let (host, port) = match parse_ldap_host_port(&cfg.server_url) {
        Some(hp) => hp,
        None => {
            return HttpResponse::Ok().json(ApiResponse::ok(TestResult {
                success: false,
                message: format!("Cannot parse server URL: {}", cfg.server_url),
            }));
        }
    };

    // Resolve hostname and attempt TCP connection
    let addr = format!("{}:{}", host, port);
    let timeout = std::time::Duration::from_secs(5);

    let result = std::net::ToSocketAddrs::to_socket_addrs(&addr.as_str())
        .map_err(|e| format!("DNS resolution failed for {}: {}", host, e))
        .and_then(|mut addrs| {
            addrs.next().ok_or_else(|| format!("No addresses found for {}", host))
        })
        .and_then(|socket_addr| {
            std::net::TcpStream::connect_timeout(&socket_addr, timeout)
                .map_err(|e| format!("Connection to {} failed: {}", addr, e))
        });

    let test_result = match result {
        Ok(_stream) => TestResult {
            success: true,
            message: format!("Successfully connected to {} (port {})", host, port),
        },
        Err(msg) => TestResult {
            success: false,
            message: msg,
        },
    };

    HttpResponse::Ok().json(ApiResponse::ok(test_result))
}

// --- LDAP Authentication ---

#[derive(Deserialize)]
struct LdapLoginRequest {
    username: String,
    password: String,
}

#[derive(Debug)]
pub struct LdapAuthResult {
    pub username: String,
    pub role: String,
}

/// Authenticate user via LDAP bind using ldapsearch CLI
pub fn ldap_authenticate(username: &str, password: &str) -> Result<LdapAuthResult, String> {
    // Validate inputs to prevent injection (before any config check)
    if username.contains('\0') || username.contains('*') || username.contains('(') || username.contains(')') {
        return Err("Invalid username characters".into());
    }
    if password.is_empty() {
        return Err("Password is required".into());
    }

    let cfg = load_config();
    if !cfg.enabled {
        return Err("LDAP is not enabled".into());
    }

    let (host, port) = parse_ldap_host_port(&cfg.server_url)
        .ok_or("Cannot parse LDAP server URL")?;

    // Step 1: Search for user DN using service account
    let user_filter = cfg.user_filter.replace("{username}", username);
    let ldap_uri = if cfg.tls || cfg.server_url.starts_with("ldaps://") {
        format!("ldaps://{}:{}", host, port)
    } else {
        format!("ldap://{}:{}", host, port)
    };

    let mut search_args = vec![
        "-x".to_string(), "-H".to_string(), ldap_uri.clone(),
        "-b".to_string(), cfg.search_base.clone(),
        "-LLL".to_string(),
    ];

    if !cfg.bind_dn.is_empty() {
        search_args.extend(["-D".to_string(), cfg.bind_dn.clone()]);
        search_args.extend(["-w".to_string(), cfg.bind_password.clone()]);
    }

    search_args.push(user_filter);
    search_args.push("dn".to_string());
    search_args.push("memberOf".to_string());

    let search_output = std::process::Command::new("ldapsearch")
        .args(&search_args)
        .output()
        .map_err(|e| format!("ldapsearch not available: {}", e))?;

    let stdout = String::from_utf8_lossy(&search_output.stdout);
    let user_dn = parse_dn_from_ldif(&stdout)
        .ok_or_else(|| "User not found in LDAP".to_string())?;

    // Step 2: Bind as the user to verify password
    let bind_output = std::process::Command::new("ldapsearch")
        .args([
            "-x", "-H", &ldap_uri,
            "-D", &user_dn,
            "-w", password,
            "-b", &user_dn,
            "-s", "base",
            "-LLL",
            "(objectClass=*)",
            "dn",
        ])
        .output()
        .map_err(|e| format!("ldapsearch bind failed: {}", e))?;

    if !bind_output.status.success() {
        let stderr = String::from_utf8_lossy(&bind_output.stderr);
        if stderr.contains("Invalid credentials") || stderr.contains("49") {
            return Err("Invalid LDAP credentials".into());
        }
        return Err(format!("LDAP bind failed: {}", stderr.trim()));
    }

    // Step 3: Determine role from group membership
    let member_of_groups = parse_member_of(&stdout);
    let role = if member_of_groups.iter().any(|g| g.contains(&cfg.admin_group) || cfg.admin_group.contains(g)) {
        "admin"
    } else if member_of_groups.iter().any(|g| g.contains(&cfg.operator_group) || cfg.operator_group.contains(g)) {
        "operator"
    } else {
        "viewer"
    };

    Ok(LdapAuthResult {
        username: username.to_string(),
        role: role.to_string(),
    })
}

fn parse_dn_from_ldif(ldif: &str) -> Option<String> {
    for line in ldif.lines() {
        let trimmed = line.trim();
        if let Some(dn) = trimmed.strip_prefix("dn: ") {
            return Some(dn.to_string());
        }
        if let Some(dn) = trimmed.strip_prefix("dn:: ") {
            // Base64 encoded DN
            if let Ok(decoded) = base64_decode(dn.trim()) {
                return Some(decoded);
            }
        }
    }
    None
}

fn parse_member_of(ldif: &str) -> Vec<String> {
    let mut groups = Vec::new();
    for line in ldif.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("memberOf: ") {
            groups.push(val.to_string());
        }
    }
    groups
}

fn base64_decode(input: &str) -> Result<String, String> {
    // Simple base64 decode for DN
    let output = std::process::Command::new("base64")
        .arg("-d")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(input.as_bytes());
            }
            child.wait_with_output()
        })
        .map_err(|e| e.to_string())?;
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}

async fn ldap_login(
    req: actix_web::HttpRequest,
    body: web::Json<LdapLoginRequest>,
    secret: web::Data<crate::auth::JwtSecret>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    user_store: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let result = match ldap_authenticate(&body.username, &body.password) {
        Ok(r) => r,
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<()>::error(&e)),
    };

    // Auto-provision user
    crate::users::ensure_oauth_user(&user_store, &result.username, &result.role);

    let sid = crate::jwt_sessions::generate_session_id();
    let ip = req.peer_addr().map(|a| a.ip().to_string()).unwrap_or_default();
    crate::jwt_sessions::register_session(&sessions, &sid, &result.username, &ip);

    match crate::jwt_sessions::create_token_pair(&secret, &result.username, &result.role, &sid) {
        Ok((access, refresh)) => HttpResponse::Ok().json(ApiResponse::ok(
            serde_json::json!({
                "token": access,
                "refresh_token": refresh,
                "username": result.username,
                "role": result.role,
                "auth_method": "ldap",
            })
        )),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/auth/ldap")
            .route("/config", web::get().to(get_config))
            .route("/config", web::post().to(save_ldap_config))
            .route("/test", web::post().to(test_connection))
            .route("/login", web::post().to(ldap_login))
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ldap_url_standard() {
        let (host, port) = parse_ldap_host_port("ldap://ldap.example.com:389").unwrap();
        assert_eq!(host, "ldap.example.com");
        assert_eq!(port, 389);
    }

    #[test]
    fn test_parse_ldaps_url_default_port() {
        let (host, port) = parse_ldap_host_port("ldaps://ad.corp.local").unwrap();
        assert_eq!(host, "ad.corp.local");
        assert_eq!(port, 636);
    }

    #[test]
    fn test_parse_ldap_url_custom_port() {
        let (host, port) = parse_ldap_host_port("ldap://10.0.0.5:3268").unwrap();
        assert_eq!(host, "10.0.0.5");
        assert_eq!(port, 3268);
    }

    #[test]
    fn test_parse_empty_url() {
        assert!(parse_ldap_host_port("").is_none());
    }

    #[test]
    fn test_masked_config_hides_password() {
        let cfg = LdapConfig {
            bind_password: "supersecret".into(),
            ..LdapConfig::default()
        };
        let masked = LdapConfigMasked::from(&cfg);
        assert_eq!(masked.bind_password, "***");
    }

    #[test]
    fn test_default_config_values() {
        let cfg = LdapConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.server_url, "ldap://localhost:389");
        assert_eq!(cfg.user_filter, "(uid={username})");
    }

    #[test]
    fn test_parse_dn_from_ldif() {
        let ldif = "dn: uid=john,ou=users,dc=example,dc=com\nuid: john\nmemberOf: cn=admins,ou=groups,dc=example,dc=com\n";
        assert_eq!(parse_dn_from_ldif(ldif), Some("uid=john,ou=users,dc=example,dc=com".into()));
    }

    #[test]
    fn test_parse_dn_empty() {
        assert_eq!(parse_dn_from_ldif(""), None);
        assert_eq!(parse_dn_from_ldif("uid: john\ncn: John"), None);
    }

    #[test]
    fn test_parse_member_of() {
        let ldif = "dn: uid=john,ou=users,dc=ex,dc=com\nmemberOf: cn=admins,ou=groups,dc=ex,dc=com\nmemberOf: cn=devs,ou=groups,dc=ex,dc=com\n";
        let groups = parse_member_of(ldif);
        assert_eq!(groups.len(), 2);
        assert!(groups[0].contains("admins"));
        assert!(groups[1].contains("devs"));
    }

    #[test]
    fn test_ldap_auth_rejects_injection() {
        // ldap_authenticate should reject usernames with special chars
        let result = ldap_authenticate("user*()", "pass");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid username"));
    }

    #[test]
    fn test_ldap_disabled_returns_error() {
        // Default config has enabled=false
        let result = ldap_authenticate("john", "password");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not enabled"));
    }
}
