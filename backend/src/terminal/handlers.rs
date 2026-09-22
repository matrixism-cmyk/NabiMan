//! REST endpoints around the session registry and the per-user settings.

use actix_web::{web, HttpRequest, HttpResponse};
use std::collections::HashMap;

use super::session::{
    capture_scrollback, now_unix, pane_preview, persist, rename_tmux_window, tmux,
    tmux_session_exists, PtyStore, TmuxSession, MAX_SCROLLBACK,
};
use super::settings::{save_settings_for, settings_for, TerminalSettings};
use super::MAX_TIMEOUT_SECS;

#[derive(serde::Serialize)]
struct SessionInfo {
    id: String, label: String, owner: String, ws_count: u32, shared: bool,
    age_secs: u64, timeout_secs: u64, scrollback: u32, keepalive: bool, alive: bool,
    /// The name the operator gave it, empty when it still goes by its label.
    title: String,
    /// The last lines on screen, for a thumbnail in the list.
    preview: String,
}

/// How many lines of the screen the list carries per session.
const PREVIEW_LINES: usize = 12;
/// Longest name a session can be given.
const MAX_TITLE_LEN: usize = 60;

async fn list_sessions(store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let username = extract_username_header(&req);
    let map = store.lock().unwrap();
    let now = now_unix();
    let mut list: Vec<SessionInfo> = map.iter()
        .filter(|(_, s)| s.owner == username || s.shared)
        .map(|(id, s)| {
            let alive = tmux_session_exists(&s.tmux_name);
            SessionInfo {
                id: id.clone(), label: s.label.clone(), owner: s.owner.clone(),
                ws_count: s.ws_count, shared: s.shared,
                age_secs: now.saturating_sub(s.created_unix),
                timeout_secs: s.timeout_secs, scrollback: s.scrollback,
                keepalive: s.keepalive, alive,
                title: s.title.clone(),
                preview: if alive { pane_preview(&s.tmux_name, PREVIEW_LINES) } else { String::new() },
            }
        }).collect();
    // A stable order: newest first. The store is a map, so without this the
    // cards would shuffle on every refresh.
    list.sort_by(|a, b| a.age_secs.cmp(&b.age_secs).then_with(|| a.id.cmp(&b.id)));
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(list))
}

#[derive(serde::Deserialize)]
struct ShareRequest { shared: bool }

#[derive(serde::Deserialize)]
struct TimeoutRequest { timeout_secs: u64 }

#[derive(serde::Deserialize)]
struct KeepaliveRequest { keepalive: bool }

#[derive(serde::Deserialize)]
struct ScrollbackQuery { lines: Option<u32> }

#[derive(serde::Deserialize)]
struct RenameRequest { name: String }

fn may_touch(map: &HashMap<String, TmuxSession>, id: &str, user: &str) -> bool {
    map.get(id).map(|s| s.owner == user).unwrap_or(false)
}

async fn toggle_share(path: web::Path<String>, body: web::Json<ShareRequest>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let mut map = store.lock().unwrap();
    if !map.contains_key(&id) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if !may_touch(&map, &id, &username) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner"));
    }
    if let Some(s) = map.get_mut(&id) { s.shared = body.shared; }
    persist(&map);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(
        if body.shared { "Session shared" } else { "Session unshared" }))
}

async fn set_timeout(path: web::Path<String>, body: web::Json<TimeoutRequest>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    if body.timeout_secs > MAX_TIMEOUT_SECS {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Timeout too large (max 30 days, 0 = permanent)"));
    }
    let mut map = store.lock().unwrap();
    if !map.contains_key(&id) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if !may_touch(&map, &id, &username) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner"));
    }
    if let Some(s) = map.get_mut(&id) {
        s.timeout_secs = body.timeout_secs;
        if body.timeout_secs != 0 { s.keepalive = false; }
    }
    persist(&map);
    let label = if body.timeout_secs == 0 { "permanent".to_string() } else { format!("{}s", body.timeout_secs) };
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(format!("Timeout set to {}", label)))
}

