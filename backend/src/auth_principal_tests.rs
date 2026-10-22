use super::*;
use crate::models::{NabimanUser, UserRole};
use std::sync::{Arc, Mutex};

#[actix_web::test]
async fn authenticated_identity_ignores_spoofed_headers_and_unverified_claims() {
    let secret: auth::JwtSecret = Arc::new("principal-test-secret".into());
    let sessions = jwt_sessions::new_session_store();
    jwt_sessions::register_session(
        &sessions,
        "principal-session",
        "operator",
        "operator-id",
        "127.0.0.1",
    );
    let users = Arc::new(Mutex::new(vec![NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: "operator-id".into(),
        username: "operator".into(),
        password_hash: "hash".into(),
        role: UserRole::Operator,
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: false,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
        remote_permissions: Default::default(),
        access: Default::default(),
    }]));
    let claims = auth::Claims {
        sub: "operator".into(),
        role: "admin".into(),
        sid: "principal-session".into(),
        iat: chrono::Utc::now().timestamp() as usize,
        exp: (chrono::Utc::now().timestamp() + 300) as usize,
    };
    let sign = |key: &[u8]| {
        jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(key),
        )
        .unwrap()
    };
    let token = sign(secret.as_bytes());
    let app = actix_web::test::init_service(
        App::new()
            .wrap(AuthCheck::new(secret, sessions, users.clone()))
            .app_data(web::Data::new(users))
            .app_data(web::Data::new(mec::MecState::mocks()))
            .configure(license::config)
            .route(
                "/api/mec/v1/jobs/{id}/recovery",
                web::get().to(mec::handlers::jobs::recovery),
            )
            .route(
                "/api/mec/v1/actor",
                web::get().to(|req: actix_web::HttpRequest| async move {
                    match mec::handlers::util::current_user(&req) {
                        Ok(user) => HttpResponse::Ok().json(user),
                        Err(response) => response,
                    }
                }),
            )
            .route(
                "/api/auth/refresh",
                web::get().to(|req: actix_web::HttpRequest| async move {
                    HttpResponse::Ok().json(auth::extract_username(&req))
                }),
            ),
    )
    .await;
    for header in ["Authorization", "X-Auth-Token"] {
        let value = if header == "Authorization" {
            format!("Bearer {token}")
        } else {
            token.clone()
        };
        let req = actix_web::test::TestRequest::get()
            .uri("/api/mec/v1/actor")
            .insert_header((header, value))
            .insert_header(("X-User", "admin"))
            .to_request();
        let res = actix_web::test::call_service(&app, req).await;
        assert_eq!(res.status(), 200);
        let actor: String = actix_web::test::read_body_json(res).await;
        assert_eq!(actor, "operator");
    }
    let denied = actix_web::test::TestRequest::get()
        .uri("/api/terminal")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("X-User", "admin"))
        .to_request();
    let denied = actix_web::test::call_service(&app, denied).await;
    assert_eq!(denied.status(), 403);
    assert_eq!(
        auth::verified_role(denied.request()),
        Some(UserRole::Operator)
    );
    assert_eq!(
        auth::extract_username(denied.request()).as_deref(),
        Some("operator")
    );
    let recovery = actix_web::test::TestRequest::get()
        .uri("/api/mec/v1/jobs/nonexistent/recovery")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("X-User", "admin"))
        .to_request();
    assert_eq!(
        actix_web::test::call_service(&app, recovery).await.status(),
        403
    );
    let license = actix_web::test::TestRequest::post()
        .uri("/api/license/activate")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("X-User", "admin"))
        .set_json(serde_json::json!({"key": "untrusted-key"}))
        .to_request();
    assert_eq!(
        actix_web::test::call_service(&app, license).await.status(),
        403
    );
    let forged = sign(b"wrong-key");
    let req = actix_web::test::TestRequest::get()
        .uri("/api/mec/v1/actor")
        .insert_header(("Authorization", format!("Bearer {forged}")))
        .insert_header(("X-User", "admin"))
        .to_request();
    assert_eq!(actix_web::test::call_service(&app, req).await.status(), 401);
    let req = actix_web::test::TestRequest::get()
        .uri("/api/auth/refresh")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let actor: Option<String> =
        actix_web::test::read_body_json(actix_web::test::call_service(&app, req).await).await;
    assert!(
        actor.is_none(),
        "Public routes must not acquire identity from an unchecked token"
    );
}
