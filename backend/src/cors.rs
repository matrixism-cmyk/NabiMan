//! Which browser origins may call the API (`NABIMAN_CORS_ORIGINS`).
//!
//! By default none but NabiMan's own: the SPA is served from the same
//! origin as the API, so it needs no CORS at all, and without this
//! middleware no `Access-Control-Allow-*` header is ever sent. Set
//! `NABIMAN_CORS_ORIGINS` to a comma-separated list of origins
//! (`https://ops.example.com`) to let other sites call it; `*` allows any
//! origin, deliberately.

use actix_cors::Cors;
use actix_web::middleware::Condition;

fn configured(origins: &str) -> Cors {
    let mut cors = Cors::default()
        .allow_any_method()
        .expose_headers(["X-Nabiman-MFA-Required"])
        .allowed_headers(["Content-Type", "Authorization", "X-Auth-Token", "X-API-Key", "X-File-Path"])
        .max_age(3600);
    for origin in origins.split(',').map(str::trim).filter(|o| !o.is_empty()) {
        cors = if origin == "*" { cors.allow_any_origin() } else { cors.allowed_origin(origin) };
    }
    cors
}

pub(crate) fn from_setting(origins: Option<&str>) -> Condition<Cors> {
    match origins.map(str::trim).filter(|o| !o.is_empty()) {
        Some(origins) => Condition::new(true, configured(origins)),
        None => Condition::new(false, Cors::default()),
    }
}

pub(crate) fn build_cors() -> Condition<Cors> {
    from_setting(std::env::var("NABIMAN_CORS_ORIGINS").ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test as awtest, web, App, HttpResponse};

    async fn allow_origin(setting: Option<&str>, origin: &str) -> Option<String> {
        let app = awtest::init_service(
            App::new().wrap(from_setting(setting)).route("/api/x", web::get().to(HttpResponse::Ok)),
        )
        .await;
        let res = awtest::call_service(&app, awtest::TestRequest::get().uri("/api/x").insert_header(("Origin", origin)).to_request()).await;
        res.headers().get("access-control-allow-origin").map(|v| v.to_str().unwrap().to_string())
    }

    #[actix_web::test]
    async fn no_setting_means_same_origin_only() {
        assert_eq!(allow_origin(None, "https://evil.example").await, None);
        assert_eq!(allow_origin(Some(" "), "https://evil.example").await, None);
    }

    #[actix_web::test]
    async fn listed_origins_and_only_those() {
        let set = Some("https://ops.example.com, https://b.example");
        assert_eq!(allow_origin(set, "https://ops.example.com").await.as_deref(), Some("https://ops.example.com"));
        assert_eq!(allow_origin(set, "https://evil.example").await, None);
        assert!(allow_origin(Some("*"), "https://any.example").await.is_some());
    }
}
