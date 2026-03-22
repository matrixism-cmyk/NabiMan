use actix_web::{web, post, HttpResponse};
use actix_web::dev::ServiceRequest;
use serde::Deserialize;
use sha2::{Sha256, Digest};
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use crate::models::ApiResponse;

pub type TokenStore = Arc<Mutex<HashSet<String>>>;
pub type PasswordStore = Arc<Mutex<String>>;

pub fn new_token_store() -> TokenStore {
    Arc::new(Mutex::new(HashSet::new()))
}

pub fn new_password_store() -> PasswordStore {
    let initial = load_password_from_file()
        .unwrap_or_else(|| std::env::var("NABIMAN_PASSWORD").unwrap_or_else(|_| "nabiman".to_string()));
    Arc::new(Mutex::new(initial))
}

fn password_file_path() -> std::path::PathBuf {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".to_string());
    std::path::PathBuf::from(dir).join("password")
}

fn load_password_from_file() -> Option<String> {
    std::fs::read_to_string(password_file_path()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn save_password_to_file(password: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = password_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(password.as_bytes())
        })
        .map_err(|e| e.to_string())
}

fn get_admin_password(store: &PasswordStore) -> String {
    store.lock().unwrap().clone()
}

fn generate_token(password: &str) -> String {
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let input = format!("{}:{}:{}", password, now, rand_seed());
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn rand_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[post("/api/auth/login")]
pub async fn login(
    body: web::Json<LoginRequest>,
    store: web::Data<TokenStore>,
    pw_store: web::Data<PasswordStore>,
) -> HttpResponse {
    if body.password != get_admin_password(&pw_store) {
        return HttpResponse::Unauthorized()
            .json(ApiResponse::<()>::error("Invalid password"));
    }

    let token = generate_token(&body.password);
    store.lock().unwrap().insert(token.clone());

    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({ "token": token })))
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[post("/api/auth/change-password")]
pub async fn change_password(
    body: web::Json<ChangePasswordRequest>,
    pw_store: web::Data<PasswordStore>,
) -> HttpResponse {
    let current = get_admin_password(&pw_store);
    if body.current_password != current {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Current password is incorrect"));
    }
    if body.new_password.len() < 4 {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Password must be at least 4 characters"));
    }

    match save_password_to_file(&body.new_password) {
        Ok(_) => {
            *pw_store.lock().unwrap() = body.new_password.clone();
            HttpResponse::Ok().json(ApiResponse::ok("Password changed"))
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<()>::error(&format!("Failed to save: {}", e))),
    }
}

#[post("/api/auth/logout")]
pub async fn logout(
    req: actix_web::HttpRequest,
    store: web::Data<TokenStore>,
) -> HttpResponse {
    if let Some(token) = req.headers().get("X-Auth-Token") {
        if let Ok(t) = token.to_str() {
            store.lock().unwrap().remove(t);
        }
    }
    HttpResponse::Ok().json(ApiResponse::ok("Logged out"))
}

pub fn check_auth(req: &ServiceRequest, store: &TokenStore) -> bool {
    let path = req.path();

    // Allow: login, terminal (auth via query param), and non-API paths
    if path == "/api/auth/login" || path == "/api/terminal" || !path.starts_with("/api/") {
        return true;
    }

    if let Some(token) = req.headers().get("X-Auth-Token") {
        if let Ok(t) = token.to_str() {
            return store.lock().unwrap().contains(t);
        }
    }
    false
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(login).service(logout).service(change_password);
}
