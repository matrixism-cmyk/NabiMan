//! The Claude Code sessions in the session list: the arrows between cards,
//! the cards of Claude sessions under no tile, and one session's header.

use actix_web::{web, HttpRequest, HttpResponse};
use std::collections::HashMap;

use super::super::session::PtyStore;

/// What travels along one arrow: the latest message's first line.
#[derive(serde::Serialize, Clone)]
pub(super) struct LinkNote {
    to: String,
    text: String,
    at: u64,
}

/// The notes for a card's arrows, named like its `directs`.
pub(super) fn notes_of(here: &super::super::claude_peers::Seen, id_of: &HashMap<&str, &str>, remote_ok: bool) -> Vec<LinkNote> {
    here.notes
        .iter()
        .filter_map(|(t, (at, text))| {
            let to = id_of.get(t.as_str()).map(|i| i.to_string()).or_else(|| (remote_ok && is_card(t)).then(|| t.clone()))?;
            Some(LinkNote { to, text: text.clone(), at: *at })
        })
        .collect()
}

/// Not a remote-only member (who sees only their own servers' shells).
pub(super) fn sees_remote_claude(user_id: &str, users: &crate::users::UserStore) -> bool {
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
pub(super) fn directs_of(here: &super::super::claude_peers::Seen, id_of: &HashMap<&str, &str>, remote_ok: bool) -> Vec<String> {
    here.leads
        .iter()
        .filter_map(|t| id_of.get(t.as_str()).map(|i| i.to_string()).or_else(|| (remote_ok && is_card(t)).then(|| t.clone())))
        .collect()
}

/// A Claude session with a card of its own rather than a tile: `rc:` on a
/// connected server, `lc:` on this machine (background sessions, other terminals).
fn is_card(id: &str) -> bool {
    id.starts_with("rc:") || id.starts_with("lc:")
}

/// A Claude session running under no tile: on a server the tiles are
/// connected to (started at that machine, say), or on this machine (a Claude
/// Code background session, or one in another terminal). It gets a card.
#[derive(serde::Serialize)]
struct RemoteClaudeCard {
    id: String,
    /// The saved server's name; empty for this machine.
    server: String,
    /// A Claude Code background session.
    background: bool,
    claude: Vec<super::super::claude_peers::ClaudePeer>,
    busy: bool,
    directs: Vec<String>,
    orchestrator: bool,
    notes: Vec<LinkNote>,
}

pub(super) async fn remote_claude(store: web::Data<PtyStore>, req: HttpRequest, users: web::Data<crate::users::UserStore>) -> HttpResponse {
    let user_id = crate::auth::verified_user_id(&req).unwrap_or_default();
    if !sees_remote_claude(&user_id, &users) {
        return HttpResponse::Ok().json(crate::models::ApiResponse::ok(Vec::<RemoteClaudeCard>::new()));
    }
    super::super::claude_remote::want(store.get_ref());
    let seen = super::super::claude_peers::look();
    let map = store.lock().unwrap();
    let id_of: HashMap<&str, &str> = map
        .iter()
        .filter(|(_, s)| super::super::session::visible_to(s, &user_id, &users))
        .map(|(id, s)| (s.tmux_name.as_str(), id.as_str()))
        .collect();
    let mut cards: Vec<RemoteClaudeCard> = seen
        .iter()
        .filter(|(k, h)| (k.starts_with("rc:") && h.ghost_server.is_some() || k.starts_with("lc:") && h.ghost_local) && !h.claude.is_empty())
        .map(|(k, h)| {
            let directs = directs_of(h, &id_of, true);
            RemoteClaudeCard {
                id: k.clone(),
                server: h.ghost_server.clone().unwrap_or_default(),
                background: h.background,
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
    claude: Vec<super::super::claude_peers::ClaudePeer>,
    busy: bool,
}

/// `GET …/sessions/{id}/claude`: the Claude sessions in one session, for its header.
pub(super) async fn session_claude(
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
                if super::super::session::visible_to(s, &user_id, &users)
                    && super::super::remote_access::allowed(s) =>
            {
                s.tmux_name.clone()
            }
            _ => {
                return HttpResponse::Ok()
                    .json(crate::models::ApiResponse::<()>::error("Session not found"))
            }
        }
    };
    let seen = super::super::claude_peers::look();
    let here = seen.get(&tmux_name);
    HttpResponse::Ok().json(crate::models::ApiResponse::ok(ClaudeInfo {
        claude: here.map(|h| h.claude.clone()).unwrap_or_default(),
        busy: here.is_some_and(|h| h.busy()),
    }))
}
