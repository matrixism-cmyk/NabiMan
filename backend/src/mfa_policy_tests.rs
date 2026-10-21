use super::*;
use crate::{auth, jwt_sessions as sessions};
use actix_web::test::TestRequest;
use std::sync::{Arc, Mutex};
struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        crate::scope_settings::TEST_PATH.with(|p| *p.borrow_mut() = None);
        let _ = std::fs::remove_file(&self.0);
    }
}
fn fixture() -> Fixture {
    let path = std::env::temp_dir().join(format!("nabiman-policy-{}.json", uuid::Uuid::new_v4()));
    crate::scope_settings::TEST_PATH.with(|p| *p.borrow_mut() = Some(path.clone()));
    Fixture(path)
}
fn user() -> NabimanUser {
    serde_json::from_value(serde_json::json!({
        "id":"member-id","username":"member","role":"viewer","password_hash":"",
        "created_at":"2026-10-06","totp_enabled":true,
        "memberships":[{"kind":"tenant","scope_id":"protected","role":"viewer"}]
    }))
    .unwrap()
}
#[test]
fn enabled_factor_is_not_login_assurance_and_existing_grants_recheck_policy() {
    let file = fixture();
    let user = user();
    let users = Arc::new(Mutex::new(vec![user.clone()]));
    let sessions = sessions::new_session_store();
    let secret = Arc::new("isolated-mfa-policy-test-secret".to_owned());
    assert!(sessions::register_authenticated_session(
        &sessions, &users, "sid", &user, false, "external"
    ));
    let (token, _) = sessions::create_token_pair(&secret, "member", "viewer", "sid").unwrap();
    let req = TestRequest::get()
        .uri("/api/remote-servers")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_srv_request();
    assert!(auth::check_auth(&req, &secret, &sessions, &users) == auth::AuthDecision::Allow);
    let grant = crate::stream_access::ConnectionGrant::from_request(
        req.request(),
        sessions.clone(),
        users.clone(),
    )
    .unwrap();
    assert!(grant.valid());
    std::fs::write(&file.0, r#"{"tenant:protected":{"require_mfa":true}}"#).unwrap();
    assert!(auth::check_auth(&req, &secret, &sessions, &users) == auth::AuthDecision::MfaRequired);
    assert!(!grant.valid());
    let recovery = TestRequest::get()
        .uri("/api/users/me")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_srv_request();
    assert!(auth::check_auth(&recovery, &secret, &sessions, &users) == auth::AuthDecision::Allow);
    assert!(sessions::register_authenticated_session(
        &sessions,
        &users,
        "verified",
        &user,
        true,
        "local-mfa"
    ));
    let (token, _) = sessions::create_token_pair(&secret, "member", "viewer", "verified").unwrap();
    let req = TestRequest::get()
        .uri("/api/remote-servers")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_srv_request();
    assert!(auth::check_auth(&req, &secret, &sessions, &users) == auth::AuthDecision::Allow);
    users.lock().unwrap()[0].totp_enabled = false;
    assert!(auth::check_auth(&req, &secret, &sessions, &users) == auth::AuthDecision::MfaRequired);
}
#[test]
fn strongest_active_scope_wins_and_corrupt_policy_fails_closed() {
    let file = fixture();
    let mut user = user();
    std::fs::write(&file.0, r#"{"tenant:protected":{"require_mfa":true}}"#).unwrap();
    assert!(required(&user).unwrap());
    user.memberships[0].enabled = false;
    assert!(!required(&user).unwrap());
    user.memberships[0].enabled = true;
    user.memberships[0].expires_at = Some("2000-01-01T00:00:00Z".into());
    assert!(!required(&user).unwrap());
    user.memberships[0].expires_at = None;
    std::fs::write(&file.0, "corrupt").unwrap();
    assert!(required(&user).is_err());
    assert!(!permitted(&user, true, "GET", "/api/remote-servers"));
    assert!(permitted(&user, false, "POST", "/api/auth/logout"));
    assert!(!permitted(&user, false, "POST", "/api/auth/2fa/disable"));
    user.role = crate::models::UserRole::Admin;
    assert!(!required(&user).unwrap());
}
