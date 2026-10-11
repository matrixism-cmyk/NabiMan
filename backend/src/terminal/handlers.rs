//! REST endpoints around the session registry and the per-user settings.

#[path = "preferences_handlers.rs"]
mod preferences;
use preferences::{get_settings, put_settings};

use actix_web::{web, HttpRequest, HttpResponse};
use std::collections::HashMap;

use super::session::{
    capture_scrollback, now_unix, pane_preview, persist, rename_tmux_window, PtyStore, TmuxSession,
    MAX_SCROLLBACK,
};
use super::settings::{SettingsStore, TerminalSettings};
use super::MAX_TIMEOUT_SECS;

#[derive(serde::Serialize)]
struct SessionInfo {
    id: String,
    label: String,
    owner: String,
    ws_count: u32,
    shared: bool,
    age_secs: u64,
    timeout_secs: u64,
    scrollback: u32,
    keepalive: bool,
    alive: bool,
    state: &'static str,
    /// The name the operator gave it, empty when it still goes by its label.
    title: String,
    /// The last lines on screen, for a thumbnail in the list.
    preview: String,
}

/// How many lines of the screen the list carries per session.
const PREVIEW_LINES: usize = 12;
/// Longest name a session can be given.
const MAX_TITLE_LEN: usize = 60;

async fn list_sessions(
    store: web::Data<PtyStore>,
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let user_id = crate::auth::verified_user_id(&req).unwrap_or_default();
    let map = store.lock().unwrap();
    let now = now_unix();
    let mut list: Vec<SessionInfo> = map
        .iter()
        .filter(|(_, s)| super::session::visible_to(s, &user_id, &users))
        .map(|(id, s)| {
            let state = if !super::remote_access::allowed(s) {
                "blocked"
            } else {
                match super::session::tmux_session_state(&s.tmux_name) {
                    Ok(true) => "alive",
                    Ok(false) => "ended",
                    Err(_) => "unknown",
                }
            };
            let alive = state == "alive";
            SessionInfo {
                id: id.clone(),
                label: s.label.clone(),
                owner: s.owner.clone(),
                ws_count: s.ws_count,
                shared: s.shared,
                age_secs: now.saturating_sub(s.created_unix),
                timeout_secs: s.timeout_secs,
                scrollback: s.scrollback,
                keepalive: s.keepalive,
                alive,
                state,
                title: s.title.clone(),
                preview: if alive {
                    pane_preview(&s.tmux_name, PREVIEW_LINES)
                } else {
                    String::new()
                },
            }
        })
        .collect();
    // A stable order: newest first. The store is a map, so without this the
    // cards would shuffle on every refresh.
    list.sort_by(|a, b| a.age_secs.cmp(&b.age_secs).then_with(|| a.id.cmp(&b.id)));
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(list))
}

#[derive(serde::Deserialize)]
struct ShareRequest {
    shared: bool,
}

#[derive(serde::Deserialize)]
struct TimeoutRequest {
    timeout_secs: u64,
}

#[derive(serde::Deserialize)]
struct KeepaliveRequest {
    keepalive: bool,
}

#[derive(serde::Deserialize)]
struct ScrollbackQuery {
    lines: Option<u32>,
}

#[derive(serde::Deserialize)]
struct RenameRequest {
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
    let candidate = super::registry::prepare_update(map, id, edit).map_err(|_| {
        HttpResponse::Conflict().json(crate::models::ApiResponse::<()>::error(
            "Terminal metadata update exceeds storage limits; existing session unchanged",
        ))
    })?;
    *map = candidate;
    persist(map);
    Ok(())
}

