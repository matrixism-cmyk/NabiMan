use actix_web::{web, post, HttpResponse};
use actix_web::dev::ServiceRequest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::models::ApiResponse;
use crate::users::UserStore;
use crate::jwt_sessions::SessionStore;

pub type PasswordStore = Arc<std::sync::Mutex<String>>; // legacy, kept for migration
pub type JwtSecret = Arc<String>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,   // username
    pub role: String,  // admin/operator/viewer
    pub exp: usize,
    pub iat: usize,
}

pub fn new_password_store() -> PasswordStore {
    let raw = load_password_from_file()
        .unwrap_or_else(|| std::env::var("NABIMAN_PASSWORD").unwrap_or_else(|_| "nabiman".into()));
    let hash = if raw.starts_with("$2b$") || raw.starts_with("$2a$") {
        raw
    } else {
        let h = bcrypt::hash(&raw, 10).expect("bcrypt hash failed");
        let _ = save_hash_to_file(&h);
        println!("Password auto-migrated to bcrypt");
        h
    };
    Arc::new(std::sync::Mutex::new(hash))
}

pub fn new_jwt_secret() -> JwtSecret {
    let path = data_dir().join("jwt_secret");
    if let Ok(s) = std::fs::read_to_string(&path) {
        let s = s.trim().to_string();
        if !s.is_empty() { return Arc::new(s); }
    }
    use rand::Rng;
    let secret: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64).map(char::from).collect();
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let _ = std::fs::write(&path, &secret);
    println!("JWT secret generated");
    Arc::new(secret)
}

fn data_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into())
    )
}
fn password_file_path() -> std::path::PathBuf { data_dir().join("password") }
fn load_password_from_file() -> Option<String> {
    std::fs::read_to_string(password_file_path()).ok()
        .map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}
fn save_hash_to_file(hash: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = password_file_path();
    if let Some(p) = path.parent() { let _ = std::fs::create_dir_all(p); }
    std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600)
        .open(&path).and_then(|mut f| { use std::io::Write; f.write_all(hash.as_bytes()) })
        .map_err(|e| e.to_string())
}

fn create_jwt(secret: &str, username: &str, role: &str) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims { sub: username.into(), role: role.into(), iat: now, exp: now + 86400 };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    ).map_err(|e| e.to_string())
}

fn verify_jwt(secret: &str, token: &str) -> Option<Claims> {
    jsonwebtoken::decode::<Claims>(
        token, &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    ).ok().map(|d| d.claims)
}

pub fn decode_claims_unverified(token: &str) -> Result<jsonwebtoken::TokenData<Claims>, String> {
    let mut validation = jsonwebtoken::Validation::default();
    validation.insecure_disable_signature_validation();
    jsonwebtoken::decode::<Claims>(
        token, &jsonwebtoken::DecodingKey::from_secret(b""), &validation,
    ).map_err(|e| e.to_string())
}

pub fn check_auth_token(secret: &str, token: &str) -> bool {
    verify_jwt(secret, token).is_some()
}

// --- Handlers ---
#[post("/api/auth/login")]
pub async fn login(
    req: actix_web::HttpRequest,
    body: web::Json<crate::models::LoginRequest>,
    user_store: web::Data<UserStore>,
    secret: web::Data<JwtSecret>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let username = body.username.as_deref().unwrap_or("admin");
    let user = match crate::users::find_user_by_name(&user_store, username) {
        Some(u) => u,
        None => return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Invalid credentials")),
    };
    match bcrypt::verify(&body.password, &user.password_hash) {
        Ok(true) => {
            // Check if 2FA is enabled
            if user.totp_enabled {
                return HttpResponse::Ok().json(ApiResponse::ok(
                    serde_json::json!({ "requires_2fa": true, "username": user.username })
                ));
            }
            crate::users::update_last_login(&user_store, username);
            let sid = crate::jwt_sessions::generate_session_id();
            let ip = req.peer_addr().map(|a| a.ip().to_string()).unwrap_or_default();
            crate::jwt_sessions::register_session(&sessions, &sid, username, &ip);
            match crate::jwt_sessions::create_token_pair(&secret, &user.username, user.role.as_str(), &sid) {
                Ok((access, refresh)) => HttpResponse::Ok().json(ApiResponse::ok(
                    serde_json::json!({ "token": access, "refresh_token": refresh, "username": user.username, "role": user.role })
                )),
                Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
            }
        }
        _ => HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Invalid credentials")),
    }
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest { pub current_password: String, pub new_password: String }

#[post("/api/auth/change-password")]
pub async fn change_password(
    req: actix_web::HttpRequest,
    body: web::Json<ChangePasswordRequest>,
    user_store: web::Data<UserStore>,
) -> HttpResponse {
    let username = extract_username(&req);
    let user = match crate::users::find_user_by_name(&user_store, &username) {
        Some(u) => u, None => return HttpResponse::Ok().json(ApiResponse::<()>::error("User not found")),
    };
    if !bcrypt::verify(&body.current_password, &user.password_hash).unwrap_or(false) {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Current password is incorrect"));
    }
    if body.new_password.len() < 8 {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Password must be at least 8 characters"));
    }
    let new_hash = match bcrypt::hash(&body.new_password, 10) {
        Ok(h) => h, Err(e) => return HttpResponse::Ok().json(ApiResponse::<()>::error(&e.to_string())),
    };
    let mut users = user_store.lock().unwrap();
    if let Some(u) = users.iter_mut().find(|u| u.username == username) {
        u.password_hash = new_hash;
    }
    HttpResponse::Ok().json(ApiResponse::ok("Password changed"))
}

#[post("/api/auth/logout")]
pub async fn logout() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok("Logged out"))
}

