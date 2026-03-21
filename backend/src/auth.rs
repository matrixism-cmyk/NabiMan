use actix_web::{web, post, HttpResponse};
use actix_web::dev::ServiceRequest;
use serde::Deserialize;
use sha2::{Sha256, Digest};
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use crate::models::ApiResponse;

pub type TokenStore = Arc<Mutex<HashSet<String>>>;

pub fn new_token_store() -> TokenStore {
    Arc::new(Mutex::new(HashSet::new()))
}

fn get_admin_password() -> String {
    std::env::var("NABIMAN_PASSWORD").unwrap_or_else(|_| "nabiman".to_string())
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
) -> HttpResponse {
    if body.password != get_admin_password() {
        return HttpResponse::Unauthorized()
            .json(ApiResponse::<()>::error("Invalid password"));
    }

    let token = generate_token(&body.password);
    store.lock().unwrap().insert(token.clone());

    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({ "token": token })))
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

    if path == "/api/auth/login" || !path.starts_with("/api/") {
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
    cfg.service(login).service(logout);
}
