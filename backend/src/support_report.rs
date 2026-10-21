//! Explicitly selected diagnostics only: no raw logs, credentials, names, addresses or paths.
#[path = "support_report_collect.rs"]
mod collect;
pub(crate) use collect::bounded;
use crate::extension_points::ExtensionPoints;
use crate::{auth, jwt_sessions::SessionStore, models::ApiResponse, users::UserStore};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum State {
    Ready,
    Busy,
    Unavailable,
    TooLarge,
}
#[derive(Serialize)]
pub(crate) struct Section<T: Serialize> {
    state: State,
    data: Option<T>,
}
impl<T: Serialize> Section<T> {
    pub(crate) fn ready(data: T) -> Self {
        Self {
            state: State::Ready,
            data: Some(data),
        }
    }
    pub(crate) fn missing(state: State) -> Self {
        Self { state, data: None }
    }
}
#[derive(Serialize)]
struct Report {
    schema_version: u32,
    report_id: String,
    generated_at: String,
    collection_ms: u128,
    runtime: crate::runtime_identity::RuntimeIdentity,
    process: collect::Process,
    accounts: Section<collect::Accounts>,
    login_sessions: Section<collect::Logins>,
    /// What a product built on NabiMan adds (Remote: terminals and servers).
    #[serde(flatten)]
    parts: serde_json::Map<String, serde_json::Value>,
    auth_storage: Section<collect::Storage>,
    recent_jobs: Section<collect::Jobs>,
    limitations: [&'static str; 4],
}
static COLLECTING: AtomicBool = AtomicBool::new(false);
static LAST_COLLECTION: Mutex<Option<Instant>> = Mutex::new(None);
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        COLLECTING.store(false, Ordering::Release);
    }
}
fn acquire() -> Option<Permit> {
    if COLLECTING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return None;
    }
    let permit = Permit;
    let mut last = LAST_COLLECTION.lock().ok()?;
    if last.is_some_and(|at| at.elapsed() < Duration::from_secs(5)) {
        return None;
    }
    *last = Some(Instant::now());
    Some(permit)
}
fn authorize(
    req: &HttpRequest,
    users: &UserStore,
    sessions: &SessionStore,
) -> Result<(), HttpResponse> {
    let denied =
        || HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Authentication required"));
    let claims = auth::verified_claims(req).ok_or_else(denied)?;
    let id = auth::verified_user_id(req).ok_or_else(denied)?;
    let users = users.try_lock().map_err(|_| unavailable())?;
    let user = users
        .iter()
        .find(|u| u.id == id && u.username == claims.sub && u.active())
        .ok_or_else(denied)?;
    if !user.super_admin() {
        return Err(HttpResponse::Forbidden()
            .json(ApiResponse::<()>::error("Highest administrator required")));
    }
    let sessions = sessions.try_lock().map_err(|_| unavailable())?;
    if claims.exp as u128 <= chrono::Utc::now().timestamp().max(0) as u128
        || !sessions
            .get(&claims.sid)
            .is_some_and(|s| s.user_id == id && s.username == claims.sub)
    {
        return Err(denied());
    }
    Ok(())
}
fn unavailable() -> HttpResponse {
    HttpResponse::ServiceUnavailable()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::<()>::error(
            "Support report unavailable; try again",
        ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectRequest {}
async fn report(
    _body: web::Json<CollectRequest>,
    req: HttpRequest,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
    state: web::Data<crate::mec::MecState>,
    identity: web::Data<crate::runtime_identity::RuntimeIdentity>,
) -> HttpResponse {
    if let Err(response) = authorize(&req, &users, &sessions) {
        return response;
    }
    let Some(permit) = acquire() else {
        return HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", "5"))
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::<()>::error(
                "Wait a few seconds before collecting another report",
            ));
    };
    let (account_store, session_store) = (users.clone(), sessions.clone());
    let points = ExtensionPoints::of(req.app_data::<web::Data<ExtensionPoints>>());
    let parts: Vec<_> = points.report_parts.iter().map(|(name, part)| (*name, part(&req))).collect();
    let result = web::block(move || {
        let _permit = permit;
        let started = Instant::now();
        let mut report = Report {
            schema_version: 1,
            report_id: uuid::Uuid::new_v4().to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            collection_ms: 0,
            runtime: identity.get_ref().clone(),
            process: collect::process(),
            accounts: collect::accounts(&account_store),
            login_sessions: collect::logins(&session_store),
            parts: parts.into_iter().map(|(name, collect)| (name.to_string(), collect())).collect(),
            auth_storage: collect::storage(),
            recent_jobs: collect::jobs(&state.db),
            limitations: [
                "point_in_time_sections_not_atomic",
                "inventory_is_not_live_connectivity",
                "not_database_integrity_or_security_attestation",
                "latest_1000_jobs_only",
            ],
        };
        report.collection_ms = started.elapsed().as_millis();
        report
    })
    .await;
    if let Err(response) = authorize(&req, &users, &sessions) {
        return response;
    }
    match result {
        Ok(report) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .json(ApiResponse::ok(report)),
        Err(_) => unavailable(),
    }
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/support-report", web::post().to(report));
}
