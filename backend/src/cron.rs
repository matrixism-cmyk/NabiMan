//! Local crontabs only; application jobs and tenant schedules are separate.
mod document;
mod operations;

#[cfg(test)]
mod tests;
use crate::models::{ApiResponse, CreateCronRequest, DeleteCronRequest, UpdateCronRequest};
use actix_web::{web, HttpRequest, HttpResponse};
use operations::Operations;
use std::sync::atomic::{AtomicBool, Ordering};
static WRITING: AtomicBool = AtomicBool::new(false);
struct Permit;
impl Drop for Permit {
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
async fn get_cron_jobs() -> HttpResponse {
    match web::block(|| Operations::system().list()).await {
        Ok(Ok(rows)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(rows)),
        _ => HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
            "Cron inventory unavailable; existing jobs must not be treated as absent",
        )),
    }
}
#[derive(serde::Deserialize)]
struct Query {
    user: String,
}
async fn snapshot(
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
    query: web::Query<Query>,
) -> HttpResponse {
    let user = query.into_inner().user;
    let can_write = admin(&req, &users) && unsafe { libc::geteuid() } == 0;
    match web::block(move||Operations::system().snapshot(&user)).await{
  Ok(Ok((row,content)))=>HttpResponse::Ok().insert_header(("Cache-Control","no-store")).json(ApiResponse::ok(serde_json::json!({"user":row.username,"uid":row.uid,"revision":document::revision(&row.username,row.uid,&content),"content":content,"can_write":can_write&&row.can_manage}))),
  _=>HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error("Crontab snapshot unavailable; no change is permitted"))
 }
}
async fn change(
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
    user: String,
    revision: String,
    edit: impl FnOnce(&str, u32) -> Result<String, String> + Send + 'static,
) -> HttpResponse {
    if !admin(&req, &users) {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error(
            "Highest administrator required for local scheduled commands",
        ));
    }
    if !crate::accounts::inventory::name(&user)
        || revision.len() != 64
        || !revision.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "A valid account and reviewed crontab revision are required",
        ));
    }
    if unsafe { libc::geteuid() } != 0 {
        return HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(
            "Privileged cron helper not configured",
        ));
    }
    let Some(sessions) = req.app_data::<web::Data<crate::jwt_sessions::SessionStore>>() else {
        return HttpResponse::Forbidden().finish();
    };
    let Some(grant) = crate::terminal_tickets::ConnectionGrant::from_request(
        &req,
        sessions.get_ref().clone(),
        users.get_ref().clone(),
    ) else {
        return HttpResponse::Forbidden().finish();
    };
    let expires = crate::auth::verified_claims(&req)
        .map(|c| c.exp)
        .unwrap_or(0);
    if WRITING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return HttpResponse::Conflict().json(ApiResponse::<()>::error(
            "A local cron change is in progress; reload before retrying",
        ));
    }
    let permit = Permit;
    match web::block(move || {
        let _permit = permit;
        let mut ops = Operations::system();
        ops.authorize = Box::new(move || {
            chrono::Utc::now().timestamp() < expires as i64 && grant.valid_for_method("POST")
        });
        ops.change(&user, &revision, edit)
    })
    .await
    {
        Ok(Ok(message)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(message)),
        Ok(Err(message)) => HttpResponse::Conflict()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::<()>::error(&message)),
        Err(_) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
            "Cron worker result unavailable; outcome unknown. Reload before retrying",
        )),
    }
}
async fn add_cron_job(
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
    body: web::Json<CreateCronRequest>,
) -> HttpResponse {
    let b = body.into_inner();
    change(
        req,
        users,
        b.user,
        b.expected_revision,
        move |content, _| document::append(content, &b.schedule, &b.command),
    )
    .await
}
async fn delete_cron_job(
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
    body: web::Json<DeleteCronRequest>,
) -> HttpResponse {
    let b = body.into_inner();
    let user = b.user.clone();
    change(
        req,
        users,
        b.user,
        b.expected_revision,
        move |content, uid| document::remove(&user, uid, content, b.id),
    )
    .await
}
async fn update_cron_job(
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
    body: web::Json<UpdateCronRequest>,
) -> HttpResponse {
    let b = body.into_inner();
    let user = b.user.clone();
    change(
        req,
        users,
        b.user,
        b.expected_revision,
        move |content, uid| document::update(&user, uid, content, b.id, &b.schedule, &b.command),
    )
    .await
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/cron")
            .route("", web::get().to(get_cron_jobs))
            .route("/snapshot", web::get().to(snapshot))
            .route("/add", web::post().to(add_cron_job))
            .route("/update", web::post().to(update_cron_job))
            .route("/delete", web::post().to(delete_cron_job)),
    );
}
