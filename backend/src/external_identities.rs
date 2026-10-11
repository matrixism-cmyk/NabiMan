//! Highest administrator confirms stable IDs in a specific Rancher realm.
use crate::mec::handlers::util::{created, from_service_error, ok_response};
use crate::mec::services::ServiceError;
use crate::mec::{
    db::identity_store::{self, IdentityLink},
    MecState,
};
use crate::models::mec::{AuditStatus, MecAuditLog};
use crate::users::UserStore;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

fn no_store(mut response: HttpResponse) -> HttpResponse {
    response.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static("no-store"),
    );
    response
}

fn target(store: &UserStore, id: &str) -> Result<(), HttpResponse> {
    let users = store.lock().unwrap();
    match users.iter().find(|u| u.id == id) {
        Some(u) if u.active() => Ok(()),
        Some(_) => Err(from_service_error(ServiceError::Conflict(
            "Only approved active members can be linked".into(),
        ))),
        None => Err(from_service_error(ServiceError::NotFound("member".into()))),
    }
}

pub async fn get(
    req: HttpRequest,
    id: web::Path<String>,
    users: web::Data<UserStore>,
    state: web::Data<MecState>,
) -> HttpResponse {
    if let Err(r) = crate::membership::require_super(&req, &users) {
        return r;
    }
    if let Err(r) = target(&users, &id) {
        return r;
    }
    let realm = match state.services.rancher.identity_realm() {
        Ok(realm) => realm,
        Err(e) => return from_service_error(e),
    };
    match identity_store::get(&state.db, &id, &realm) {
        Ok(link) => no_store(ok_response(serde_json::json!({"realm":realm,"link":link}))),
        Err(e) => from_service_error(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkRequest {
    pub realm: String,
    pub remote_user_id: String,
    pub reason: String,
}

pub async fn link(
    req: HttpRequest,
    id: web::Path<String>,
    body: web::Json<LinkRequest>,
    users: web::Data<UserStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let started = std::time::Instant::now();
    if let Err(r) = crate::membership::require_super(&req, &users) {
        return r;
    }
    if let Err(r) = target(&users, &id) {
        return r;
    }
    if body.remote_user_id.is_empty()
        || body.remote_user_id.len() > 256
        || !body
            .remote_user_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b))
        || !(8..=1000).contains(&body.reason.trim().len())
        || body.reason.chars().any(char::is_control)
    {
        return from_service_error(ServiceError::InvalidInput("Enter a valid remote account ID and a reason of 8–1000 bytes without control characters".into()));
    }
    let realm = match state.services.rancher.identity_realm() {
        Ok(realm) => realm,
        Err(e) => return from_service_error(e),
    };
    if realm != body.realm {
        return from_service_error(ServiceError::Conflict(
            "The configured Rancher realm changed; reload and verify the target".into(),
        ));
    }
    let remote = match state.services.rancher.get_user(&body.remote_user_id).await {
        Ok(user) => user,
        Err(e) => return from_service_error(e),
    };
    if remote.id != body.remote_user_id
        || !remote.enabled
        || remote.username.is_empty()
        || remote.username.len() > 256
        || remote.username.chars().any(char::is_control)
    {
        return from_service_error(ServiceError::Conflict(
            "Rancher account is disabled or its identity could not be verified".into(),
        ));
    }
    // Recheck current local identities after upstream I/O. Hold the user store
    // through commit so review/delete cannot race this local decision.
    let current = users.lock().unwrap();
    let Some(claims) = crate::auth::verified_claims(&req) else {
        return HttpResponse::Unauthorized().finish();
    };
    let active_sessions = sessions.lock().unwrap();
    if claims.exp as u128 <= chrono::Utc::now().timestamp().max(0) as u128
        || !active_sessions.get(&claims.sid).is_some_and(|s| {
            s.username == claims.sub
                && Some(s.user_id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
        })
    {
        return HttpResponse::Unauthorized().finish();
    }
    let actor = crate::auth::extract_username(&req).and_then(|name| {
        current.iter().find(|u| {
            u.username == name
                && Some(u.id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
        })
    });
    let Some(actor) = actor.filter(|u| u.super_admin() && u.active()) else {
        return HttpResponse::Forbidden().finish();
    };
    if !current.iter().any(|u| u.id == *id && u.active()) {
        return from_service_error(ServiceError::Conflict(
            "Member approval changed; reload before linking".into(),
        ));
    }
    let now = chrono::Utc::now();
    let link = IdentityLink {
        local_user_id: id.to_string(),
        realm,
        remote_user_id: remote.id,
        remote_username: remote.username,
        verified_at: now.to_rfc3339(),
        linked_by: actor.username.clone(),
        reason: body.reason.trim().into(),
    };
    let audit = MecAuditLog {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: now,
        user: actor.username.clone(),
        action: "external_identity_link".into(),
        resource_type: "member".into(),
        resource_id: id.to_string(),
        status: AuditStatus::Success,
        duration_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        input: Some(
            serde_json::json!({"realm":link.realm,"remote_user_id":link.remote_user_id,"reason":link.reason}),
        ),
        output: Some(serde_json::json!({"remote_username":link.remote_username})),
        error: None,
        operations: vec![],
        source_ip: crate::mec::handlers::util::source_ip(&req),
        user_agent: crate::mec::handlers::util::user_agent(&req),
    };
    match identity_store::insert(&state.db, &link, &audit) {
        Ok(()) => no_store(created(link)),
        Err(e) => from_service_error(e),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route(
        "/api/users/{id}/external-identities/rancher",
        web::get().to(get),
    )
    .route(
        "/api/users/{id}/external-identities/rancher",
        web::post().to(link),
    );
}

#[cfg(test)]
#[path = "external_identity_tests.rs"]
mod tests;
