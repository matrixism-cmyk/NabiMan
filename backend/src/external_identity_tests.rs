use super::*;
use crate::{
    auth, jwt_sessions, mec,
    models::{AccountStatus, NabimanUser, UserRole},
    AuthCheck,
};
use std::sync::{Arc, Mutex};
fn member(id: &str, role: UserRole, status: AccountStatus) -> NabimanUser {
    NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: id.into(),
        username: id.into(),
        password_hash: "unused".into(),
        role,
        status,
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: false,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
    }
}
#[actix_web::test]
async fn explicit_link_requires_approval_super_admin_realm_and_atomic_audit() {
    let secret: auth::JwtSecret = Arc::new("identity-fixture-key".into());
    let sessions = jwt_sessions::new_session_store();
    let users = Arc::new(Mutex::new(vec![
        member("admin", UserRole::Admin, AccountStatus::Active),
        member("alice", UserRole::Viewer, AccountStatus::Active),
        member("bob", UserRole::Viewer, AccountStatus::Active),
        member("pending", UserRole::Viewer, AccountStatus::Pending),
    ]));
    let tokens: Vec<_> = ["admin", "alice"]
        .iter()
        .map(|name| {
            jwt_sessions::register_session(&sessions, name, name, name, "127.0.0.1");
            jsonwebtoken::encode(
                &jsonwebtoken::Header::default(),
                &auth::Claims {
                    sub: (*name).into(),
                    role: "admin".into(),
                    sid: (*name).into(),
                    iat: chrono::Utc::now().timestamp() as usize,
                    exp: (chrono::Utc::now().timestamp() + 300) as usize,
                },
                &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
            )
            .unwrap()
        })
        .collect();
    let state = web::Data::new(mec::MecState::mocks());
    // The usernames intentionally differ. Linking uses stable IDs, never names.
    let remote = state
        .services
        .rancher
        .create_user("external-alice", "fixture-password")
        .await
        .unwrap();
    let other = state
        .services
        .rancher
        .create_user("external-bob", "fixture-password")
        .await
        .unwrap();
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .wrap(AuthCheck::new(secret, sessions.clone(), users.clone()))
            .app_data(web::Data::new(users.clone()))
            .app_data(web::Data::new(sessions.clone()))
            .app_data(state.clone())
            .configure(config)
            .configure(crate::users::config),
    )
    .await;
    let url = |id: &str| format!("/api/users/{id}/external-identities/rancher");
    for (id, token, status) in [
        ("alice", tokens[1].as_str(), 403),
        ("pending", tokens[0].as_str(), 409),
        ("missing", tokens[0].as_str(), 404),
        ("alice", tokens[0].as_str(), 200),
    ] {
        let req = actix_web::test::TestRequest::get()
            .uri(&url(id))
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let res = actix_web::test::call_service(&app, req).await;
        assert_eq!(res.status().as_u16(), status);
        if status == 200 {
            let body: serde_json::Value = actix_web::test::read_body_json(res).await;
            assert_eq!(body["data"]["realm"], "mock://rancher");
            assert!(body["data"]["link"].is_null());
        }
    }
    for (id, realm, remote_id, reason, status) in [
        (
            "alice",
            "https://other",
            remote.id.as_str(),
            "Verified account ownership",
            409,
        ),
        (
            "alice",
            "mock://rancher",
            "../users",
            "Verified account ownership",
            400,
        ),
        ("alice", "mock://rancher", remote.id.as_str(), "short", 400),
        (
            "pending",
            "mock://rancher",
            remote.id.as_str(),
            "Verified account ownership",
            409,
        ),
        (
            "alice",
            "mock://rancher",
            "missing",
            "Verified account ownership",
            404,
        ),
        (
            "alice",
            "mock://rancher",
            remote.id.as_str(),
            "Verified account ownership",
            201,
        ),
        (
            "bob",
            "mock://rancher",
            remote.id.as_str(),
            "Verified account ownership",
            409,
        ),
        (
            "alice",
            "mock://rancher",
            other.id.as_str(),
            "Verified account ownership",
            409,
        ),
    ] {
        let req = actix_web::test::TestRequest::post()
            .uri(&url(id))
            .insert_header(("Authorization", format!("Bearer {}", tokens[0])))
            .set_json(serde_json::json!({"realm":realm,"remote_user_id":remote_id,"reason":reason}))
            .to_request();
        assert_eq!(
            actix_web::test::call_service(&app, req)
                .await
                .status()
                .as_u16(),
            status,
            "{id}/{remote_id}"
        );
    }
    let link = identity_store::get(&state.db, "alice", "mock://rancher")
        .unwrap()
        .unwrap();
    assert_eq!(link.remote_user_id, remote.id);
    assert_eq!(link.remote_username, "external-alice");
    assert_eq!(state.audit_store.count_recent(10).unwrap().len(), 1);
    assert_eq!(users.lock().unwrap()[1].role, UserRole::Viewer);
    assert!(users.lock().unwrap()[1].memberships.is_empty());
    assert!(state
        .services
        .rancher
        .list_prtb("any-project")
        .await
        .unwrap()
        .is_empty());
    // Audit failure rolls back the link, then retry succeeds after repair.
    state.db.lock().unwrap().execute_batch("CREATE TRIGGER reject_identity_audit BEFORE INSERT ON audit_logs BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;").unwrap();
    for status in [500, 201] {
        let req=actix_web::test::TestRequest::post().uri(&url("bob")).insert_header(("Authorization",format!("Bearer {}",tokens[0]))).set_json(serde_json::json!({"realm":"mock://rancher","remote_user_id":other.id,"reason":"Verified account ownership"})).to_request();
        assert_eq!(
            actix_web::test::call_service(&app, req)
                .await
                .status()
                .as_u16(),
            status
        );
        if status == 500 {
            assert!(identity_store::get(&state.db, "bob", "mock://rancher")
                .unwrap()
                .is_none());
            state
                .db
                .lock()
                .unwrap()
                .execute_batch("DROP TRIGGER reject_identity_audit")
                .unwrap();
        }
    }
    sessions.lock().unwrap().remove("admin");
    let req = actix_web::test::TestRequest::get()
        .uri(&url("alice"))
        .insert_header(("Authorization", format!("Bearer {}", tokens[0])))
        .to_request();
    assert_eq!(actix_web::test::call_service(&app, req).await.status(), 401);
}
