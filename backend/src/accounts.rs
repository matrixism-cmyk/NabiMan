//! Local OS accounts, distinct from NabiMan members. Legacy root execution remains explicit.
pub(crate) mod inventory;
mod operations;
#[cfg(test)]
mod tests;
use crate::models::{
    ApiResponse, ChangePasswordRequest, CreateAccountRequest, DeleteAccountRequest,
};
use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use operations::Operations;
use std::sync::atomic::{AtomicBool, Ordering};
static WRITING: AtomicBool = AtomicBool::new(false);
struct WritePermit;
impl Drop for WritePermit {
    fn drop(&mut self) {
        WRITING.store(false, Ordering::Release);
    }
}
fn admin(req: &HttpRequest, users: &crate::users::UserStore) -> bool {
    let id = crate::auth::verified_user_id(req);
    users.lock().ok().is_some_and(|u| {
        u.iter()
            .any(|u| Some(u.id.as_str()) == id.as_deref() && u.active() && u.super_admin())
    })
}
fn denied() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::<()>::error(
        "Highest administrator required for local OS account changes",
    ))
}
fn invalid() -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::<()>::error("Use a lowercase POSIX account name (1–32 characters), and a 12–256 byte password without control characters"))
}
async fn dispatch(
    req: &HttpRequest,
    users: &crate::users::UserStore,
    work: impl FnOnce(Operations) -> Result<String, String> + Send + 'static,
) -> HttpResponse {
    let Some(sessions) = req.app_data::<web::Data<crate::jwt_sessions::SessionStore>>() else {
        return denied();
    };
    let Some(grant) = crate::terminal_tickets::ConnectionGrant::from_request(
        req,
        sessions.get_ref().clone(),
        users.clone(),
    ) else {
        return denied();
    };
    let expires = crate::auth::verified_claims(req)
        .map(|c| c.exp)
        .unwrap_or(0);
    let method = req.method().as_str().to_string();
    if unsafe { libc::geteuid() } != 0 {
        return HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
            "Local OS account writer requires root; a privileged account helper is not configured",
        ));
    }
    if WRITING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return HttpResponse::Conflict().json(ApiResponse::<()>::error(
            "Another local account operation is in progress; inspect its result before retrying",
        ));
    }
    let permit = WritePermit;
    // The worker owns admission until completion even if the HTTP caller disconnects.
    match web::block(move || {let _permit=permit;{
            let mut ops=Operations::system();
            ops.authorize=Box::new(move || chrono::Utc::now().timestamp() < expires as i64 && grant.valid_for_method(&method));
            work(ops)
        }}).await {
        Ok(Ok(message))=>HttpResponse::Ok().insert_header(("Cache-Control","no-store")).json(ApiResponse::ok(message)),
        Ok(Err(message))=>HttpResponse::Conflict().insert_header(("Cache-Control","no-store")).json(ApiResponse::<()>::error(&message)),
        Err(_)=>HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Local account worker result unavailable; outcome unknown. Inspect local state before retrying")),
    }
}
#[get("/api/accounts")]
async fn list_accounts() -> HttpResponse {
    match web::block(|| Operations::system().inventory()).await {
        Ok(Ok(rows)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(
                rows.into_iter()
                    .filter(|r| r.uid >= 1000 || r.uid == 0)
                    .collect::<Vec<_>>(),
            )),
        _ => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
            "Local account inventory is unreadable or malformed; no empty-list fallback",
        )),
    }
}
#[get("/api/accounts/options")]
async fn options(req: HttpRequest, users: web::Data<crate::users::UserStore>) -> HttpResponse {
    let can_write = admin(&req, &users) && unsafe { libc::geteuid() } == 0;
    match inventory::shells(&Operations::system().shells) {
        Ok(shells)=>HttpResponse::Ok().insert_header(("Cache-Control","no-store")).json(ApiResponse::ok(serde_json::json!({"shells":shells,"can_write":can_write,"home_base":"/home","uid_min":1000,"uid_max":65533,"login_status":"not_collected"}))),
        Err(message)=>HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(&message)),
    }
}
#[post("/api/accounts")]
async fn create_account(
    req: HttpRequest,
    body: web::Json<CreateAccountRequest>,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    if !admin(&req, &users) {
        return denied();
    }
    if !inventory::name(&body.username) || !inventory::password(&body.password) {
        return invalid();
    }
    let body = body.into_inner();
    dispatch(&req, &users, move |ops| ops.create(body)).await
}
#[post("/api/accounts/password")]
async fn change_password(
    req: HttpRequest,
    body: web::Json<ChangePasswordRequest>,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    if !admin(&req, &users) {
        return denied();
    }
    if !inventory::name(&body.username) || !inventory::password(&body.new_password) {
        return invalid();
    }
    let body = body.into_inner();
    dispatch(&req, &users, move |ops| ops.password(body)).await
}
#[delete("/api/accounts")]
async fn delete_account(
    req: HttpRequest,
    body: web::Json<DeleteAccountRequest>,
    users: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    if !admin(&req, &users) {
        return denied();
    }
    if !inventory::name(&body.username) {
        return invalid();
    }
    let body = body.into_inner();
    dispatch(&req, &users, move |ops| ops.delete(body)).await
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(list_accounts)
        .service(options)
        .service(create_account)
        .service(change_password)
        .service(delete_account);
}
