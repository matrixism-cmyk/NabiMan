//! Each account's SFTP column layout, per server (scope), kept so the file
//! panel looks the same on every device the owner signs in from.
//!
//! The layout is the page's own JSON — which columns show, in what order, how
//! wide, sorted by what — stored opaque here with size caps, so adding a
//! column later needs no backend change. Share-link visitors have no account
//! and keep their choice in the browser instead.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::Value;

use crate::models::ApiResponse;

/// user id -> { scope -> layout }.
type Map = HashMap<String, HashMap<String, Value>>;

const MAX_USERS: usize = 8192;
const MAX_SCOPES: usize = 400;
/// One layout, serialized.
const MAX_LAYOUT: usize = 8 * 1024;

fn file() -> PathBuf {
    nabiman_server::data_dir().join("sftp_columns.json")
}

fn load(file: &Path) -> Map {
    let Ok(bytes) = std::fs::read(file) else { return Map::new() };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        eprintln!("[sftp] {} is unreadable ({e}); column layouts start empty", file.display());
        Map::new()
    })
}

fn store() -> &'static Mutex<Map> {
    static STORE: OnceLock<Mutex<Map>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(load(&file())))
}

/// A scope names one server (`srv_…`), this machine (`local`), or similar.
fn valid_scope(scope: &str) -> bool {
    (1..=128).contains(&scope.len())
        && scope.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn save(map: &Map, file: &Path) -> Result<(), String> {
    let json = serde_json::to_vec(map).map_err(|e| e.to_string())?;
    crate::private_storage::write(file, &json).map_err(|e| format!("Cannot save column layout: {e}"))
}

fn user(req: &HttpRequest) -> Result<String, HttpResponse> {
    crate::auth::verified_user_id(req)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Sign in again")))
}

/// GET: the signed-in account's layouts, `{ scope: layout }`.
async fn get(req: HttpRequest) -> HttpResponse {
    let id = match user(&req) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let map = store().lock().unwrap_or_else(|e| e.into_inner());
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(map.get(&id).cloned().unwrap_or_default()))
}

#[derive(Deserialize)]
struct Put {
    scope: String,
    /// `null` clears this scope's layout, so it falls back to the default.
    #[serde(default)]
    layout: Value,
}

/// PUT: save (or clear) one scope's layout for the signed-in account.
async fn put(req: HttpRequest, body: web::Json<Put>) -> HttpResponse {
    let id = match user(&req) {
        Ok(id) => id,
        Err(r) => return r,
    };
    if !valid_scope(&body.scope) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid scope"));
    }
    let clearing = body.layout.is_null();
    if !clearing
        && (!body.layout.is_object()
            || serde_json::to_vec(&body.layout).map(|b| b.len()).unwrap_or(usize::MAX) > MAX_LAYOUT)
    {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Column layout too large"));
    }
    let mut map = store().lock().unwrap_or_else(|e| e.into_inner());
    {
        let scopes = map.entry(id.clone()).or_default();
        if clearing {
            scopes.remove(&body.scope);
        } else {
            if !scopes.contains_key(&body.scope) && scopes.len() >= MAX_SCOPES {
                return HttpResponse::BadRequest()
                    .json(ApiResponse::<()>::error("Too many saved column layouts"));
            }
            scopes.insert(body.scope.clone(), body.layout.clone());
        }
        if scopes.is_empty() {
            map.remove(&id);
        }
    }
    if map.len() > MAX_USERS {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Storage limit reached"));
    }
    if let Err(e) = save(&map, &file()) {
        return HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(&e));
    }
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok("saved"))
}

pub(crate) fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/sftp/columns", web::get().to(get))
        .route("/api/terminal/sftp/columns", web::put().to(put));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_must_be_a_plain_identifier() {
        assert!(valid_scope("srv_6aa91840_1") && valid_scope("local"));
        assert!(!valid_scope("") && !valid_scope("../etc") && !valid_scope("a b"));
        assert!(!valid_scope(&"x".repeat(129)));
    }
}
