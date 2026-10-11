//! Account login sessions; tenant membership never grants access to another account's sessions.
use crate::{
    auth,
    jwt_sessions::{SessionInfo, SessionStore},
    models::{ApiResponse, NabimanUser},
    users::UserStore,
};
use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::collections::HashMap;

fn actor<'a>(
    req: &HttpRequest,
    users: &'a [NabimanUser],
    sessions: &HashMap<String, SessionInfo>,
) -> Option<(&'a NabimanUser, String)> {
    let id = auth::verified_user_id(req)?;
    let claims = auth::verified_claims(req)?;
    let user = users
        .iter()
        .find(|user| user.id == id && user.username == claims.sub && user.active())?;
    let session = sessions.get(&claims.sid)?;
    if id.is_empty() || session.user_id != id || session.username != user.username {
        return None;
    }
    Some((user, claims.sid))
}

#[get("/api/auth/sessions")]
async fn list(
    req: HttpRequest,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let users = users.lock().unwrap();
    let sessions = sessions.lock().unwrap();
    let Some((actor, current)) = actor(&req, &users, &sessions) else {
        return HttpResponse::Unauthorized().finish();
    };
    let mut list: Vec<_> = sessions.values().filter(|s| s.user_id == actor.id || actor.super_admin()).map(|s| {
        serde_json::json!({"session_id":s.session_id,"user_id":s.user_id,"username":s.username,
            "created_at":s.created_at,"last_active":s.last_active,"ip":s.ip,"is_current":s.session_id == current})
    }).collect();
    list.sort_by(|a, b| a["created_at"].as_str().cmp(&b["created_at"].as_str()));
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(list))
}
#[derive(Deserialize)]
struct RevokeRequest {
    session_id: String,
}
#[post("/api/auth/sessions/revoke")]
async fn revoke(
    req: HttpRequest,
    body: web::Json<RevokeRequest>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let users = users.lock().unwrap();
    let mut sessions = sessions.lock().unwrap();
    let Some((actor, _)) = actor(&req, &users, &sessions) else {
        return HttpResponse::Unauthorized().finish();
    };
    if !sessions
        .get(&body.session_id)
        .is_some_and(|s| s.user_id == actor.id || actor.super_admin())
    {
        return HttpResponse::NotFound().json(ApiResponse::<()>::error("Session not found"));
    }
    sessions.remove(&body.session_id);
    HttpResponse::Ok().json(ApiResponse::ok("Session revoked"))
}
fn revoke_own(
    req: HttpRequest,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
    keep_current: bool,
) -> HttpResponse {
    let users = users.lock().unwrap();
    let mut sessions = sessions.lock().unwrap();
    let Some((actor, current)) = actor(&req, &users, &sessions) else {
        return HttpResponse::Unauthorized().finish();
    };
    let id = actor.id.clone();
    sessions.retain(|sid, s| s.user_id != id || (keep_current && *sid == current));
    HttpResponse::Ok().json(ApiResponse::ok("Sessions revoked"))
}
#[post("/api/auth/sessions/revoke-all")]
async fn revoke_all(
    req: HttpRequest,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    revoke_own(req, users, sessions, false)
}
#[post("/api/auth/sessions/revoke-others")]
async fn revoke_others(
    req: HttpRequest,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    revoke_own(req, users, sessions, true)
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list)
        .service(revoke)
        .service(revoke_all)
        .service(revoke_others);
}
