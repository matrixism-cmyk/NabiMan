//! `/api/license`: the licence's state, with what the plan counts now and
//! what the screen and the banner should warn about.
use super::plan::{self, Counts};
use super::{data_dir, key_path, load_keys, status_at, verify, LicenseStatus, Stored};
use crate::lock_safe::LockExt;
use crate::models::ApiResponse;
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

static WRITES: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivateRequest {
    pub key: String,
}

#[derive(Serialize)]
struct View {
    #[serde(flatten)]
    status: LicenseStatus,
    /// Present with a plan: what its limits count now.
    usage: Option<Counts>,
    notices: Vec<&'static str>,
}

/// Users with an active account and saved servers, as the limits count them.
pub(crate) fn counts(users: &crate::users::UserStore) -> Counts {
    Counts {
        users: plan::seats(&users.lock_safe()),
        servers: crate::remote_servers::load_servers().map_or(0, |s| s.len()),
    }
}

fn view(status: LicenseStatus, users: &crate::users::UserStore) -> View {
    // Nothing is counted (or read) without a plan: the answer is what it always was.
    let usage = status.limits.is_some().then(|| counts(users));
    let notices = usage.as_ref().map_or_else(Vec::new, |c| plan::notices(&status, c));
    View { status, usage, notices }
}

async fn get_status(users: web::Data<crate::users::UserStore>) -> HttpResponse {
    let status = status_at(&data_dir(), &key_path(), Utc::now());
    HttpResponse::Ok().json(ApiResponse::ok(view(status, &users)))
}
async fn get_features() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::ok(
        status_at(&data_dir(), &key_path(), Utc::now()).features,
    ))
}
async fn activate(
    req: HttpRequest,
    users: web::Data<crate::users::UserStore>,
    body: web::Json<ActivateRequest>,
) -> HttpResponse {
    if let Err(r) = crate::membership::require_super(&req, &users) {
        return r;
    }
    let keys = match load_keys(&key_path()) {
        Ok(keys) => keys,
        Err(e) => return HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(&e)),
    };
    let key = body.key.trim();
    if let Err(e) = verify(key, &keys, Utc::now().date_naive()) {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(&e));
    }
    let _guard = WRITES.lock_safe();
    let stored = serde_json::to_vec(&Stored {
        signed_key: key.into(),
    })
    .expect("Stored license serialization");
    if crate::private_storage::write(&data_dir().join("license.json"), &stored).is_err() {
        return HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error("Cannot persist signed license"));
    }
    let status = status_at(&data_dir(), &key_path(), Utc::now());
    HttpResponse::Ok().json(ApiResponse::ok(view(status, &users)))
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/license")
            .route("", web::get().to(get_status))
            .route("/activate", web::post().to(activate))
            .route("/features", web::get().to(get_features)),
    );
}
