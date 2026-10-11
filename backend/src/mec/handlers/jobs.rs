use super::util::{from_service_error, list_response, ok_response};
use crate::mec::MecState;
use crate::models::mec::JobStatusResponse;
use actix_web::{web, HttpResponse};

pub async fn list(state: web::Data<MecState>) -> HttpResponse {
    match state.job_store.list_recent(50) {
        Ok(jobs) => {
            let out: Vec<JobStatusResponse> =
                jobs.iter().map(JobStatusResponse::from_job).collect();
            list_response(out)
        }
        Err(e) => from_service_error(e.into()),
    }
}

pub async fn get(path: web::Path<String>, state: web::Data<MecState>) -> HttpResponse {
    match state.job_store.get(&path) {
        Ok(Some(j)) => ok_response(JobStatusResponse::from_job(&j)),
        Ok(None) => from_service_error(crate::mec::services::ServiceError::NotFound(format!(
            "job {}",
            &path
        ))),
        Err(e) => from_service_error(e.into()),
    }
}

pub async fn recovery(
    req: actix_web::HttpRequest,
    path: web::Path<String>,
    users: web::Data<crate::users::UserStore>,
    state: web::Data<MecState>,
) -> HttpResponse {
    if let Err(response) = crate::membership::require_super(&req, &users) {
        return response;
    }
    let user = match super::util::current_user(&req) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let audit =
        super::audit_helper::begin_audit(&req, &state, &user, "job_recovery_inspect", "job", &path);
    match crate::mec::recovery::inspect(&state, &path).await {
        Ok(report) => {
            audit.success(Some(serde_json::json!({"tenant_id": report.tenant_id, "records_conflict": report.records_conflict})));
            ok_response(report)
        }
        Err(e) => {
            audit.failure(e.to_string());
            from_service_error(e)
        }
    }
}
