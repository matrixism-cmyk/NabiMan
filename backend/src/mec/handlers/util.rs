use crate::mec::services::ServiceError;
use crate::models::mec::{MecError, MecListResponse, MecResponse};
use actix_web::{HttpResponse, HttpRequest};
use serde::Serialize;

pub fn ok_response<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Ok().json(MecResponse::ok(data))
}

pub fn list_response<T: Serialize>(data: Vec<T>) -> HttpResponse {
    HttpResponse::Ok().json(MecListResponse::new(data))
}

pub fn created<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Created().json(MecResponse::ok(data))
}

#[allow(dead_code)]
pub fn accepted<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Accepted().json(MecResponse::ok(data))
}

pub fn no_content() -> HttpResponse {
    HttpResponse::NoContent().finish()
}

pub fn from_service_error(err: ServiceError) -> HttpResponse {
    let e = MecError::with_details(err.code(), &err.to_string(), serde_json::json!({
        "kind": err.code()
    }));
    let body = MecResponse::<serde_json::Value>::error(e);
    let status = err.http_status();
    let builder = match status {
        400 => HttpResponse::BadRequest(),
        401 => HttpResponse::Unauthorized(),
        403 => HttpResponse::Forbidden(),
        404 => HttpResponse::NotFound(),
        409 => HttpResponse::Conflict(),
        422 => HttpResponse::UnprocessableEntity(),
        502 => HttpResponse::BadGateway(),
        503 => HttpResponse::ServiceUnavailable(),
        _ => HttpResponse::InternalServerError(),
    };
    let mut b = builder;
    b.json(body)
}

pub fn current_user(req: &HttpRequest) -> String {
    req.headers()
        .get("X-User")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("admin")
        .to_string()
}

pub fn source_ip(req: &HttpRequest) -> Option<String> {
    req.connection_info()
        .realip_remote_addr()
        .map(|s| s.to_string())
}

pub fn user_agent(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_mapping_status() {
        let r = from_service_error(ServiceError::NotFound("x".into()));
        assert_eq!(r.status(), 404);
    }

    #[test]
    fn error_mapping_conflict() {
        let r = from_service_error(ServiceError::Conflict("x".into()));
        assert_eq!(r.status(), 409);
    }
}