async fn toggle_share(
    path: web::Path<String>,
    body: web::Json<ShareRequest>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
) -> HttpResponse {
    let id = path.into_inner();
    let mut map = store.lock().unwrap();
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

async fn set_timeout(
    path: web::Path<String>,
    body: web::Json<TimeoutRequest>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
) -> HttpResponse {
    let id = path.into_inner();
    if body.timeout_secs > MAX_TIMEOUT_SECS {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "Timeout too large (max 30 days, 0 = permanent)",
        ));
    }
    let mut map = store.lock().unwrap();
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
        s.timeout_secs = body.timeout_secs;
        if body.timeout_secs != 0 {
            s.keepalive = false;
        }
    }) {
        return response;
    }
    let label = if body.timeout_secs == 0 {
        "permanent".to_string()
    } else {
        format!("{}s", body.timeout_secs)
    };
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(format!(
        "Timeout set to {}",
        label
    )))
}

/// Turn "keep this session alive" on or off for a running session.
async fn set_keepalive(
    path: web::Path<String>,
    body: web::Json<KeepaliveRequest>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
    preferences: web::Data<SettingsStore>,
) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let mut map = store.lock().unwrap();
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
    let user_settings = match preferences.lock().unwrap().get(&username) {
        Ok(settings) => settings,
        Err(_) => {
            return HttpResponse::ServiceUnavailable().json(
                crate::models::ApiResponse::<()>::error(
                    "Terminal preferences unavailable; storage recovery required",
                ),
            )
        }
    };
    if let Err(response) = commit_edit(&mut map, &id, |s| {
        s.keepalive = body.keepalive;
        s.timeout_secs = if body.keepalive {
            0
        } else {
            user_settings.idle_timeout_secs.min(MAX_TIMEOUT_SECS)
        };
    }) {
        return response;
    }
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(if body.keepalive {
        "Keep-alive enabled"
    } else {
        "Keep-alive disabled"
    }))
}

/// Name a session something the operator will recognise. An empty name puts it
/// back to going by its label.
async fn rename_session(
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

    let mut map = store.lock().unwrap();
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

async fn kill_session(
    path: web::Path<String>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
) -> HttpResponse {
    let id = path.into_inner();
    let mut map = store.lock().unwrap();
    if !may_touch(
        &map,
        &id,
        &crate::auth::verified_user_id(&req).unwrap_or_default(),
    ) {
        return HttpResponse::Ok()
            .json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if let Some(s) = map.get(&id) {
        if super::session::close_tmux_session(&s.tmux_name).is_err() {
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

async fn get_scrollback(
    path: web::Path<String>,
    query: web::Query<ScrollbackQuery>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let id = path.into_inner();
    let user_id = crate::auth::verified_user_id(&req).unwrap_or_default();
    let (tmux_name, lines) = {
        let map = store.lock().unwrap();
        match map.get(&id) {
            Some(s)
                if super::session::visible_to(s, &user_id, &users)
                    && super::remote_access::allowed(s) =>
            {
                (s.tmux_name.clone(), query.lines.unwrap_or(s.scrollback))
            }
            _ => {
                return HttpResponse::Ok()
                    .json(crate::models::ApiResponse::<()>::error("Session not found"))
            }
        }
    };
    match capture_scrollback(&tmux_name, lines.min(MAX_SCROLLBACK)) {
        Ok(text) => HttpResponse::Ok().json(crate::models::ApiResponse::ok(text)),
        Err(e) => HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(&e)),
    }
}

pub(crate) fn extract_username_header(req: &HttpRequest) -> String {
    crate::auth::extract_username(req).unwrap_or_default()
}

/// Routes that operate on the session registry and the per-user settings.
pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/settings", web::get().to(get_settings))
        .route("/api/terminal/settings", web::put().to(put_settings))
        .route("/api/terminal/sessions", web::get().to(list_sessions))
        .route(
            "/api/terminal/sessions/{id}",
            web::delete().to(kill_session),
        )
        .route(
            "/api/terminal/sessions/{id}/name",
            web::post().to(rename_session),
        )
        .route(
            "/api/terminal/sessions/{id}/share",
            web::post().to(toggle_share),
        )
        .route(
            "/api/terminal/sessions/{id}/timeout",
            web::post().to(set_timeout),
        )
        .route(
            "/api/terminal/sessions/{id}/keepalive",
            web::post().to(set_keepalive),
        )
        .route(
            "/api/terminal/sessions/{id}/scrollback",
            web::get().to(get_scrollback),
        );
}
