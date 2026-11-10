//! The HTTP side of the orchestration channel: a tile's status and its close
//! for administrators (the same lookups as `nabiman-claude status`/`close`),
//! and the orchestration guide for the dashboard's "AI 안내" button.

use crate::lock_safe::LockExt;

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use super::claude_control::{self as control, GUIDE};
use super::claude_control_ops::{close_for, status_for};
use super::session::PtyStore;
use crate::models::ApiResponse;
use crate::users::UserStore;

#[derive(Deserialize)]
struct StatusQuery {
    lines: Option<usize>,
}

#[derive(Deserialize)]
struct CloseQuery {
    force: Option<String>,
}

fn signed_in(req: &HttpRequest) -> Option<String> {
    crate::auth::verified_user_id(req).filter(|u| !u.is_empty())
}

/// A remote-only member (or an account that is gone): the member rules
/// would let one reach `/sessions/{id}/…`, but these are for administrators.
fn member(users: &UserStore, account: &str) -> bool {
    users.lock_safe().iter().find(|u| u.id == account).is_none_or(crate::member_servers::is_member)
}

fn for_members() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::<()>::error("Not available to this account"))
}

fn code(status: u16) -> actix_web::http::StatusCode {
    actix_web::http::StatusCode::from_u16(status).unwrap_or(actix_web::http::StatusCode::BAD_REQUEST)
}

/// `GET /api/terminal/sessions/{id}/status?lines=N`
async fn status(path: web::Path<String>, query: web::Query<StatusQuery>, req: HttpRequest, store: web::Data<PtyStore>, users: web::Data<UserStore>) -> HttpResponse {
    let Some(account) = signed_in(&req) else {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Not signed in"));
    };
    if member(&users, &account) {
        return for_members();
    }
    let (store, users, id, lines) = (store.get_ref().clone(), users.get_ref().clone(), path.into_inner(), query.lines);
    match web::block(move || status_for(&store, &users, &account, &id, lines)).await {
        Ok(Ok(s)) => HttpResponse::Ok().json(ApiResponse::ok(s)),
        Ok(Err(e)) => HttpResponse::NotFound().json(ApiResponse::<()>::error(&e)),
        Err(_) => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error("status stopped unexpectedly")),
    }
}

/// `POST /api/terminal/sessions/{id}/close?force=1` — the audit middleware
/// records it with the signed-in administrator.
async fn close(path: web::Path<String>, query: web::Query<CloseQuery>, req: HttpRequest, store: web::Data<PtyStore>, users: web::Data<UserStore>) -> HttpResponse {
    let Some(account) = signed_in(&req) else {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Not signed in"));
    };
    if member(&users, &account) {
        return for_members();
    }
    let force = query.force.as_deref().is_some_and(|f| f == "1" || f == "true");
    let (store, users, id) = (store.get_ref().clone(), users.get_ref().clone(), path.into_inner());
    // A browser is in no tile, so there is no session of its own to spare.
    match web::block(move || close_for(&store, &users, &account, &id, None, force)).await {
        Ok(Ok(t)) => HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({"session_id": t.id, "title": t.title}))),
        Ok(Err((_, (status, why)))) => HttpResponse::build(code(status)).json(ApiResponse::<()>::error(&why)),
        Err(_) => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error("close stopped unexpectedly")),
    }
}

#[derive(Deserialize)]
struct GuideQuery {
    session: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Lead {
    pub address: String,
    /// The lead tile's name.
    pub tile: String,
}

#[derive(Serialize, Debug)]
struct Guide {
    text: String,
    lead: Option<Lead>,
    updated: Option<String>,
}

/// Who the pasted guide is for: two or three lines on top of it.
pub(crate) fn header(title: &str, id: &str, claude: Option<&str>, lead: Option<&Lead>) -> String {
    let mut h = format!("> 이 안내는 NabiMan 세션 «{title}» ({id}) 에 보낸 것입니다.\n");
    match claude {
        Some(a) => h.push_str(&format!("> 이 세션의 Claude 주소: `{a}`\n")),
        None => h.push_str("> 이 세션의 Claude 주소는 `nabiman-claude list` 로 확인하세요.\n"),
    }
    if let Some(l) = lead {
        h.push_str(&format!("> 총괄(lead) 주소: `{}` (타일 {})\n", l.address, l.tile));
    }
    h.push('\n');
    h
}

/// `GET /api/orchestration/guide[?session=pty_…]` — any signed-in account
/// but a remote-only member's.
async fn guide(query: web::Query<GuideQuery>, req: HttpRequest, store: web::Data<PtyStore>, users: web::Data<UserStore>) -> HttpResponse {
    let Some(account) = signed_in(&req) else {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Not signed in"));
    };
    if member(&users, &account) {
        return for_members();
    }
    let (store, users) = (store.get_ref().clone(), users.get_ref().clone());
    let asked = query.session.clone();
    let built = web::block(move || {
        // tmux and /proc are asked with no lock held.
        let seen = super::claude_peers::look();
        let lead_tmux = super::claude_lead::lead_tmux();
        let map = store.lock_safe().clone();
        let lead = lead_tmux.and_then(|t| {
            let tile = map.values().find(|s| s.tmux_name == t)?;
            let address = seen.get(&t)?.claude.first()?.address.clone();
            Some(Lead { address, tile: if tile.title.is_empty() { tile.label.clone() } else { tile.title.clone() } })
        });
        let mut text = GUIDE.to_string();
        if let Some(id) = asked {
            let s = map.get(&id).filter(|s| super::session::visible_to(s, &account, &users)).ok_or(())?;
            let title = if s.title.is_empty() { &s.label } else { &s.title };
            let claude = seen.get(&s.tmux_name).and_then(|h| h.claude.first()).map(|p| p.address.as_str());
            text = header(title, &id, claude, lead.as_ref()) + &text;
        }
        Ok::<_, ()>(Guide { text, lead, updated: control::guide_updated(GUIDE).map(str::to_string) })
    })
    .await;
    match built {
        Ok(Ok(g)) => HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(g)),
        Ok(Err(())) => HttpResponse::NotFound().json(ApiResponse::<()>::error("Session not found")),
        Err(_) => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error("guide stopped unexpectedly")),
    }
}

pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/sessions/{id}/status", web::get().to(status))
        .route("/api/terminal/sessions/{id}/close", web::post().to(close))
        .route("/api/orchestration/guide", web::get().to(guide));
}
