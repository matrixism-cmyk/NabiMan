//! REST endpoints around the session registry and the per-user settings.

#[path = "claude_handlers.rs"]
mod claude;
#[path = "preferences_handlers.rs"]
mod preferences;
#[path = "session_edit_handlers.rs"]
mod edits;
use claude::{directs_of, notes_of, remote_claude, sees_remote_claude, session_claude, LinkNote};
use edits::{kill_session, rename_session, set_keepalive, set_timeout, toggle_share};
use preferences::{get_settings, put_settings};

use actix_web::{web, HttpRequest, HttpResponse};
use std::collections::HashMap;

use super::session::{capture_scrollback, now_unix, pane_preview, PtyStore, TmuxSession, MAX_SCROLLBACK};
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
    /// The AI agents running in it, by kind (`claude`, `codex`, `gemini`…).
    agents: Vec<String>,
    /// Sessions in this list its Claude has lately handed work to.
    directs: Vec<String>,
    /// Its Claude leads others here and is led by none.
    orchestrator: bool,
    /// The session the operator made the lead of all Claude work (총괄).
    lead: bool,
    /// For each of `directs`: the first line of the work it last sent there, and when.
    notes: Vec<LinkNote>,
}

/// How many lines of the screen the list carries per session.
const PREVIEW_LINES: usize = 12;

async fn list_sessions(
    store: web::Data<PtyStore>,
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let lead = super::claude_lead::lead_tmux();
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
                agents: here.map(|h| h.agents.clone()).unwrap_or_default(),
                orchestrator: here.is_some_and(|h| h.orchestrator) && !directs.is_empty(),
                lead: lead.as_deref() == Some(s.tmux_name.as_str()),
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
struct ScrollbackQuery {
    lines: Option<u32>,
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
