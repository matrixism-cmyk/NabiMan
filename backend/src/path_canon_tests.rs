//! The role check and the router must judge the same path. These tests run
//! requests through the real `AuthCheck` middleware and actix router, with
//! probe handlers mounted where the real ones are registered.
use super::*;
use crate::models::{NabimanUser, UserRole};
use actix_web::http::StatusCode;
use actix_web::test::{call_service, init_service, read_body, TestRequest};
use std::sync::{Arc, Mutex};

const SECRET: &str = "path-canon-test-secret";

fn user(name: &str, role: UserRole) -> NabimanUser {
    let mut u: NabimanUser = serde_json::from_value(serde_json::json!({
        "id": format!("{name}-id"), "username": name, "password_hash": "hash",
        "role": "viewer", "created_at": "now", "status": "active"
    }))
    .unwrap();
    u.role = role;
    u
}

fn token(name: &str) -> String {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = auth::Claims {
        sub: name.into(), role: "ignored".into(), sid: format!("{name}-sid"), iat: now, exp: now + 300,
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .unwrap()
}

async fn reached() -> HttpResponse {
    HttpResponse::Ok().body("reached")
}

macro_rules! probe_app {
    () => {{
        let sessions = jwt_sessions::new_session_store();
        let mut people = Vec::new();
        for (name, role) in [("viewer", UserRole::Viewer), ("operator", UserRole::Operator), ("member", UserRole::Remote)] {
            jwt_sessions::register_session(&sessions, &format!("{name}-sid"), name, &format!("{name}-id"), "127.0.0.1");
            people.push(user(name, role));
        }
        let users: users::UserStore = Arc::new(Mutex::new(people));
        init_service(
            App::new()
                .wrap(AuthCheck::new(Arc::new(SECRET.into()), sessions, users))
                .route("/api/terminal", web::get().to(reached))
                .route("/api/terminal/sessions/{id}/share", web::post().to(reached))
                .route("/api/server/status", web::get().to(reached))
                .service(web::scope("/api/files").route("/download", web::get().to(reached)))
                .service(web::scope("/api/backup").route("/restore", web::post().to(reached)))
                .service(web::scope("/api/audit").route("", web::get().to(reached)))
                .service(web::scope("/api/config").route("/{service_id}", web::get().to(reached)))
                .service(web::scope("/api/users").route("", web::post().to(reached))),
        )
        .await
    }};
}

fn request(who: Option<&str>, method: &str, uri: &str) -> TestRequest {
    let req = match method {
        "POST" => TestRequest::post(),
        _ => TestRequest::get(),
    }
    .uri(uri);
    match who {
        Some(name) => req.insert_header(("Authorization", format!("Bearer {}", token(name)))),
        None => req,
    }
}

/// (account, method, canonical path the account is denied, other spellings
/// of that path that the router resolves to the same handler or nearby).
fn denied_cases() -> Vec<(Option<&'static str>, &'static str, &'static str, Vec<&'static str>)> {
    vec![
        (Some("viewer"), "GET", "/api/terminal", vec!["/api/%74erminal", "/%61pi/terminal", "/api//terminal", "/api/./terminal", "/api/x/../terminal", "//api/terminal", "/API/terminal", "/api/terminal/"]),
        (Some("viewer"), "GET", "/api/files/download", vec!["/api/%66iles/download", "/api/files/%64ownload", "/api/files%2Fdownload", "/api/files//download"]),
        (Some("viewer"), "GET", "/api/config/mysql", vec!["/api/%63onfig/mysql", "/api/config/./mysql"]),
        (Some("viewer"), "GET", "/api/audit", vec!["/api/%61udit", "/api/aud%69t"]),
        (Some("operator"), "GET", "/api/terminal", vec!["/api/%74erminal", "/api/t%65rminal"]),
        (Some("operator"), "POST", "/api/backup/restore", vec!["/api/%62ackup/restore", "/api/backup/%72estore"]),
        (Some("operator"), "POST", "/api/users", vec!["/api/%75sers", "/api/user%73"]),
        (Some("member"), "POST", "/api/terminal/sessions/one/share", vec!["/api/terminal/sessions/one/%73hare"]),
        (None, "GET", "/api/server/status", vec!["/%61pi/server/status", "/%61pi/terminal"]),
    ]
}

#[actix_web::test]
async fn non_canonical_spellings_never_reach_a_denied_handler() {
    let app = probe_app!();
    let mut reached_by = Vec::new();
    for (who, method, canonical, variants) in denied_cases() {
        let res = call_service(&app, request(who, method, canonical).to_request()).await;
        assert_ne!(res.status(), StatusCode::OK, "{who:?} {method} {canonical} is meant to be denied");
        for uri in variants {
            let res = call_service(&app, request(who, method, uri).to_request()).await;
            if res.status() == StatusCode::OK && read_body(res).await == "reached" {
                reached_by.push(format!("{who:?} {method} {uri}"));
            }
        }
    }
    assert!(reached_by.is_empty(), "denied handlers reached: {reached_by:#?}");
}

#[actix_web::test]
async fn non_canonical_api_paths_are_refused_before_any_credential() {
    let app = probe_app!();
    for (who, method, _, variants) in denied_cases() {
        for uri in variants.into_iter().filter(|u| !u.starts_with("/API/") && !u.ends_with('/')) {
            let res = call_service(&app, request(who, method, uri).to_request()).await;
            assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{who:?} {method} {uri}");
        }
    }
    // A WebSocket upgrade and a public-API key are judged at the same gate.
    let ws = TestRequest::get()
        .uri("/api/%74erminal?ticket=x")
        .insert_header(("Authorization", format!("Bearer {}", token("viewer"))))
        .insert_header(("Connection", "Upgrade"))
        .insert_header(("Upgrade", "websocket"))
        .insert_header(("Sec-WebSocket-Version", "13"))
        .insert_header(("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ=="))
        .to_request();
    assert_eq!(call_service(&app, ws).await.status(), StatusCode::BAD_REQUEST);
    let key = TestRequest::get()
        .uri("/api/%73erver/status")
        .insert_header(("X-API-Key", format!("nbm_{}", "a".repeat(64))))
        .to_request();
    assert_eq!(call_service(&app, key).await.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn canonical_requests_are_unchanged() {
    let app = probe_app!();
    let ok = call_service(&app, request(Some("viewer"), "GET", "/api/server/status").to_request()).await;
    assert_eq!(ok.status(), StatusCode::OK);
    // Escapes a client legitimately sends (encodeURIComponent never escapes
    // unreserved characters) still reach the handler, judged as decoded.
    let encoded = call_service(&app, request(Some("operator"), "GET", "/api/config/my%20sql").to_request()).await;
    assert_eq!(encoded.status(), StatusCode::FORBIDDEN);
    let anon = call_service(&app, request(None, "GET", "/api/server/status").to_request()).await;
    assert_eq!(anon.status(), StatusCode::UNAUTHORIZED);
    // Paths outside the API (the web app's own) are not judged here.
    let page = call_service(&app, request(None, "GET", "/%61pps/x").to_request()).await;
    assert_eq!(page.status(), StatusCode::NOT_FOUND);
}
