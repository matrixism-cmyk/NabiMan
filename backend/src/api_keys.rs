//! Bounded, owner-bound automation tokens. Legacy keys never gain new authority.
mod handlers;
mod storage;
#[cfg(test)]
mod tests;
use crate::models::NabimanUser;
pub use handlers::config;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
pub use storage::Store;
pub type ApiKeyStore = Arc<Mutex<Store>>;
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
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub owner_id: String,
    #[serde(default)]
    pub security_stamp: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}
pub const SCOPES: &[&str] = &["status:read", "servers:read", "sessions:read", "tenants:read"];
pub fn new_api_key_store() -> ApiKeyStore {
    Arc::new(Mutex::new(
        Store::open(nabiman_server::data_dir().join("api_keys.json"))
            .expect("Cannot load API token storage; inspect private storage"),
    ))
}
fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
fn stamp(user: &NabimanUser) -> String {
    // Changes in credentials, role or memberships require deliberate reissuance.
    hash(
        &serde_json::to_string(&(
            &user.id,
            &user.password_hash,
            user.totp_enabled,
            &user.totp_secret,
            &user.role,
            &user.memberships,
            user.scope_restricted,
            &user.share_authorization_version,
        ))
        .unwrap(),
    )
}
fn allows(scopes: &[String], method: &str, path: &str) -> bool {
    if method != "GET" {
        return false;
    }
    let scope = match path {
        "/api/server/status" => "status:read",
        "/api/remote-servers" => "servers:read",
        "/api/terminal/sessions" => "sessions:read",
        "/api/mec/v1/tenants" => "tenants:read",
        _ => {
            let Some(rest) = path.strip_prefix("/api/mec/v1/tenants/") else {
                return false;
            };
            let parts: Vec<_> = rest.split('/').collect();
            if parts[0].is_empty()
                || !parts[0]
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || !(parts.len() == 1
                    || (parts.len() == 2 && matches!(parts[1], "resources" | "quota-usage")))
            {
                return false;
            }
            "tenants:read"
        }
    };
    scopes.iter().any(|s| s == scope)
}
#[derive(Clone)]
pub struct TokenActor {
    pub id: String,
    pub user: NabimanUser,
    pub expires: usize,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rejection {
    Invalid,
    Forbidden,
    Storage,
}
pub fn authenticate(
    raw: &str,
    store: &ApiKeyStore,
    users: &crate::users::UserStore,
    method: &str,
    path: &str,
) -> Result<TokenActor, Rejection> {
    if std::env::var("NABIMAN_API_TOKENS_DISABLED").as_deref() == Ok("1") {
        return Err(Rejection::Invalid);
    }
    if raw.len() != 68
        || !raw.starts_with("nbm_")
        || !raw[4..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Rejection::Invalid);
    }
    let mut state = store.lock().map_err(|_| Rejection::Storage)?;
    if state.failed {
        return Err(Rejection::Storage);
    }
    let digest = hash(raw);
    let index = state
        .keys
        .iter()
        .position(|k| k.key_hash == digest)
        .ok_or(Rejection::Invalid)?;
    let key = &state.keys[index];
    let expires = key
        .expires_at
        .as_deref()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .ok_or(Rejection::Invalid)?;
    if key.version != 1 || expires <= chrono::Utc::now() {
        return Err(Rejection::Invalid);
    }
    let user = users
        .lock()
        .map_err(|_| Rejection::Storage)?
        .iter()
        .find(|u| u.id == key.owner_id && u.active())
        .cloned()
        .ok_or(Rejection::Invalid)?;
    if key.security_stamp != stamp(&user) {
        return Err(Rejection::Invalid);
    }
    if !allows(&key.scopes, method, path) || !crate::scope_access::permission(&user, method, path) {
        return Err(Rejection::Forbidden);
    }
    let actor = TokenActor {
        id: key.id.clone(),
        user,
        expires: expires.timestamp() as usize,
    };
    let mut changed = state.keys.clone();
    changed[index].last_used = Some(chrono::Utc::now().to_rfc3339());
    state.commit(changed).map_err(|_| Rejection::Storage)?;
    Ok(actor)
}
