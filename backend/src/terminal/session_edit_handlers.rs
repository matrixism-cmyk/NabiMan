//! Changing one session from the list: share it, its timeout and keep-alive,
//! its name, or closing it. Only its owner may.

use crate::lock_safe::LockExt;
use actix_web::{web, HttpRequest, HttpResponse};
use std::collections::HashMap;

use super::super::session::{persist, rename_tmux_window, PtyStore, TmuxSession};
use super::super::settings::SettingsStore;
use super::super::MAX_TIMEOUT_SECS;
use super::extract_username_header;

/// Longest name a session can be given.
const MAX_TITLE_LEN: usize = 60;

#[derive(serde::Deserialize)]
pub(super) struct ShareRequest {
    shared: bool,
}

#[derive(serde::Deserialize)]
pub(super) struct TimeoutRequest {
    timeout_secs: u64,
}

#[derive(serde::Deserialize)]
pub(super) struct KeepaliveRequest {
    keepalive: bool,
}

#[derive(serde::Deserialize)]
pub(super) struct RenameRequest {
    name: String,
}

fn may_touch(map: &HashMap<String, TmuxSession>, id: &str, user_id: &str) -> bool {
    map.get(id).map(|s| s.owned_by(user_id)).unwrap_or(false)
}
fn commit_edit(
    map: &mut HashMap<String, TmuxSession>,
    id: &str,
    edit: impl FnOnce(&mut TmuxSession),
) -> Result<(), HttpResponse> {
    let candidate = super::super::registry::prepare_update(map, id, edit).map_err(|_| {
        HttpResponse::Conflict().json(crate::models::ApiResponse::<()>::error(
            "Terminal metadata update exceeds storage limits; existing session unchanged",
        ))
    })?;
    *map = candidate;
    persist(map);
    Ok(())
}

pub(super) async fn toggle_share(
    path: web::Path<String>,
    body: web::Json<ShareRequest>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
) -> HttpResponse {
    let id = path.into_inner();
    let mut map = store.lock_safe();
    if !map.contains_key(&id) {
        return HttpResponse::Ok()
            .json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if !may_touch(
        &map,
        &id,
        &crate::auth::verified_user_id(&req).unwrap_or_default(),
    ) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner"));
    }
    if let Err(response) = commit_edit(&mut map, &id, |s| {
        s.shared = body.shared;
    }) {
        return response;
    }
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(if body.shared {
        "Session shared"
    } else {
        "Session unshared"
    }))
}

/// The signed-in account, to hold a member's session to its limits.
fn account(req: &HttpRequest, users: &crate::users::UserStore) -> Option<crate::models::NabimanUser> {
    let id = crate::auth::verified_user_id(req)?;
    users.lock().ok()?.iter().find(|u| u.id == id).cloned()
}

pub(super) async fn set_timeout(
    path: web::Path<String>,
    body: web::Json<TimeoutRequest>,
    store: web::Data<PtyStore>,
    users: web::Data<crate::users::UserStore>,
    req: HttpRequest,
) -> HttpResponse {
    let id = path.into_inner();
    if body.timeout_secs > MAX_TIMEOUT_SECS {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "Timeout too large (max 30 days, 0 = permanent)",
        ));
    }
    let mut map = store.lock_safe();
    if !map.contains_key(&id) {
        return HttpResponse::Ok()
            .json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if !may_touch(
        &map,
        &id,
        &crate::auth::verified_user_id(&req).unwrap_or_default(),
    ) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner"));
    }
    let (_, timeout_secs) = account(&req, &users)
        .map_or((false, body.timeout_secs), |u| crate::member_servers::session_terms(&u, false, body.timeout_secs));
    if let Err(response) = commit_edit(&mut map, &id, |s| {
        s.timeout_secs = timeout_secs;
        if timeout_secs != 0 {
            s.keepalive = false;
        }
    }) {
        return response;
    }
    let label = if timeout_secs == 0 {
        "permanent".to_string()
    } else {
        format!("{}s", timeout_secs)
    };
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(format!(
        "Timeout set to {}",
        label
    )))
}

/// Turn "keep this session alive" on or off for a running session.
pub(super) async fn set_keepalive(
    path: web::Path<String>,
    body: web::Json<KeepaliveRequest>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
    preferences: web::Data<SettingsStore>,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let mut map = store.lock_safe();
    if !map.contains_key(&id) {
        return HttpResponse::Ok()
            .json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if !may_touch(
        &map,
        &id,
        &crate::auth::verified_user_id(&req).unwrap_or_default(),
    ) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner"));
    }
    let user_settings = match preferences.lock_safe().get(&username) {
        Ok(settings) => settings,
        Err(_) => {
            return HttpResponse::ServiceUnavailable().json(
                crate::models::ApiResponse::<()>::error(
                    "Terminal preferences unavailable; storage recovery required",
                ),
            )
        }
    };
    let asked = if body.keepalive { 0 } else { user_settings.idle_timeout_secs.min(MAX_TIMEOUT_SECS) };
    let (keepalive, timeout_secs) = account(&req, &users)
        .map_or((body.keepalive, asked), |u| crate::member_servers::session_terms(&u, body.keepalive, asked));
    if body.keepalive && !keepalive {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "세션 유지는 관리자가 허용하지 않았습니다",
        ));
    }
    if let Err(response) = commit_edit(&mut map, &id, |s| {
        s.keepalive = keepalive;
        s.timeout_secs = timeout_secs;
    }) {
        return response;
    }
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(if keepalive {
        "Keep-alive enabled"
    } else {
        "Keep-alive disabled"
    }))
}

/// Name a session something the operator will recognise. An empty name puts it
/// back to going by its label.
pub(super) async fn rename_session(
    path: web::Path<String>,
    body: web::Json<RenameRequest>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
) -> HttpResponse {
    let id = path.into_inner();
    let title: String = body
        .name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TITLE_LEN)
        .collect();

    let mut map = store.lock_safe();
    if !may_touch(
        &map,
        &id,
        &crate::auth::verified_user_id(&req).unwrap_or_default(),
    ) {
        return HttpResponse::Ok()
            .json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    let tmux_name = map[&id].tmux_name.clone();
    if let Err(response) = commit_edit(&mut map, &id, |s| s.title = title.clone()) {
        return response;
    }
    drop(map);

    // Keep tmux's own status line in step with the name.
    rename_tmux_window(&tmux_name, if title.is_empty() { "shell" } else { &title });
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(title))
}

pub(super) async fn kill_session(
    path: web::Path<String>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
) -> HttpResponse {
    let id = path.into_inner();
    let mut map = store.lock_safe();
    if !may_touch(
        &map,
        &id,
        &crate::auth::verified_user_id(&req).unwrap_or_default(),
    ) {
        return HttpResponse::Ok()
            .json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if let Some(s) = map.get(&id) {
        if super::super::session::close_tmux_session(&s.tmux_name).is_err() {
            return HttpResponse::ServiceUnavailable().json(
                crate::models::ApiResponse::<()>::error(
                    "Could not confirm session termination; metadata retained",
                ),
            );
        }
    }
    map.remove(&id);
    persist(&map);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok("Session closed"))
}
