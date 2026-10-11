use super::{validate_url, Error, Forwarder};
use crate::models::ApiResponse;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
fn fail(error: Error) -> HttpResponse {
    let status = match error {
        Error::Invalid => actix_web::http::StatusCode::BAD_REQUEST,
        Error::Conflict => actix_web::http::StatusCode::CONFLICT,
        Error::Limit => actix_web::http::StatusCode::CONFLICT,
        _ => actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
    };
    HttpResponse::build(status)
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::<()>::error(error.message()))
}
fn ok(value: impl serde::Serialize) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(value))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    name: String,
    url: String,
    #[serde(default)]
    auth_header: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    revision: String,
}
async fn list(service: web::Data<Forwarder>) -> HttpResponse {
    let result=service.access(|s|Ok(serde_json::json!({"destinations":s.list()?,"counters":s.counters()?,"legacy_requires_review":service.0.legacy,"delivery_paused_by_operator":service.0.paused,"delivery":"at least once; HTTP 2xx acknowledgement is not proof of receiver durable storage","queue_limit":super::store::LIMIT})));
    match result {
        Ok(value) => ok(value),
        Err(error) => fail(error),
    }
}
async fn create(service: web::Data<Forwarder>, body: web::Json<Create>) -> HttpResponse {
    let body = body.into_inner();
    let name = body.name.trim();
    if name.is_empty()
        || name.chars().count() > 80
        || name.chars().any(char::is_control)
        || body.auth_header.len() > 4096
        || body.auth_header.chars().any(char::is_control)
    {
        return fail(Error::Invalid);
    }
    let url = match validate_url(&body.url) {
        Ok(url) => url,
        Err(e) => return fail(e),
    };
    if !body.auth_header.is_empty()
        && reqwest::header::HeaderValue::from_str(&body.auth_header).is_err()
    {
        return fail(Error::Invalid);
    }
    let auth = if body.auth_header.is_empty() {
        String::new()
    } else {
        match crate::secret_store::encrypt(&body.auth_header) {
            Ok(value) => value,
            Err(_) => return fail(Error::Credential),
        }
    };
    match service.access(|s| s.add(name, &url, &auth)) {
        Ok(id) => ok(serde_json::json!({"id":id,"enabled":false})),
        Err(e) => fail(e),
    }
}
async fn action(
    service: web::Data<Forwarder>,
    path: web::Path<(String, String)>,
    body: web::Json<Action>,
) -> HttpResponse {
    let (id, action) = path.into_inner();
    if !["enable", "disable", "delete", "retry"].contains(&action.as_str()) {
        return fail(Error::Invalid);
    }
    match service.access(|s| s.change(&id, &body.revision, &action)) {
        Ok(()) => ok("Destination updated; previously dispatched requests may finish"),
        Err(e) => fail(e),
    }
}
async fn test(
    service: web::Data<Forwarder>,
    path: web::Path<String>,
    body: web::Json<Action>,
) -> HttpResponse {
    let entry = crate::audit::AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        user: "forwarding-probe".into(),
        method: "POST".into(),
        path: "/api/audit/forwarding/probe".into(),
        status: 200,
        ip: String::new(),
        hash: None,
    };
    match service.access(|s| s.enqueue(&entry, Some((&path, &body.revision)))) {
        Ok(count) => HttpResponse::Accepted()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(
                serde_json::json!({"queued":count,"delivered":false}),
            )),
        Err(e) => fail(e),
    }
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/audit/forwarding")
            .route("", web::get().to(list))
            .route("", web::post().to(create))
            .route("/{id}/test", web::post().to(test))
            .route("/{id}/{action}", web::post().to(action)),
    );
}
