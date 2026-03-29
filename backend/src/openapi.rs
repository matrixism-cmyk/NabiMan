use actix_web::{web, HttpResponse};

async fn openapi_spec() -> HttpResponse {
    let spec = include_str!("openapi_spec.json");
    HttpResponse::Ok()
        .insert_header(("Content-Type", "application/json"))
        .body(spec)
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/openapi.json", web::get().to(openapi_spec));
}
