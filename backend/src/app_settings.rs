//! NabiMan's own settings (환경설정), starting with signing out after a time
//! without use.
//!
//! "Use" is what a person does — a click, a key, a touch, speech — reported
//! by the page (`POST /api/auth/activity`); the page's own polling is not
//! use. The page signs out on time and warns a minute before; the server
//! refuses a sign-in that has been idle too long, so a forgotten tab or a
//! copied token does not keep it alive.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::models::ApiResponse;

/// What may be chosen, in minutes; 0 is never.
pub const IDLE_CHOICES: [u32; 10] = [0, 5, 10, 15, 30, 60, 120, 240, 480, 1440];
/// Slack for the page's own timer and clocks that disagree a little.
const GRACE_SECS: i64 = 90;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct AppSettings {
    #[serde(default)]
    pub idle_logout_minutes: u32,
}

fn path() -> std::path::PathBuf {
    nabiman_server::data_dir().join("app_settings.json")
}

fn idle_minutes() -> &'static AtomicU32 {
    static IDLE: OnceLock<AtomicU32> = OnceLock::new();
    IDLE.get_or_init(|| AtomicU32::new(load().idle_logout_minutes))
}

pub fn load() -> AppSettings {
    std::fs::read(path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Whether a sign-in last used at `last_activity` has been idle too long.
pub fn idle_expired(last_activity: i64, now: i64) -> bool {
    let minutes = idle_minutes().load(Ordering::Relaxed);
    minutes > 0 && now - last_activity > i64::from(minutes) * 60 + GRACE_SECS
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn super_admin(req: &HttpRequest) -> Result<(), HttpResponse> {
    let users = req.app_data::<web::Data<crate::users::UserStore>>()
        .ok_or_else(|| HttpResponse::ServiceUnavailable().finish())?;
    let actor = crate::membership::actor(req, users)?;
    if actor.super_admin() { Ok(()) } else {
        Err(HttpResponse::Forbidden().json(ApiResponse::<()>::error("Only a super administrator can change NabiMan's settings")))
    }
}

/// Every signed-in page reads this: it needs to know when to sign out.
async fn get() -> HttpResponse {
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(AppSettings { idle_logout_minutes: idle_minutes().load(Ordering::Relaxed) }))
}

async fn put(req: HttpRequest, body: web::Json<AppSettings>) -> HttpResponse {
    if let Err(r) = super_admin(&req) {
        return r;
    }
    if !IDLE_CHOICES.contains(&body.idle_logout_minutes) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Choose one of the offered times"));
    }
    let json = match serde_json::to_vec_pretty(&*body) { Ok(j) => j, Err(e) => return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e.to_string())) };
    if let Err(e) = crate::private_storage::write(&path(), &json) {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&format!("Cannot save the settings: {e}")));
    }
    idle_minutes().store(body.idle_logout_minutes, Ordering::Relaxed);
    // Turning the limit on must not sign out whoever is using NabiMan right now.
    if let Some(sessions) = req.app_data::<web::Data<crate::jwt_sessions::SessionStore>>() {
        if let Some(sid) = crate::auth::verified_claims(&req).map(|c| c.sid) {
            if let Some(s) = sessions.lock().unwrap().get_mut(&sid) { s.last_activity = now(); }
        }
    }
    get().await
}

/// The person at this sign-in did something just now.
async fn activity(req: HttpRequest, sessions: web::Data<crate::jwt_sessions::SessionStore>) -> HttpResponse {
    let Some(sid) = crate::auth::verified_claims(&req).map(|c| c.sid) else {
        return HttpResponse::Unauthorized().finish();
    };
    if let Some(s) = sessions.lock().unwrap().get_mut(&sid) {
        s.last_activity = now();
    }
    HttpResponse::NoContent().finish()
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/app-settings", web::get().to(get))
        .route("/api/app-settings", web::put().to(put))
        .route("/api/auth/activity", web::post().to(activity));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_by_default_and_then_by_the_minute_with_some_slack() {
        idle_minutes().store(0, Ordering::Relaxed);
        assert!(!idle_expired(0, 10_000_000));
        idle_minutes().store(30, Ordering::Relaxed);
        assert!(!idle_expired(1000, 1000 + 30 * 60 + GRACE_SECS));
        assert!(idle_expired(1000, 1000 + 30 * 60 + GRACE_SECS + 1));
        idle_minutes().store(0, Ordering::Relaxed);
    }

    #[test]
    fn the_server_refuses_and_forgets_a_sign_in_idle_too_long() {
        let secret = "idle-fixture-secret";
        let sessions = crate::jwt_sessions::new_session_store();
        crate::jwt_sessions::register_session(&sessions, "busy", "admin", "u1", "127.0.0.1");
        crate::jwt_sessions::register_session(&sessions, "away", "admin", "u1", "127.0.0.1");
        sessions.lock().unwrap().get_mut("away").unwrap().last_activity = now() - 3600;
        let (busy, _) = crate::jwt_sessions::create_token_pair(secret, "admin", "admin", "busy").unwrap();
        let (away, _) = crate::jwt_sessions::create_token_pair(secret, "admin", "admin", "away").unwrap();
        idle_minutes().store(15, Ordering::Relaxed);
        let busy_ok = crate::auth::check_auth_token(secret, &busy, &sessions);
        let away_ok = crate::auth::check_auth_token(secret, &away, &sessions);
        idle_minutes().store(0, Ordering::Relaxed);
        assert!(busy_ok && !away_ok);
        assert!(!sessions.lock().unwrap().contains_key("away"), "an idle sign-in is over for good");
    }
}
