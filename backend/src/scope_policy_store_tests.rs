use super::*;
struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        super::super::TEST_PATH.with(|p| *p.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> Fixture {
    let dir = std::env::temp_dir().join(format!("nabiman-policy-store-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    super::super::TEST_PATH.with(|p| *p.borrow_mut() = Some(dir.join("scope-settings.json")));
    Fixture(dir)
}
fn actor() -> NabimanUser {
    serde_json::from_value(serde_json::json!({"id":"immutable-admin","username":"admin","role":"admin","password_hash":"","created_at":"2026-10-06"})).unwrap()
}
#[test]
fn legacy_read_is_non_mutating_and_atomic_save_retains_other_scopes() {
    let fixture = fixture();
    let path = fixture.0.join("scope-settings.json");
    let old = r#"{"tenant:one":{"require_mfa":true},"server:two":{"allow_commands":false}}"#;
    std::fs::write(&path, old).unwrap();
    let mut all = load().unwrap();
    let legacy = all["tenant:one"].revision.clone();
    assert!(all["tenant:one"].history.is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), old);
    assert_eq!(load().unwrap()["tenant:one"].revision, legacy);
    let mut next = all["tenant:one"].policy.clone();
    next.member_notice = "Changed notice".into();
    all.get_mut("tenant:one")
        .unwrap()
        .replace(next, &actor(), "Reviewed change".into());
    save(&all).unwrap();
    let disk = load().unwrap();
    assert_ne!(disk["tenant:one"].revision, legacy);
    assert!(disk["tenant:one"].policy.require_mfa);
    assert!(!disk["server:two"].policy.allow_commands);
    assert_eq!(disk["tenant:one"].history[0].actor_id, "immutable-admin");
    assert_eq!(disk["tenant:one"].history[0].fields.len(), 1);
    assert_eq!(disk["tenant:one"].history[0].fields[0].before, "");
    assert_eq!(
        disk["tenant:one"].history[0].fields[0].after,
        "Changed notice"
    );
}
#[test]
fn history_is_bounded_versions_change_even_without_value_changes_and_commit_failure_is_reported() {
    let fixture = fixture();
    let path = fixture.0.join("scope-settings.json");
    let mut record = Record::legacy("tenant:one", ScopeSettings::default());
    let first = record.revision.clone();
    for _ in 0..105 {
        record.replace(ScopeSettings::default(), &actor(), "Explicit resave".into());
    }
    assert_eq!(record.history.len(), 100);
    assert!(record.history.iter().all(|c| c.fields.is_empty()));
    assert_ne!(record.revision, first);
    for pair in record.history.windows(2) {
        assert_eq!(pair[0].revision, pair[1].previous_revision);
    }
    let records = BTreeMap::from([("tenant:one".into(), record)]);
    std::fs::create_dir(&path).unwrap();
    assert!(save(&records).is_err());
    assert!(path.is_dir());
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[actix_web::test]
async fn policy_commit_rechecks_session_after_middleware_authentication() {
    use crate::{auth, jwt_sessions};
    use actix_web::{test::TestRequest, web};
    use std::sync::{Arc, Mutex};
    let fixture = fixture();
    let administrator = actor();
    let users = Arc::new(Mutex::new(vec![administrator.clone()]));
    let sessions = jwt_sessions::new_session_store();
    let secret = Arc::new("policy-commit-fixture-secret".to_string());
    let state = web::Data::new(crate::mec::MecState::mocks());
    let tenant=serde_json::from_value(serde_json::json!({
        "id":"one","display_name":"One","namespace":"one","allocation":{"type":"shared"},
        "quota":{"cpu_requests":"1","cpu_limits":"2","memory_requests":"1Gi","memory_limits":"2Gi","gpu":0,"storage":"10Gi","pods":10,"pvcs":2,"lb_services":1},
        "created_at":"2026-10-06T00:00:00Z","updated_at":"2026-10-06T00:00:00Z","status":{"state":"active"}
    })).unwrap();
    state.tenants.upsert(&tenant, "admin").unwrap();
    jwt_sessions::register_session(&sessions, "sid", "admin", &administrator.id, "fixture");
    let (token, _) = jwt_sessions::create_token_pair(&secret, "admin", "admin", "sid").unwrap();
    let req = TestRequest::put()
        .uri("/api/scope-settings/tenant/one")
        .app_data(web::Data::new(sessions.clone()))
        .app_data(state)
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_srv_request();
    assert!(auth::check_auth(&req, &secret, &sessions, &users) == auth::AuthDecision::Allow);
    sessions.lock().unwrap().remove("sid");
    let (req, _) = req.into_parts();
    let snapshot = Record::legacy("tenant:one", ScopeSettings::default());
    let response=super::super::put(req,web::Path::from(("tenant".into(),"one".into())),
        web::Json(serde_json::json!({"expected_revision":snapshot.revision,"change_reason":"Attempt after logout","member_notice":"must not persist"})),web::Data::new(users)).await;
    assert_eq!(response.status(), 401);
    assert!(!fixture.0.join("scope-settings.json").exists());
}
