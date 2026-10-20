//! Conversations, one file each, belonging to the account that started them.
//! A conversation remembers Claude's session, so the next question goes on
//! from where it left off (`--resume`).

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::models::ApiResponse;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Message {
    /// `user` or `assistant`.
    pub role: String,
    pub text: String,
    pub at: i64,
    /// The question went unanswered: why.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Chat {
    pub id: String,
    pub owner: String,
    pub title: String,
    /// Claude Code's session, once there is one.
    #[serde(default)]
    pub session_id: String,
    pub created: i64,
    pub updated: i64,
    #[serde(default)]
    pub messages: Vec<Message>,
}

pub(crate) fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn dir() -> std::path::PathBuf {
    super::data_dir().join("claude-voice")
}

pub(crate) fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// The conversation `id`, if it is `owner`'s.
pub(crate) fn load(id: &str, owner: &str) -> Option<Chat> {
    if !valid_id(id) {
        return None;
    }
    let chat: Chat = serde_json::from_slice(&std::fs::read(dir().join(format!("{id}.json"))).ok()?).ok()?;
    (chat.owner == owner).then_some(chat)
}

pub(crate) fn save(chat: &Chat) -> Result<(), String> {
    let json = serde_json::to_vec(chat).map_err(|e| e.to_string())?;
    crate::private_storage::write(&dir().join(format!("{}.json", chat.id)), &json)
        .map_err(|e| format!("Cannot save the conversation: {e}"))
}

/// Who is asking: the signed-in account.
pub(crate) fn owner(req: &HttpRequest) -> Result<String, HttpResponse> {
    crate::auth::verified_claims(req).map(|c| c.sub)
        .ok_or_else(|| HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Sign in again")))
}

pub(crate) fn not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::<()>::error("No such conversation"))
}

/// The account's conversations, newest first, without their messages.
pub(super) async fn list(req: HttpRequest) -> HttpResponse {
    let who = match owner(&req) { Ok(w) => w, Err(r) => return r };
    let mut out: Vec<serde_json::Value> = std::fs::read_dir(dir()).into_iter().flatten().flatten()
        .filter_map(|e| serde_json::from_slice::<Chat>(&std::fs::read(e.path()).ok()?).ok())
        .filter(|c| c.owner == who)
        .map(|c| serde_json::json!({ "id": c.id, "title": c.title, "created": c.created, "updated": c.updated, "count": c.messages.len() }))
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse(c["updated"].as_i64().unwrap_or(0)));
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(out))
}

pub(super) async fn create(req: HttpRequest) -> HttpResponse {
    let who = match owner(&req) { Ok(w) => w, Err(r) => return r };
    let id = format!("{:032x}", rand::Rng::gen::<u128>(&mut rand::thread_rng()));
    let chat = Chat { id, owner: who, title: String::new(), session_id: String::new(), created: now(), updated: now(), messages: vec![] };
    match save(&chat) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::ok(chat)),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

pub(super) async fn get(req: HttpRequest, id: web::Path<String>) -> HttpResponse {
    let who = match owner(&req) { Ok(w) => w, Err(r) => return r };
    match load(&id, &who) {
        Some(chat) => HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(chat)),
        None => not_found(),
    }
}

pub(super) async fn delete(req: HttpRequest, id: web::Path<String>) -> HttpResponse {
    let who = match owner(&req) { Ok(w) => w, Err(r) => return r };
    if load(&id, &who).is_none() {
        return not_found();
    }
    match std::fs::remove_file(dir().join(format!("{id}.json"))) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::ok(())),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_32_lowercase_hex() {
        assert!(valid_id(&"a1".repeat(16)));
        assert!(!valid_id("../../etc/passwd") && !valid_id(&"A1".repeat(16)) && !valid_id("abc"));
    }
}