fn extract_username(req: &actix_web::HttpRequest) -> String {
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                if let Ok(data) = decode_claims_unverified(token) {
                    return data.claims.sub;
                }
            }
        }
    }
    "admin".into()
}

// --- Middleware auth check (JWT + RBAC) ---
pub fn check_auth(req: &ServiceRequest, secret: &JwtSecret) -> bool {
    let path = req.path();
    if path == "/api/auth/login" || path == "/api/auth/refresh" || path == "/api/auth/2fa/verify"
        || path == "/api/auth/oauth/callback" || path == "/api/auth/ldap/login"
        || !path.starts_with("/api/") {
        return true;
    }

    let token = extract_token(req);
    let token = match token {
        Some(t) => t, None => return false,
    };
    let claims = match verify_jwt(secret, &token) {
        Some(c) => c, None => return false,
    };
    let role = crate::rbac::role_from_str(&claims.role);
    crate::rbac::check_permission(&role, req.method().as_str(), path)
}

fn extract_token(req: &ServiceRequest) -> Option<String> {
    // Terminal: query param
    if req.path() == "/api/terminal" {
        if let Some(q) = req.uri().query() {
            for pair in q.split('&') {
                if let Some(tok) = pair.strip_prefix("token=") {
                    return Some(tok.to_string());
                }
            }
        }
    }
    // Authorization: Bearer
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(tok) = val.strip_prefix("Bearer ") {
                return Some(tok.trim().to_string());
            }
        }
    }
    // Legacy X-Auth-Token
    req.headers().get("X-Auth-Token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(login).service(logout).service(change_password);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_create_and_verify() {
        let secret = "test_secret_key_for_jwt_12345678";
        let token = create_jwt(secret, "admin", "admin").unwrap();
        assert!(!token.is_empty());
        let claims = verify_jwt(secret, &token).unwrap();
        assert_eq!(claims.sub, "admin");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_jwt_expired_rejected() {
        let secret = "test_secret";
        // Create a token that's already expired
        let now = chrono::Utc::now().timestamp() as usize;
        let claims = Claims { sub: "u".into(), role: "admin".into(), iat: now - 200, exp: now - 100 };
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
        ).unwrap();
        assert!(verify_jwt(secret, &token).is_none());
    }

    #[test]
    fn test_jwt_wrong_secret_rejected() {
        let token = create_jwt("secret1", "admin", "admin").unwrap();
        assert!(verify_jwt("secret2", &token).is_none());
    }

    #[test]
    fn test_check_auth_token() {
        let secret = "test123";
        let token = create_jwt(secret, "admin", "admin").unwrap();
        assert!(check_auth_token(secret, &token));
        assert!(!check_auth_token(secret, "garbage"));
    }

    #[test]
    fn test_decode_claims_unverified() {
        let token = create_jwt("any_secret", "testuser", "viewer").unwrap();
        let data = decode_claims_unverified(&token).unwrap();
        assert_eq!(data.claims.sub, "testuser");
        assert_eq!(data.claims.role, "viewer");
    }
}
