//! The user database with and without passkeys in it, across a restart.

use super::*;

/// A user file as written before passkeys existed (the `mfa` object has no
/// `passkeys`), and one older still (no `mfa` at all).
const BEFORE_PASSKEYS: &str = r#"[
  { "id": "u1", "username": "kim", "share_authorization_version": "v1", "password_hash": "$2b$04$x", "role": "admin",
    "status": "active", "memberships": [], "requested_memberships": [], "scope_restricted": false, "created_at": "2026-01-01 00:00:00",
    "last_login": null, "totp_enabled": true, "totp_secret": "JBSWY3DPEHPK3PXP",
    "mfa": { "recovery_hashes": ["aa"], "last_used_step": 5, "pending_expires_at": null, "revision": "r1" },
    "remote_permissions": {}, "access": {} },
  { "id": "u2", "username": "lee", "password_hash": "$2b$04$y", "role": "viewer", "created_at": "2025-01-01 00:00:00" }
]"#;

#[core::prelude::v1::test]
fn a_user_file_from_before_passkeys_still_loads() {
    let users: Vec<NabimanUser> = serde_json::from_str(BEFORE_PASSKEYS).unwrap();
    assert!(users.iter().all(|u| u.mfa.passkeys.is_empty()));
    assert_eq!(users[0].mfa.recovery_hashes, ["aa"]);

    // And through the server's own start-up path.
    let storage = crate::users::isolated_test_storage();
    std::fs::write(storage.file(), BEFORE_PASSKEYS).unwrap();
    let store = crate::users::new_user_store("");
    assert_eq!(store.lock_safe().len(), 2);
    assert!(store.lock_safe().iter().all(|u| u.mfa.passkeys.is_empty()));
}

#[actix_web::test]
async fn passkeys_survive_a_restart_and_still_sign_in() {
    let world = World::with(vec![user("u1", "kim", "admin"), user("u2", "lee", "viewer")]);
    world.update("kim", |u| u.share_authorization_version = "v1".into());
    world.update("lee", |u| u.share_authorization_version = "v2".into());
    let mut browser = Browser::at(ORIGIN);
    let id = {
        let app = serve!(world);
        let token = password_token(&app, "kim").await;
        let id = register(&app, &mut browser, &token, "보안 키").await;
        assert_eq!(sign_in(&app, &mut browser, &id).await.0, StatusCode::OK);
        id
    };

    // What is on disk: the passkey with its key, under the account's `mfa`.
    let path = crate::users::users_file_for_tests();
    let disk: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let kept = &disk[0]["mfa"]["passkeys"][0];
    assert_eq!(kept["name"], "보안 키");
    assert!(kept["credential"].is_object() && kept["last_used"].is_string());
    assert_eq!(disk[1]["mfa"]["passkeys"], json!([]));
    // What an API answer carries of an account: none of it.
    let api = serde_json::to_string(&world.account("kim")).unwrap();
    assert!(!api.contains("passkeys") && !api.contains("credential"), "{api}");

    // Restart: a new server reads that file, with nothing left in memory.
    let reloaded = crate::users::new_user_store("");
    assert_eq!(reloaded.lock_safe()[0].mfa.passkeys.len(), 1);
    let after = World {
        users: reloaded,
        sessions: crate::jwt_sessions::new_session_store(),
        passkeys: web::Data::new(Passkeys::from_setting(Some(ORIGIN), "NabiMan")),
        recorded: Recorded::default(),
        _storage: Box::new(()),
    };
    let app = serve!(after);
    let (status, body) = sign_in(&app, &mut browser, &id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["username"], "kim");
    // The signature counter kept counting across the restart, and was saved again.
    let again: Vec<NabimanUser> = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(again[0].mfa.passkeys.len(), 1);
    assert_eq!(sign_in(&app, &mut browser, &id).await.0, StatusCode::OK);
    drop(world);
}

#[actix_web::test]
async fn the_authenticator_app_settings_leave_passkeys_alone_but_a_password_reset_removes_them() {
    let world = World::with(vec![user("u1", "kim", "admin"), user("u2", "lee", "viewer")]);
    let app = serve!(world);
    let mut browser = Browser::at(ORIGIN);
    for name in ["kim", "lee"] {
        let token = password_token(&app, name).await;
        register(&app, &mut browser, &token, "key").await;
    }

    // Starting an authenticator-app enrollment replaces the rest of `mfa`.
    let lee = password_token(&app, "lee").await;
    let (status, _) = call(&app, "POST", "/api/auth/2fa/setup", Some(&lee), json!({ "password": PASSWORD })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(world.account("lee").mfa.recovery_hashes.len(), 10);
    assert_eq!(world.account("lee").mfa.passkeys.len(), 1);

    // An administrator resetting Lee's password: whoever knew the old one
    // may have added a passkey, so they go.
    let users = actix_web::web::Data::new(world.users.clone());
    let sessions = actix_web::web::Data::new(world.sessions.clone());
    let admin = test::init_service(
        actix_web::App::new()
            .wrap(crate::AuthCheck::new(std::sync::Arc::new("test-secret".to_string()), world.sessions.clone(), world.users.clone()))
            .app_data(users)
            .app_data(sessions)
            .configure(crate::users::config),
    )
    .await;
    let kim = password_token(&app, "kim").await;
    let reset = test::TestRequest::put()
        .uri("/api/users/u2")
        .insert_header(("Authorization", format!("Bearer {kim}")))
        .set_json(json!({ "password": "a new long password 1!" }))
        .to_request();
    let response = test::call_service(&admin, reset).await;
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["success"], true, "{body}");
    assert!(world.account("lee").mfa.passkeys.is_empty());
    assert_eq!(world.account("kim").mfa.passkeys.len(), 1, "only that account's");
}
