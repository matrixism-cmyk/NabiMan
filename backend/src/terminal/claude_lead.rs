//! The Claude session the operator made the lead of all the others (총괄):
//! every link between it and another session is drawn from it, whoever
//! wrote first, since the others' reports to it are not it being led.

use crate::lock_safe::LockExt;
use std::sync::Mutex;

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use super::session::PtyStore;
use crate::models::ApiResponse;

/// The lead's tmux session name, as read from disk (`None` until first read).
static LEAD: Mutex<Option<Option<String>>> = Mutex::new(None);

fn path() -> std::path::PathBuf {
    nabiman_server::data_dir().join("claude-lead")
}

/// The lead's tmux session name, if one is set.
pub(crate) fn lead_tmux() -> Option<String> {
    let mut slot = LEAD.lock_safe();
    if slot.is_none() {
        let read = std::fs::read_to_string(path()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        *slot = Some(read);
    }
    slot.clone().flatten()
}

fn set_lead(tmux: Option<&str>) -> std::io::Result<()> {
    match tmux {
        Some(t) => crate::private_storage::write(&path(), t.as_bytes())?,
        None => {
            let _ = std::fs::remove_file(path());
        }
    }
    *LEAD.lock_safe() = Some(tmux.map(str::to_string));
    Ok(())
}

#[derive(Deserialize)]
struct LeadBody {
    /// The NabiMan session to make the lead; null to have none.
    session_id: Option<String>,
}

#[derive(Serialize)]
struct LeadInfo {
    session_id: Option<String>,
}

fn lead_id(store: &PtyStore) -> Option<String> {
    let tmux = lead_tmux()?;
    store.lock_safe().iter().find(|(_, s)| s.tmux_name == tmux).map(|(id, _)| id.clone())
}

async fn get_lead(store: web::Data<PtyStore>) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(LeadInfo { session_id: lead_id(store.get_ref()) }))
}

/// `PUT /api/terminal/claude-lead`: a super administrator names the lead session.
async fn put_lead(
    body: web::Json<LeadBody>,
    store: web::Data<PtyStore>,
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let actor = match crate::membership::actor(&req, &users) {
        Ok(a) => a,
        Err(r) => return r,
    };
    if !actor.super_admin() {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error("Only a super administrator names the lead session"));
    }
    let tmux = match &body.session_id {
        None => None,
        Some(id) => match store.lock_safe().get(id) {
            Some(s) => Some(s.tmux_name.clone()),
            None => return HttpResponse::Ok().json(ApiResponse::<()>::error("Session not found")),
        },
    };
    if let Err(e) = set_lead(tmux.as_deref()) {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e.to_string()));
    }
    super::claude_peers::forget();
    HttpResponse::Ok().json(ApiResponse::ok(LeadInfo { session_id: body.session_id.clone() }))
}

pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/terminal/claude-lead", web::get().to(get_lead))
        .route("/api/terminal/claude-lead", web::put().to(put_lead));
}
