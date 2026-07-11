use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};

mod config;
mod ldif;

use config::{load_config, save_config, LdapConfig, LdapConfigMasked};
use ldif::{parse_dn_from_ldif, parse_ldap_host_port, parse_member_of};

#[derive(Serialize)]
struct TestResult {
    pub success: bool,
    pub message: String,
}

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