/// Turn "keep this session alive" on or off for a running session.
async fn set_keepalive(path: web::Path<String>, body: web::Json<KeepaliveRequest>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let mut map = store.lock().unwrap();
    if !map.contains_key(&id) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if !may_touch(&map, &id, &username) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Not owner"));
    }
    let user_settings = settings_for(&username);
    if let Some(s) = map.get_mut(&id) {
        s.keepalive = body.keepalive;
        s.timeout_secs = if body.keepalive { 0 } else { user_settings.idle_timeout_secs };
    }
    persist(&map);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(
        if body.keepalive { "Keep-alive enabled" } else { "Keep-alive disabled" }))
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
    let username = extract_username_header(&req);
    let title: String = body.name.trim().chars().filter(|c| !c.is_control()).take(MAX_TITLE_LEN).collect();

    let mut map = store.lock().unwrap();
    if !may_touch(&map, &id, &username) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    let tmux_name = match map.get_mut(&id) {
        Some(s) => { s.title = title.clone(); s.tmux_name.clone() }
        None => return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found")),
    };
    persist(&map);
    drop(map);

    // Keep tmux's own status line in step with the name.
    rename_tmux_window(&tmux_name, if title.is_empty() { "shell" } else { &title });
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(title))
}

async fn kill_session(path: web::Path<String>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let mut map = store.lock().unwrap();
    if !may_touch(&map, &id, &username) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found"));
    }
    if let Some(s) = map.remove(&id) {
        let _ = tmux().args(["kill-session", "-t", &s.tmux_name]).output();
    }
    persist(&map);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok("Session closed"))
}

async fn get_scrollback(path: web::Path<String>, query: web::Query<ScrollbackQuery>, store: web::Data<PtyStore>, req: HttpRequest) -> HttpResponse {
    let id = path.into_inner();
    let username = extract_username_header(&req);
    let (tmux_name, lines) = {
        let map = store.lock().unwrap();
        match map.get(&id) {
            Some(s) if s.owner == username || s.shared => {
                (s.tmux_name.clone(), query.lines.unwrap_or(s.scrollback))
            }
            _ => return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error("Session not found")),
        }
    };
    match capture_scrollback(&tmux_name, lines.min(MAX_SCROLLBACK)) {
        Ok(text) => HttpResponse::Ok().json(crate::models::ApiResponse::ok(text)),
        Err(e) => HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(&e)),
    }
}

async fn get_settings(req: HttpRequest) -> HttpResponse {
    let username = extract_username_header(&req);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(settings_for(&username)))
}

async fn put_settings(body: web::Json<TerminalSettings>, req: HttpRequest) -> HttpResponse {
    let username = extract_username_header(&req);
    let mut settings = body.into_inner();
    if settings.scrollback_lines < 100 || settings.scrollback_lines > MAX_SCROLLBACK {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "scrollback_lines must be between 100 and 200000"));
    }
    if settings.idle_timeout_secs > MAX_TIMEOUT_SECS {
        return HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(
            "idle_timeout_secs must be 0 (permanent) or at most 30 days"));
    }
    settings.keepalive_interval_secs = settings.keepalive_interval_secs.clamp(5, 3600);
    settings.font_size = settings.font_size.clamp(8, 28);
    if settings.keepalive { settings.idle_timeout_secs = 0; }
    match save_settings_for(&username, &settings) {
        Ok(()) => HttpResponse::Ok().json(crate::models::ApiResponse::ok(settings)),
        Err(e) => HttpResponse::Ok().json(crate::models::ApiResponse::<()>::error(&e)),
    }
}

pub(crate) fn extract_username_header(req: &HttpRequest) -> String {
    req.headers().get("Authorization")
        .and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|t| crate::auth::decode_claims_unverified(t).ok())
        .map(|d| d.claims.sub).unwrap_or_default()
}

/// Routes that operate on the session registry and the per-user settings.
pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/settings", web::get().to(get_settings))
        .route("/api/terminal/settings", web::put().to(put_settings))
        .route("/api/terminal/sessions", web::get().to(list_sessions))
        .route("/api/terminal/sessions/{id}", web::delete().to(kill_session))
        .route("/api/terminal/sessions/{id}/name", web::post().to(rename_session))
        .route("/api/terminal/sessions/{id}/share", web::post().to(toggle_share))
        .route("/api/terminal/sessions/{id}/timeout", web::post().to(set_timeout))
        .route("/api/terminal/sessions/{id}/keepalive", web::post().to(set_keepalive))
        .route("/api/terminal/sessions/{id}/scrollback", web::get().to(get_scrollback));
}
