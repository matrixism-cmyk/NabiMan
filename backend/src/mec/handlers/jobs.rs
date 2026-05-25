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
        Ok(None) => from_service_error(
            crate::mec::services::ServiceError::NotFound(format!("job {}", &path)),
        ),
        Err(e) => from_service_error(e.into()),
    }
}
