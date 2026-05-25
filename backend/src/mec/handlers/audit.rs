use super::util::{from_service_error, list_response, ok_response};
use crate::mec::MecState;
use crate::models::mec::AuditQuery;
use actix_web::{web, HttpResponse};

pub async fn list(
    query: web::Query<AuditQuery>,
    state: web::Data<MecState>,
) -> HttpResponse {
    match state.audit_store.query(&query) {
        Ok(v) => list_response(v),
        Err(e) => from_service_error(e.into()),
    }
}

pub async fn get(path: web::Path<String>, state: web::Data<MecState>) -> HttpResponse {
    match state.audit_store.get(&path) {
        Ok(Some(v)) => ok_response(v),
        Ok(None) => from_service_error(
            crate::mec::services::ServiceError::NotFound(format!("audit {}", &path)),
        ),
        Err(e) => from_service_error(e.into()),
    }
}
