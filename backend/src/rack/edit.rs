//! `POST /api/rack/layout/moves`: an administrator moved devices on the
//! page and saves. The body is `{base_version, moves: [{id, rack, u}]}`;
//! the answer is the layout as `GET /api/rack/layout` gives it, with the
//! new `version` and the new file's name. See `save` for what is written.
//!
//! Administrators only, signed in themselves: operators, viewers, members
//! and scoped accounts are stopped by the route's class before they get
//! here, API keys cannot write at all, and the handler checks again.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use super::layout::Layout;
use super::moves::Move;
use super::save::{self, Failure, Saved};
use super::Service;
use crate::lock_safe::LockExt;
use crate::models::{ApiResponse, UserRole};

/// Saves one account may make in `WINDOW` (each one is a new file).
pub(super) const MAX_SAVES: usize = 30;
pub(super) const WINDOW: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
pub(super) struct MovesRequest {
    pub base_version: String,
    pub moves: Vec<Move>,
}

#[derive(Serialize)]
struct SavedAnswer {
    configured: bool,
    layout: Layout,
    error: Option<()>,
    version: String,
    file: String,
}

impl Service {
    /// Save the moves as a new version of the layout file. One save at a
    /// time in this process; other writers are kept apart by the file names.
    pub(crate) fn save_moves(&self, base_version: &str, moves: &[Move], by: &str) -> Result<Saved, Failure> {
        if !self.enabled() {
            return Err(Failure::Off);
        }
        let Some(path) = &self.layout_file else {
            return Err(Failure::Unusable { code: "no_file", detail: "NABIMAN_RACK_LAYOUT_FILE (rack_layout_file) is not set".into() });
        };
        let _one_at_a_time = self.saving.lock_safe();
        let saved = save::save(path, base_version, moves, by, chrono::Utc::now());
        match &saved {
            Ok(s) => log_info!("rack", "rack layout saved as a new version", version = s.version, file = s.file, moved = s.moved.len(), by = by),
            Err(Failure::Storage(why)) => log_warn!("rack", "the rack layout could not be saved", reason = why),
            Err(_) => {}
        }
        saved
    }
}

fn refusal(status: StatusCode, code: &str, message: &str, more: serde_json::Value) -> HttpResponse {
    let mut data = serde_json::json!({ "code": code });
    if let (Some(data), Some(more)) = (data.as_object_mut(), more.as_object()) {
        data.extend(more.clone());
    }
    HttpResponse::build(status)
        .insert_header(("Cache-Control", "no-store"))
        .json(serde_json::json!({ "success": false, "code": code, "data": data, "message": message }))
}

/// The answer for a save that did not happen.
pub(super) fn failed(failure: &Failure) -> HttpResponse {
    let none = serde_json::Value::Null;
    match failure {
        Failure::Off => refusal(StatusCode::SERVICE_UNAVAILABLE, "not_configured", "The rack page is not configured on this server", none),
        Failure::Unusable { code, detail } => refusal(StatusCode::UNPROCESSABLE_ENTITY, "layout_unusable", &format!("The rack layout file cannot be used: {detail}"), serde_json::json!({ "fault": code })),
        Failure::Conflict { current } => refusal(
            StatusCode::CONFLICT,
            "version_conflict",
            &format!("The layout was saved as version {current} meanwhile; nothing was written"),
            serde_json::json!({ "current_version": current }),
        ),
        Failure::Refused(r) => refusal(StatusCode::BAD_REQUEST, "invalid_move", &r.detail, serde_json::json!({ "reason": r.reason })),
        Failure::Storage(why) => refusal(StatusCode::INTERNAL_SERVER_ERROR, "write_failed", &format!("The layout was not saved: {why}"), none),
    }
}

/// Whether `who` may save now; counts the save when so.
pub(super) fn admitted(recent: &Mutex<HashMap<String, Vec<Instant>>>, who: &str, now: Instant) -> bool {
    let mut recent = recent.lock_safe();
    recent.retain(|_, times| {
        times.retain(|at| now.duration_since(*at) < WINDOW);
        !times.is_empty()
    });
    let times = recent.entry(who.to_string()).or_default();
    if times.len() >= MAX_SAVES {
        return false;
    }
    times.push(now);
    true
}

fn position(rack: &str, u: Option<u32>) -> String {
    u.map_or_else(|| format!("{rack} (no slot)"), |u| format!("{rack} U{u}"))
}

async fn post_moves(req: HttpRequest, body: web::Json<MovesRequest>) -> HttpResponse {
    static RECENT: OnceLock<Arc<Mutex<HashMap<String, Vec<Instant>>>>> = OnceLock::new();
    // The route's class already keeps everyone else out; an API key's owner may be an administrator, the key may not write.
    let admin = crate::auth::verified_role(&req) == Some(UserRole::Admin) && crate::auth::token_audit_actor(&req).is_none();
    let Some(by) = crate::auth::extract_username(&req).filter(|_| admin) else {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error("Only administrators can move devices"));
    };
    if !admitted(RECENT.get_or_init(Default::default), &by, Instant::now()) {
        return HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", WINDOW.as_secs().to_string()))
            .json(ApiResponse::<()>::error("Too many saves; try again in a minute"));
    }
    let request = body.into_inner();
    let who = by.clone();
    let saved = web::block(move || super::service().save_moves(&request.base_version, &request.moves, &who)).await;
    let saved = match saved {
        Ok(Ok(saved)) => saved,
        Ok(Err(failure)) => return failed(&failure),
        Err(e) => return failed(&Failure::Storage(e.to_string())),
    };
    // What moved, never the file's contents.
    let mut change = crate::audit_change::Change::new("rack_layout", &saved.version, "move");
    for m in &saved.moved {
        change = change.field("position", Some(&format!("{} @ {}", m.id, position(&m.from_rack, m.from_u))), Some(&format!("{} @ {}", m.id, position(&m.to_rack, Some(m.to_u)))));
    }
    crate::audit_change::note(&req, change);
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(SavedAnswer { configured: true, layout: saved.layout, error: None, version: saved.version, file: saved.file }))
}

pub(super) fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/rack/layout/moves", web::post().to(post_moves));
}
