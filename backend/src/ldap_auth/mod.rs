use crate::models::ApiResponse;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

mod bind;
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
    let cfg = match load_config() {
        Ok(cfg) => cfg,
        Err(error) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error))
        }
    };
    let masked = LdapConfigMasked::from(&cfg);
    HttpResponse::Ok().json(ApiResponse::ok(masked))
}

async fn save_ldap_config(body: web::Json<LdapConfig>) -> HttpResponse {
    let mut cfg = body.into_inner();

    // If password is masked, preserve existing password
    if cfg.bind_password == "***" {
        let existing = match load_config() {
            Ok(existing) => existing,
            Err(error) => {
                return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error))
            }
        };
        cfg.bind_password = existing.bind_password;
    }

    if cfg.server_url.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Server URL is required"));
    }

    if let Err(error) = save_config(&cfg) {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error));
    }
    let masked = LdapConfigMasked::from(&cfg);
    HttpResponse::Ok().json(ApiResponse::ok(masked))
}

async fn test_connection() -> HttpResponse {
    let cfg = match load_config() {
        Ok(cfg) => cfg,
        Err(error) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error))
        }
    };

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
            addrs
                .next()
                .ok_or_else(|| format!("No addresses found for {}", host))
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
    if username.contains('\0')
        || username.contains('*')
        || username.contains('(')
        || username.contains(')')
    {
        return Err("Invalid username characters".into());
    }
    if password.is_empty() {
        return Err("Password is required".into());
    }

    let cfg = load_config()?;
    if !cfg.enabled {
        return Err("LDAP is not enabled".into());
    }

    let (host, port) =
        parse_ldap_host_port(&cfg.server_url).ok_or("Cannot parse LDAP server URL")?;
    let ldap_uri = if cfg.tls || cfg.server_url.starts_with("ldaps://") {
        format!("ldaps://{}:{}", host, port)
    } else {
        format!("ldap://{}:{}", host, port)
    };

    // Step 1: find the user's DN with the service account.
    let stdout = bind::find_user(&cfg, &ldap_uri, username)?;
    let user_dn =
        parse_dn_from_ldif(&stdout).ok_or_else(|| "User not found in LDAP".to_string())?;

    // Step 2: bind as the user to verify the password.
    bind::bind_as(&ldap_uri, &user_dn, password)?;

    // Step 3: the role from group membership (the same DN, not a substring).
    let member_of_groups = parse_member_of(&stdout);
    let role = if bind::member_of(&member_of_groups, &cfg.admin_group) {
        "admin"
    } else if bind::member_of(&member_of_groups, &cfg.operator_group) {
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
    // ldapsearch runs twice: on the blocking pool, each run bounded.
    let (username, password) = (body.username.clone(), body.password.clone());
    let result = match web::block(move || ldap_authenticate(&username, &password)).await {
        Ok(Ok(r)) => r,
        _ => {
            return HttpResponse::Unauthorized()
                .json(ApiResponse::<()>::error("Invalid credentials"))
        }
    };

    // Auto-provision user
    if let Err(error) = crate::users::ensure_oauth_user(&user_store, &result.username, &result.role)
    {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error));
    }

    let user = match crate::users::find_user_by_name(&user_store, &result.username) {
        Some(user) if user.active() => user,
        _ => {
            return HttpResponse::Forbidden().json(ApiResponse::<()>::error(
                "최고관리자의 가입 승인이 필요합니다",
            ))
        }
    };
    let role = user.role.as_str().to_string();
    let sid = crate::jwt_sessions::generate_session_id();
    let ip = crate::access_rules::client_ip(&req)
        .map(|a| a.to_string())
        .unwrap_or_default();
    if !crate::jwt_sessions::register_authenticated_session(
        &sessions,
        &user_store,
        &sid,
        &user,
        false,
        &ip,
    ) {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error(
            "Account changed during authentication; sign in again",
        ));
    }

    match crate::jwt_sessions::create_token_pair(&secret, &result.username, &role, &sid) {
        Ok((access, refresh)) => HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
            "token": access,
            "refresh_token": refresh,
            "username": result.username,
            "role": role,
            "auth_method": "ldap",
        }))),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

async fn provider() -> HttpResponse {
    match load_config() {
        Ok(cfg) => {
            HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({"enabled": cfg.enabled})))
        }
        Err(_) => HttpResponse::ServiceUnavailable()
            .json(ApiResponse::<()>::error("LDAP configuration unavailable")),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/auth/ldap")
            .route("/provider", web::get().to(provider))
            .route("/config", web::get().to(get_config))
            .route("/config", web::post().to(save_ldap_config))
            .route("/test", web::post().to(test_connection))
            .route("/login", web::post().to(ldap_login)),
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
