use super::*;
use crate::models::{NabimanUser, UserRole};
use std::sync::{Arc, Mutex};

#[actix_web::test]
async fn second_factor_alone_cannot_issue_tokens() {
    let _storage = crate::users::isolated_test_storage();
    let secret = base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &[7u8; 20]);
    let code = make_totp(&secret).unwrap().generate_current().unwrap();
    let users: UserStore = Arc::new(Mutex::new(vec![NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: "1".into(),
        username: "admin".into(),
        password_hash: bcrypt::hash("correct-password", 4).unwrap(),
        role: UserRole::Admin,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: true,
        totp_secret: Some(secret),
        mfa: Default::default(),
        remote_permissions: Default::default(),
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: false,
    }]));
    let jwt_secret: crate::auth::JwtSecret = Arc::new("test-secret".into());
    let sessions = crate::jwt_sessions::new_session_store();
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .app_data(web::Data::new(users))
            .app_data(web::Data::new(jwt_secret))
            .app_data(web::Data::new(sessions.clone()))
            .service(verify_login),
    )
    .await;
    let denied_request = actix_web::test::TestRequest::post()
        .uri("/api/auth/2fa/verify")
        .set_json(serde_json::json!({"username":"admin","password":"wrong","code":code}))
        .to_request();
    let denied = actix_web::test::call_service(&app, denied_request).await;
    assert_eq!(denied.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    assert!(sessions.lock().unwrap().is_empty());

    let allowed_request = actix_web::test::TestRequest::post()
        .uri("/api/auth/2fa/verify")
        .set_json(serde_json::json!({"username":"admin","password":"correct-password","code":code}))
        .to_request();
    let allowed = actix_web::test::call_service(&app, allowed_request).await;
    assert_eq!(allowed.status(), actix_web::http::StatusCode::OK);
    assert_eq!(sessions.lock().unwrap().len(), 1);
}

#[actix_web::test]
async fn active_second_factor_cannot_be_replaced_without_disabling_it() {
    let original_secret = base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &[9u8; 20]);
    let users: UserStore = Arc::new(Mutex::new(vec![NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: "1".into(),
        username: "admin".into(),
        password_hash: "hash".into(),
        role: UserRole::Admin,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: true,
        totp_secret: Some(original_secret.clone()),
        mfa: Default::default(),
        remote_permissions: Default::default(),
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: false,
    }]));
    let claims = crate::auth::Claims {
        sub: "admin".into(),
        role: "admin".into(),
        sid: "session".into(),
        iat: chrono::Utc::now().timestamp() as usize,
        exp: (chrono::Utc::now().timestamp() + 300) as usize,
    };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(b"test"),
    )
    .unwrap();
    let sessions = crate::jwt_sessions::new_session_store();
    crate::jwt_sessions::register_session(&sessions, "session", "admin", "1", "127.0.0.1");
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .wrap(crate::AuthCheck::new(
                Arc::new("test".into()),
                sessions,
                users.clone(),
            ))
            .app_data(web::Data::new(users.clone()))
            .service(super::settings::setup_2fa),
    )
    .await;
    let request = actix_web::test::TestRequest::post()
        .uri("/api/auth/2fa/setup")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let response = actix_web::test::call_service(&app, request).await;
    assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
    let users = users.lock().unwrap();
    assert!(users[0].totp_enabled);
    assert_eq!(
        users[0].totp_secret.as_deref(),
        Some(original_secret.as_str())
    );
}

#[test]
fn authenticator_steps_and_recovery_codes_are_consumed_once() {
    let mut user = fixture_user();
    let now = 1_900_000_000;
    let code = make_totp(user.totp_secret.as_deref().unwrap())
        .unwrap()
        .generate(now);
    assert!(consume_totp(&mut user, &code, now));
    assert!(!consume_totp(&mut user, &code, now));
    let (codes, hashes) = recovery_codes();
    assert_eq!(
        codes.iter().collect::<std::collections::HashSet<_>>().len(),
        10
    );
    user.mfa.recovery_hashes = hashes;
    assert!(consume_factor(&mut user, &codes[0].to_uppercase(), now));
    assert!(!consume_factor(&mut user, &codes[0], now));
    assert_eq!(user.mfa.recovery_hashes.len(), 9);
    let api = serde_json::to_string(&user).unwrap();
    assert!(!api.contains("recovery_hashes") && !api.contains("totp_secret"));
    assert!(codes.iter().all(|code| !api.contains(code)));
}
fn fixture_user() -> crate::models::NabimanUser {
    crate::models::NabimanUser {
        id: "factor-id".into(),
        username: "factor".into(),
        password_hash: bcrypt::hash("fixture-password", 4).unwrap(),
        role: crate::models::UserRole::Viewer,
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: true,
        created_at: "fixture".into(),
        last_login: None,
        totp_enabled: true,
        totp_secret: Some(base32::encode(
            base32::Alphabet::Rfc4648 { padding: false },
            &[5u8; 20],
        )),
        mfa: Default::default(),
        remote_permissions: Default::default(),
        share_authorization_version: "fixture".into(),
    }
}
#[actix_web::test]
async fn persistence_failure_does_not_consume_factor_or_issue_login() {
    use std::sync::{Arc, Mutex};
    let storage = crate::users::isolated_test_storage();
    std::fs::create_dir(storage.file()).unwrap();
    let user = fixture_user();
    let code = make_totp(user.totp_secret.as_deref().unwrap())
        .unwrap()
        .generate_current()
        .unwrap();
    let users: UserStore = Arc::new(Mutex::new(vec![user]));
    let sessions = crate::jwt_sessions::new_session_store();
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .app_data(web::Data::new(users.clone()))
            .app_data(web::Data::new(sessions.clone()))
            .app_data(web::Data::new(Arc::new("test-secret".to_string())))
            .service(verify_login),
    )
    .await;
    let request = || {
        actix_web::test::TestRequest::post()
            .uri("/api/auth/2fa/verify")
            .set_json(
                serde_json::json!({"username":"factor","password":"fixture-password","code":code}),
            )
            .to_request()
    };
    assert_eq!(
        actix_web::test::call_service(&app, request())
            .await
            .status(),
        500
    );
    assert!(users.lock().unwrap()[0].mfa.last_used_step.is_none());
    assert!(sessions.lock().unwrap().is_empty());
    std::fs::remove_dir(storage.file()).unwrap();
    assert_eq!(
        actix_web::test::call_service(&app, request())
            .await
            .status(),
        200
    );
    let persisted: Vec<crate::models::NabimanUser> =
        serde_json::from_str(&std::fs::read_to_string(storage.file()).unwrap()).unwrap();
    assert!(persisted[0].mfa.last_used_step.is_some());
    assert_eq!(
        actix_web::test::call_service(&app, request())
            .await
            .status(),
        401
    );
    assert_eq!(sessions.lock().unwrap().len(), 1);
}
