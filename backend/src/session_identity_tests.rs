use crate::{
    auth, jwt_sessions as sessions,
    models::{NabimanUser, UserRole},
    users::UserStore,
    AuthCheck,
};
use actix_web::{test, web, App, HttpResponse};
use std::sync::{Arc, Mutex};
fn user(name: &str, role: UserRole, restricted: bool) -> NabimanUser {
    NabimanUser {
        id: format!("{name}-id"),
        username: name.into(),
        password_hash: "fixture".into(),
        role,
        status: Default::default(),
        memberships: if restricted {
            vec![crate::models::Membership {
                kind: crate::models::ScopeKind::Tenant,
                scope_id: "tenant".into(),
                role: UserRole::Admin,
                enabled: true,
                allow_exec: None,
                expires_at: None,
            }]
        } else {
            vec![]
        },
        requested_memberships: vec![],
        scope_restricted: restricted,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
        remote_permissions: Default::default(),
        share_authorization_version: "fixture".into(),
    }
}
#[actix_web::test]
async fn late_session_registration_cannot_cross_deleted_and_recreated_account_identity() {
    let secret: auth::JwtSecret = Arc::new("fixture-session-identity-secret".into());
    let store = sessions::new_session_store();
    let users: UserStore = Arc::new(Mutex::new(vec![user("member", UserRole::Viewer, false)]));
    let (access, refresh) =
        sessions::create_token_pair(&secret, "member", "viewer", "old").unwrap();
    // Password verification started for the old ID, then delete/recreate completed
    // before that login finally registers. Username and session existence still match.
    users.lock().unwrap()[0].id = "replacement-id".into();
    users.lock().unwrap()[0].role = UserRole::Admin;
    sessions::register_session(&store, "old", "member", "member-id", "127.0.0.1");
    let app = test::init_service(
        App::new()
            .wrap(AuthCheck::new(secret.clone(), store.clone(), users.clone()))
            .app_data(web::Data::new(secret.clone()))
            .app_data(web::Data::new(store.clone()))
            .app_data(web::Data::new(users))
            .configure(sessions::config)
            .route(
                "/api/users/me",
                web::get().to(|| async { HttpResponse::Ok().finish() }),
            ),
    )
    .await;
    for path in ["/api/users/me", "/api/auth/sessions"] {
        let req = test::TestRequest::get()
            .uri(path)
            .insert_header(("Authorization", format!("Bearer {access}")))
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status(), 401);
    }
    let req = test::TestRequest::post()
        .uri("/api/auth/refresh")
        .set_json(serde_json::json!({"refresh_token":refresh}))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), 401);
    sessions::register_session(&store, "new", "member", "replacement-id", "127.0.0.1");
    let (fresh, _) = sessions::create_token_pair(&secret, "member", "admin", "new").unwrap();
    let req = test::TestRequest::get()
        .uri("/api/users/me")
        .insert_header(("Authorization", format!("Bearer {fresh}")))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), 200);
}
#[actix_web::test]
async fn session_privacy_and_revocation_are_account_scoped_for_every_role() {
    let secret: auth::JwtSecret = Arc::new("fixture-session-scope-secret".into());
    let store = sessions::new_session_store();
    let users: UserStore = Arc::new(Mutex::new(vec![
        user("root", UserRole::Admin, false),
        user("manager", UserRole::Viewer, true),
        user("viewer", UserRole::Viewer, false),
    ]));
    let mut tokens = std::collections::HashMap::new();
    for name in ["root", "manager", "viewer"] {
        for suffix in ["current", "other"] {
            let sid = format!("{name}-{suffix}");
            sessions::register_session(&store, &sid, name, &format!("{name}-id"), "127.0.0.1");
            tokens.insert(
                sid.clone(),
                sessions::create_token_pair(&secret, name, "admin", &sid)
                    .unwrap()
                    .0,
            );
        }
    }
    let app = test::init_service(
        App::new()
            .wrap(AuthCheck::new(secret, store.clone(), users.clone()))
            .app_data(web::Data::new(store.clone()))
            .app_data(web::Data::new(users))
            .configure(sessions::config),
    )
    .await;
    for name in ["root", "manager", "viewer"] {
        let token = &tokens[&format!("{name}-current")];
        let req = test::TestRequest::get()
            .uri("/api/auth/sessions")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), 200);
        assert_eq!(res.headers().get("cache-control").unwrap(), "no-store");
        let body: serde_json::Value = test::read_body_json(res).await;
        let rows = body["data"].as_array().unwrap();
        assert_eq!(rows.len(), if name == "root" { 6 } else { 2 });
        assert_eq!(rows.iter().filter(|r| r["is_current"] == true).count(), 1);
        if name != "root" {
            assert!(rows.iter().all(|r| r["username"] == name));
        }
    }
    for name in ["manager", "viewer"] {
        let token = &tokens[&format!("{name}-current")];
        for target in ["root-current", "nonexistent"] {
            let req = test::TestRequest::post()
                .uri("/api/auth/sessions/revoke")
                .insert_header(("Authorization", format!("Bearer {token}")))
                .set_json(serde_json::json!({"session_id":target}))
                .to_request();
            assert_eq!(test::call_service(&app, req).await.status(), 404);
        }
        let req = test::TestRequest::post()
            .uri("/api/auth/sessions/revoke-others")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status(), 200);
        assert!(store
            .lock()
            .unwrap()
            .contains_key(&format!("{name}-current")));
        assert!(!store.lock().unwrap().contains_key(&format!("{name}-other")));
        assert!(store.lock().unwrap().contains_key("root-other"));
    }
    let req = test::TestRequest::post()
        .uri("/api/auth/sessions/revoke")
        .insert_header((
            "Authorization",
            format!("Bearer {}", tokens["root-current"]),
        ))
        .set_json(serde_json::json!({"session_id":"manager-current"}))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), 200);
    assert!(!store.lock().unwrap().contains_key("manager-current"));
}

#[test]
async fn authentication_snapshot_rejects_password_mfa_and_authority_changes_before_registration() {
    let original = user("member", UserRole::Viewer, false);
    for change in ["password", "mfa", "authority", "identity"] {
        let users: UserStore = Arc::new(Mutex::new(vec![original.clone()]));
        let store = sessions::new_session_store();
        {
            let mut current = users.lock().unwrap();
            match change {
                "password" => current[0].password_hash = "changed".into(),
                "mfa" => current[0].totp_enabled = true,
                "authority" => current[0].share_authorization_version = "changed".into(),
                _ => current[0].id = "replacement-id".into(),
            }
        }
        assert!(!sessions::register_authenticated_session(
            &store,
            &users,
            "late",
            &original,
            false,
            "127.0.0.1"
        ));
        assert!(store.lock().unwrap().is_empty());
        let current = users.lock().unwrap()[0].clone();
        assert!(sessions::register_authenticated_session(
            &store,
            &users,
            "fresh",
            &current,
            false,
            "127.0.0.1"
        ));
    }
}
