use super::*;
use std::sync::{Arc, Mutex};
use crate::models::{NabimanUser, UserRole};
use actix_web::{test, App};
fn account() -> NabimanUser {
    NabimanUser {
        id: "stable-id".into(),
        username: "member".into(),
        password_hash: "fixture".into(),
        role: UserRole::Viewer,
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: true,
        created_at: "fixture".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
        share_authorization_version: "fixture".into(),
    }
}
#[actix_web::test]
async fn rotation_preserves_deadline_and_replay_revokes_only_its_login() {
    let secret: JwtSecret = Arc::new("rotation-fixture-secret".into());
    let sessions = new_session_store();
    let users: UserStore = Arc::new(Mutex::new(vec![account()]));
    register_session(&sessions, "one", "member", "stable-id", "127.0.0.1");
    register_session(&sessions, "two", "member", "stable-id", "127.0.0.1");
    let deadline = chrono::Utc::now().timestamp() as usize + 120;
    let (_, original) = token_pair(&secret, "member", "viewer", "one", 0, deadline).unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(secret.clone()))
            .app_data(web::Data::new(sessions.clone()))
            .app_data(web::Data::new(users))
            .service(refresh_token),
    )
    .await;
    let request = |token: &str| {
        test::TestRequest::post()
            .uri("/api/auth/refresh")
            .set_json(serde_json::json!({"refresh_token":token}))
            .to_request()
    };
    // Invalid signatures must not let an unauthenticated caller revoke a family.
    assert_eq!(
        test::call_service(&app, request(&(original.clone() + "x")))
            .await
            .status(),
        401
    );
    assert_eq!(sessions.lock().unwrap().len(), 2);
    let response = test::call_service(&app, request(&original)).await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    let body: serde_json::Value = test::read_body_json(response).await;
    let rotated = body["data"]["refresh_token"].as_str().unwrap();
    assert_ne!(rotated, original);
    let claims = jsonwebtoken::decode::<RefreshClaims>(
        rotated,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    )
    .unwrap()
    .claims;
    assert_eq!(claims.generation, 1);
    assert_eq!(claims.exp, deadline);
    assert_eq!(
        test::call_service(&app, request(&original)).await.status(),
        401
    );
    assert!(!sessions.lock().unwrap().contains_key("one"));
    assert!(sessions.lock().unwrap().contains_key("two"));
    assert_eq!(
        test::call_service(&app, request(rotated)).await.status(),
        401
    );
}
#[actix_web::test]
async fn expiration_leeway_does_not_extend_the_refresh_deadline() {
    let secret: JwtSecret = Arc::new("expiry-fixture-secret".into());
    let sessions = new_session_store();
    register_session(&sessions, "one", "member", "stable-id", "127.0.0.1");
    let users: UserStore = Arc::new(Mutex::new(vec![account()]));
    let (_, expired) = token_pair(
        &secret,
        "member",
        "viewer",
        "one",
        0,
        chrono::Utc::now().timestamp() as usize - 1,
    )
    .unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(secret))
            .app_data(web::Data::new(sessions.clone()))
            .app_data(web::Data::new(users))
            .service(refresh_token),
    )
    .await;
    let request = test::TestRequest::post()
        .uri("/api/auth/refresh")
        .set_json(serde_json::json!({"refresh_token":expired}))
        .to_request();
    assert_eq!(test::call_service(&app, request).await.status(), 401);
    assert!(sessions.lock().unwrap().is_empty());
}
