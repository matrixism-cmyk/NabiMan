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
    /// Claude Code sessions running in it, with the address others message them by.
    claude: Vec<super::claude_peers::ClaudePeer>,
    /// A Claude session in it is working on a turn.
    busy: bool,
    /// Sessions in this list its Claude has lately handed work to.
    directs: Vec<String>,
    /// Its Claude leads others here and is led by none.
    orchestrator: bool,
    /// For each of `directs`: the first line of the work it last sent there, and when.
    notes: Vec<LinkNote>,
}

/// What travels along one arrow: the latest message's first line.
#[derive(serde::Serialize, Clone)]
struct LinkNote {
    to: String,
    text: String,
    at: u64,
}

/// The notes for a card's arrows, named like its `directs`.
fn notes_of(here: &super::claude_peers::Seen, id_of: &HashMap<&str, &str>, remote_ok: bool) -> Vec<LinkNote> {
    here.notes
        .iter()
        .filter_map(|(t, (at, text))| {
            let to = id_of.get(t.as_str()).map(|i| i.to_string()).or_else(|| (remote_ok && t.starts_with("rc:")).then(|| t.clone()))?;
            Some(LinkNote { to, text: text.clone(), at: *at })
        })
        .collect()
}

/// Not a remote-only member (who sees only their own servers' shells).
fn sees_remote_claude(user_id: &str, users: &crate::users::UserStore) -> bool {
    !user_id.is_empty()
        && users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.id == user_id)
            .is_some_and(|u| !crate::member_servers::is_member(u))
}

/// The cards a session's Claude has lately handed work to: sessions this
/// account sees, and remote Claude sessions with no tile when it sees those.
fn directs_of(here: &super::claude_peers::Seen, id_of: &HashMap<&str, &str>, remote_ok: bool) -> Vec<String> {
    here.leads
        .iter()
        .filter_map(|t| id_of.get(t.as_str()).map(|i| i.to_string()).or_else(|| (remote_ok && t.starts_with("rc:")).then(|| t.clone())))
        .collect()
}

/// A Claude session on a server the tiles are connected to, running under
/// none of them (started at that machine, say). It gets a card of its own.
#[derive(serde::Serialize)]
struct RemoteClaudeCard {
    id: String,
    server: String,
    claude: Vec<super::claude_peers::ClaudePeer>,
    busy: bool,
    directs: Vec<String>,
    orchestrator: bool,
    notes: Vec<LinkNote>,
}

async fn remote_claude(store: web::Data<PtyStore>, req: HttpRequest, users: web::Data<crate::users::UserStore>) -> HttpResponse {
    let user_id = crate::auth::verified_user_id(&req).unwrap_or_default();
    if !sees_remote_claude(&user_id, &users) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::ok(Vec::<RemoteClaudeCard>::new()));
    }
    super::claude_remote::want(store.get_ref());
    let seen = super::claude_peers::look();
    let map = store.lock().unwrap();
    let id_of: HashMap<&str, &str> = map
        .iter()
        .filter(|(_, s)| super::session::visible_to(s, &user_id, &users))
        .map(|(id, s)| (s.tmux_name.as_str(), id.as_str()))
        .collect();
    let mut cards: Vec<RemoteClaudeCard> = seen
        .iter()
        .filter(|(k, h)| k.starts_with("rc:") && h.ghost_server.is_some() && !h.claude.is_empty())
        .map(|(k, h)| {
            let directs = directs_of(h, &id_of, true);
            RemoteClaudeCard {
                id: k.clone(),
                server: h.ghost_server.clone().unwrap_or_default(),
                claude: h.claude.clone(),
                busy: h.busy(),
                orchestrator: h.orchestrator && !directs.is_empty(),
                notes: notes_of(h, &id_of, true),
                directs,
            }
        })
        .collect();
    cards.sort_by(|a, b| (&a.server, &a.claude[0].name).cmp(&(&b.server, &b.claude[0].name)));
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(cards))
}

#[derive(serde::Serialize)]
struct ClaudeInfo {
    claude: Vec<super::claude_peers::ClaudePeer>,
    busy: bool,
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
    // Remote servers' Claude sessions are for operators, not remote-only members.
    let remote_ok = sees_remote_claude(&user_id, &users);
    if remote_ok {
        super::claude_remote::want(store.get_ref());
    }
    // Before the lock: it asks tmux and reads /proc.
    let seen = super::claude_peers::look();
    let map = store.lock().unwrap();
    let now = now_unix();
    let visible: Vec<(&String, &TmuxSession)> = map
        .iter()
        .filter(|(_, s)| super::session::visible_to(s, &user_id, &users))
        .collect();
    // Links are shown only between sessions this account sees.
    let id_of: HashMap<&str, &str> = visible.iter().map(|(id, s)| (s.tmux_name.as_str(), id.as_str())).collect();
    let mut list: Vec<SessionInfo> = visible
        .into_iter()
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
            let here = seen.get(&s.tmux_name).filter(|_| alive);
            let directs: Vec<String> = here.map(|h| directs_of(h, &id_of, remote_ok)).unwrap_or_default();
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
                claude: here.map(|h| h.claude.clone()).unwrap_or_default(),
                busy: here.is_some_and(|h| h.busy()),
                orchestrator: here.is_some_and(|h| h.orchestrator) && !directs.is_empty(),
                notes: here.map(|h| notes_of(h, &id_of, remote_ok)).unwrap_or_default(),
                directs,
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

/// The signed-in account, to hold a member's session to its limits.
fn account(req: &HttpRequest, users: &crate::users::UserStore) -> Option<crate::models::NabimanUser> {
    let id = crate::auth::verified_user_id(req)?;
    users.lock().ok()?.iter().find(|u| u.id == id).cloned()
}

async fn set_timeout(
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
async fn set_keepalive(
    path: web::Path<String>,
    body: web::Json<KeepaliveRequest>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
    preferences: web::Data<SettingsStore>,
    users: web::Data<crate::users::UserStore>,
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

/// `GET …/sessions/{id}/claude`: the Claude sessions in one session, for its header.
async fn session_claude(
    path: web::Path<String>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let id = path.into_inner();
    let user_id = crate::auth::verified_user_id(&req).unwrap_or_default();
    let tmux_name = {
        let map = store.lock().unwrap();
        match map.get(&id) {
            Some(s)
                if super::session::visible_to(s, &user_id, &users)
                    && super::remote_access::allowed(s) =>
            {
                s.tmux_name.clone()
            }
            _ => {
                return HttpResponse::Ok()
                    .json(crate::models::ApiResponse::<()>::error("Session not found"))
            }
        }
    };
    let seen = super::claude_peers::look();
    let here = seen.get(&tmux_name);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(ClaudeInfo {
        claude: here.map(|h| h.claude.clone()).unwrap_or_default(),
        busy: here.is_some_and(|h| h.busy()),
    }))
}

pub(crate) fn extract_username_header(req: &HttpRequest) -> String {
    crate::auth::extract_username(req).unwrap_or_default()
}

/// Routes that operate on the session registry and the per-user settings.
pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/settings", web::get().to(get_settings))
        .route("/api/terminal/settings", web::put().to(put_settings))
        .route("/api/terminal/sessions", web::get().to(list_sessions))
        .route("/api/terminal/claude-remote", web::get().to(remote_claude))
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
        )
        .route(
            "/api/terminal/sessions/{id}/claude",
            web::get().to(session_claude),
        );
}
