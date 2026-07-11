use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()))
}

fn api_keys_path() -> PathBuf {
    data_dir().join("api_keys.json")
}

// --- Models ---

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key_hash: String,
    pub prefix: String,
    pub role: String,
    pub created_at: String,
    #[serde(default)]
    pub last_used: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ApiKeyCreateRequest {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub expires_in_days: Option<u64>,
}

#[derive(Serialize)]
pub struct ApiKeyCreateResponse {
    pub id: String,
    pub name: String,
    pub key: String,
    pub prefix: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct ApiKeyListItem {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub role: String,
    pub created_at: String,
    pub last_used: Option<String>,
    pub expires_at: Option<String>,
}

pub type ApiKeyStore = Arc<Mutex<Vec<ApiKey>>>;

// --- Storage ---

fn load_keys() -> Vec<ApiKey> {
    let path = api_keys_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn save_keys(keys: &[ApiKey]) {
    let path = api_keys_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::to_string_pretty(keys).unwrap_or_default());
}

pub fn new_api_key_store() -> ApiKeyStore {
    Arc::new(Mutex::new(load_keys()))
}

// --- Helpers ---

fn generate_raw_key() -> String {
    use rand::Rng;
    let chars: Vec<char> = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(40)
        .map(char::from)
        .collect();
    format!("nbm_{}", chars.iter().collect::<String>())
}

fn hash_key(key: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

fn generate_id() -> String {
    use rand::Rng;
    let id: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    format!("ak_{}", id)
}

#[allow(dead_code)]
pub fn validate_api_key(key: &str, store: &ApiKeyStore) -> Option<(String, String)> {
    if !key.starts_with("nbm_") {
        return None;
    }
    let key_hash = hash_key(key);
    let now = chrono::Utc::now();
    let mut keys = store.lock().ok()?;
    let found = keys.iter_mut().find(|k| k.key_hash == key_hash)?;

    // Check expiration
    if let Some(ref expires) = found.expires_at {
        if let Ok(exp) = chrono::NaiveDateTime::parse_from_str(expires, "%Y-%m-%d %H:%M:%S") {
            if exp.and_utc() < now {
                return None;
            }
        }
    }

    found.last_used = Some(now.format("%Y-%m-%d %H:%M:%S").to_string());
    let name = found.name.clone();
    let role = found.role.clone();
    drop(keys);
    Some((name, role))
}

// --- Handlers ---

async fn list_keys(store: web::Data<ApiKeyStore>) -> HttpResponse {
    let keys = store.lock().unwrap();
    let items: Vec<ApiKeyListItem> = keys.iter().map(|k| ApiKeyListItem {
        id: k.id.clone(),
        name: k.name.clone(),
        prefix: format!("{}...", k.prefix),
        role: k.role.clone(),
        created_at: k.created_at.clone(),
        last_used: k.last_used.clone(),
        expires_at: k.expires_at.clone(),
    }).collect();
    HttpResponse::Ok().json(ApiResponse::ok(items))
}

async fn create_key(
    body: web::Json<ApiKeyCreateRequest>,
    store: web::Data<ApiKeyStore>,
) -> HttpResponse {
    if body.name.is_empty() || body.name.len() > 100 {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Name must be 1-100 characters"));
    }
    let valid_roles = ["admin", "operator", "viewer"];
    if !valid_roles.contains(&body.role.as_str()) {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Role must be admin, operator, or viewer"));
    }

    let raw_key = generate_raw_key();
    let key_hash = hash_key(&raw_key);
    let prefix = raw_key[..8].to_string(); // "nbm_" + first 4 random chars
    let id = generate_id();
    let now = chrono::Utc::now();

    let expires_at = body.expires_in_days.map(|days| {
        (now + chrono::Duration::days(days as i64)).format("%Y-%m-%d %H:%M:%S").to_string()
    });

    let api_key = ApiKey {
        id: id.clone(),
        name: body.name.clone(),
        key_hash,
        prefix: prefix.clone(),
        role: body.role.clone(),
        created_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        last_used: None,
        expires_at,
    };

    let mut keys = store.lock().unwrap();
    keys.push(api_key);
    save_keys(&keys);

    HttpResponse::Ok().json(ApiResponse::ok(ApiKeyCreateResponse {
        id,
        name: body.name.clone(),
        key: raw_key,
        prefix,
        role: body.role.clone(),
    }))
}

async fn delete_key(
    path: web::Path<String>,
    store: web::Data<ApiKeyStore>,
) -> HttpResponse {
    let id = path.into_inner();
    let mut keys = store.lock().unwrap();
    let before = keys.len();
    keys.retain(|k| k.id != id);
    if keys.len() == before {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("API key not found"));
    }
    save_keys(&keys);
    HttpResponse::Ok().json(ApiResponse::ok("API key revoked"))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/apikeys")
            .route("", web::get().to(list_keys))
            .route("", web::post().to(create_key))
            .route("/{id}", web::delete().to(delete_key))
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_raw_key_format() {
        let key = generate_raw_key();
        assert!(key.starts_with("nbm_"));
        assert_eq!(key.len(), 44); // "nbm_" (4) + 40 chars
    }

    #[test]
    fn test_hash_key_deterministic() {
        let key = "nbm_testkey12345";
        assert_eq!(hash_key(key), hash_key(key));
    }

    #[test]
    fn test_validate_rejects_non_nbm_prefix() {
        let store = new_api_key_store();
        assert!(validate_api_key("invalid_key", &store).is_none());
    }

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        assert!(id.starts_with("ak_"));
        assert_eq!(id.len(), 15); // "ak_" (3) + 12 chars
    }
}
