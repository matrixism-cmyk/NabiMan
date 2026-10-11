use super::*;
use crate::{models::ApiResponse, users::UserStore};
use actix_web::{web, HttpRequest, HttpResponse};
fn response(code: actix_web::http::StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(code)
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::<()>::error(message))
}
fn unavailable() -> HttpResponse {
    response(
        actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
        "Token storage unavailable; inspect storage before restart",
    )
}
fn administrator(req: &HttpRequest, users: &UserStore) -> Result<NabimanUser, HttpResponse> {
    let id = crate::auth::verified_user_id(req);
    users
        .lock()
        .unwrap()
        .iter()
        .find(|u| Some(u.id.as_str()) == id.as_deref() && u.active() && u.super_admin())
        .cloned()
        .ok_or_else(|| {
            response(
                actix_web::http::StatusCode::FORBIDDEN,
                "Highest administrator required",
            )
        })
}
async fn list(
    req: HttpRequest,
    store: web::Data<ApiKeyStore>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    if let Err(r) = administrator(&req, &users) {
        return r;
    }
    let state = store.lock().unwrap();
    if state.failed {
        return unavailable();
    }
    let members = users.lock().unwrap();
    let items: Vec<_> = state.keys.iter().map(|k| {
        let owner = members.iter().find(|u| u.id == k.owner_id);
        let status = if k.version != 1 { "legacy_disabled" }
            else if k.expires_at.as_deref().and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok()).is_none_or(|e| e <= chrono::Utc::now()) { "expired" }
            else if owner.is_none_or(|u| !u.active() || stamp(u) != k.security_stamp) { "owner_changed" }
            else if std::env::var("NABIMAN_API_TOKENS_DISABLED").as_deref() == Ok("1") { "operator_disabled" }
            else { "active" };
        serde_json::json!({"id":k.id,"name":k.name,"prefix":k.prefix,"created_at":k.created_at,"last_used":k.last_used,
            "expires_at":k.expires_at,"scopes":k.scopes,"owner_id":k.owner_id,"owner_name":owner.map(|u| &u.username),"status":status})
    }).collect();
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(items))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    name: String,
    scopes: Vec<String>,
    expires_in_days: u32,
}
async fn create(
    req: HttpRequest,
    body: web::Json<Create>,
    store: web::Data<ApiKeyStore>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    let actor = match administrator(&req, &users) {
        Ok(u) => u,
        Err(r) => return r,
    };
    if body.name.trim().is_empty()
        || body.name.len() > 100
        || body.name.chars().any(char::is_control)
        || !(1..=90).contains(&body.expires_in_days)
        || body.scopes.is_empty()
        || body.scopes.len() > SCOPES.len()
        || body.scopes.iter().any(|s| !SCOPES.contains(&s.as_str()))
        || body
            .scopes
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != body.scopes.len()
    {
        return response(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Choose a name, unique supported scopes, and expiry from 1 to 90 days",
        );
    }
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let raw = format!("nbm_{}", hex::encode(bytes));
    let now = chrono::Utc::now();
    let key = ApiKey {
        id: format!("ak_{}", uuid::Uuid::new_v4()),
        name: body.name.trim().into(),
        key_hash: hash(&raw),
        prefix: raw[..12].into(),
        role: "scoped".into(),
        created_at: now.to_rfc3339(),
        last_used: None,
        expires_at: Some((now + chrono::Duration::days(body.expires_in_days.into())).to_rfc3339()),
        version: 1,
        owner_id: actor.id.clone(),
        security_stamp: stamp(&actor),
        scopes: body.scopes.clone(),
    };
    let mut state = store.lock().unwrap();
    if state.failed {
        return unavailable();
    }
    if state.keys.len() >= 1000 {
        return response(
            actix_web::http::StatusCode::CONFLICT,
            "Token limit reached; revoke unused tokens",
        );
    }
    let payload = serde_json::json!({"id":key.id,"name":key.name,"key":raw,"prefix":key.prefix,"scopes":key.scopes,"expires_at":key.expires_at});
    let mut changed = state.keys.clone();
    changed.push(key);
    if state.commit(changed).is_err() {
        return unavailable();
    }
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(payload))
}
async fn revoke(
    req: HttpRequest,
    path: web::Path<String>,
    store: web::Data<ApiKeyStore>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    if let Err(r) = administrator(&req, &users) {
        return r;
    }
    let mut state = store.lock().unwrap();
    if state.failed {
        return unavailable();
    }
    let changed: Vec<_> = state
        .keys
        .iter()
        .filter(|k| k.id != *path)
        .cloned()
        .collect();
    if changed.len() == state.keys.len() {
        return response(actix_web::http::StatusCode::NOT_FOUND, "Token not found");
    }
    if state.commit(changed).is_err() {
        return unavailable();
    }
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(
            "Token revoked; already accepted requests may finish",
        ))
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/apikeys")
            .route("", web::get().to(list))
            .route("", web::post().to(create))
            .route("/{id}", web::delete().to(revoke)),
    );
}
