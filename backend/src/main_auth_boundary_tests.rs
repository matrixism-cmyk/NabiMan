use super::*;
use crate::models::{NabimanUser, UserRole};
use std::sync::{Arc, Mutex};

#[actix_web::test]
async fn viewer_cannot_reach_shell_or_root_file_download() {
    let secret: auth::JwtSecret = Arc::new("test-secret".into());
    let sessions = jwt_sessions::new_session_store();
    jwt_sessions::register_session(
        &sessions,
        "viewer-session",
        "viewer",
        "viewer-id",
        "127.0.0.1",
    );
    let users: users::UserStore = Arc::new(Mutex::new(vec![NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: "viewer-id".into(),
        username: "viewer".into(),
        password_hash: "hash".into(),
        role: UserRole::Viewer,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: false,
    }]));
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = auth::Claims {
        sub: "viewer".into(),
        role: "viewer".into(),
        sid: "viewer-session".into(),
        iat: now,
        exp: now + 300,
    };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();
    let app = actix_web::test::init_service(
        App::new()
            .wrap(AuthCheck::new(secret, sessions, users))
            .route(
                "/api/terminal",
                web::get().to(|| async { HttpResponse::Ok().finish() }),
            )
            .route(
                "/api/files/download",
                web::get().to(|| async { HttpResponse::Ok().finish() }),
            )
            .route(
                "/api/server/status",
                web::get().to(|| async { HttpResponse::Ok().finish() }),
            ),
    )
    .await;
    for path in ["/api/terminal", "/api/files/download"] {
        let request = actix_web::test::TestRequest::get()
            .uri(path)
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let response = actix_web::test::call_service(&app, request).await;
        assert_eq!(response.status(), actix_web::http::StatusCode::FORBIDDEN);
    }
    let request = actix_web::test::TestRequest::get()
        .uri("/api/server/status")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let response = actix_web::test::call_service(&app, request).await;
    assert_eq!(response.status(), actix_web::http::StatusCode::OK);
}
