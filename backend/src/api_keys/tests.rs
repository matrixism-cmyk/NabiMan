use super::*;
fn user() -> NabimanUser {
    serde_json::from_value(serde_json::json!({"id":"immutable-owner","username":"admin","password_hash":"hash","role":"admin","created_at":"now","status":"active"})).unwrap()
}
fn fixture() -> (
    std::path::PathBuf,
    ApiKeyStore,
    crate::users::UserStore,
    String,
) {
    let dir = std::env::temp_dir().join(format!("nb-token-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let raw = format!("nbm_{}", "a".repeat(64));
    let owner = user();
    let mut store = Store::open(dir.join("api_keys.json")).unwrap();
    store
        .commit(vec![ApiKey {
            id: "token-one".into(),
            name: "test".into(),
            key_hash: hash(&raw),
            prefix: raw[..12].into(),
            role: "scoped".into(),
            created_at: "now".into(),
            last_used: None,
            expires_at: Some((chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339()),
            version: 1,
            owner_id: owner.id.clone(),
            security_stamp: stamp(&owner),
            scopes: vec!["status:read".into()],
        }])
        .unwrap();
    (
        dir,
        Arc::new(Mutex::new(store)),
        Arc::new(Mutex::new(vec![owner])),
        raw,
    )
}
#[test]
fn expiry_malformed_and_legacy_fail_closed() {
    let (dir, store, users, raw) = fixture();
    for expiry in ["garbage", "2001-01-01T00:00:00Z"] {
        store.lock().unwrap().keys[0].expires_at = Some(expiry.into());
        assert!(matches!(
            authenticate(&raw, &store, &users, "GET", "/api/server/status"),
            Err(Rejection::Invalid)
        ));
    }
    store.lock().unwrap().keys[0].expires_at =
        Some((chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339());
    store.lock().unwrap().keys[0].version = 0;
    assert!(matches!(
        authenticate(&raw, &store, &users, "GET", "/api/server/status"),
        Err(Rejection::Invalid)
    ));
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn authorization_is_exact_and_default_deny() {
    let scopes = SCOPES.iter().map(|v| v.to_string()).collect::<Vec<_>>();
    for path in [
        "/api/server/status",
        "/api/remote-servers",
        "/api/terminal/sessions",
        "/api/mec/v1/tenants",
        "/api/mec/v1/tenants/id-1",
        "/api/mec/v1/tenants/id_2/resources",
    ] {
        assert!(allows(&scopes, "GET", path));
        assert!(!allows(&scopes, "POST", path));
    }
    for path in [
        "/api/apikeys",
        "/api/users",
        "/api/terminal",
        "/api/files",
        "/api/mec/v1/tenants//resources",
        "/api/mec/v1/tenants/x/members",
        "/api/mec/v1/tenants/x/../secrets",
        "/api/mec/v1/tenants/x%2fy/resources",
        "/api/server/status/extra",
        "/api/terminal/sessions/abc",
    ] {
        assert!(!allows(&scopes, "GET", path));
    }
    // A scope grants only its own path: status:read cannot read sessions.
    assert!(!allows(&["status:read".to_string()], "GET", "/api/terminal/sessions"));
    assert!(allows(&["sessions:read".to_string()], "GET", "/api/terminal/sessions"));
}
#[test]
fn usage_restart_revocation_and_owner_changes() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, store, users, raw) = fixture();
    assert!(authenticate(&raw, &store, &users, "GET", "/api/server/status").is_ok());
    assert!(matches!(
        authenticate(&raw, &store, &users, "GET", "/api/remote-servers"),
        Err(Rejection::Forbidden)
    ));
    let disk = Store::open(dir.join("api_keys.json")).unwrap();
    assert!(disk.keys[0].last_used.is_some());
    assert!(!std::fs::read_to_string(dir.join("api_keys.json"))
        .unwrap()
        .contains(&raw));
    assert_eq!(
        std::fs::metadata(dir.join("api_keys.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let original = users.lock().unwrap()[0].clone();
    for kind in 0..5 {
        let mut changed = original.clone();
        match kind {
            0 => changed.id = "replacement".into(),
            1 => changed.password_hash = "new".into(),
            2 => changed.totp_enabled = true,
            3 => changed.role = crate::models::UserRole::Viewer,
            _ => changed.scope_restricted = true,
        }
        users.lock().unwrap()[0] = changed;
        assert!(matches!(
            authenticate(&raw, &store, &users, "GET", "/api/server/status"),
            Err(Rejection::Invalid)
        ));
    }
    users.lock().unwrap()[0] = original;
    store.lock().unwrap().commit(vec![]).unwrap();
    assert!(matches!(
        authenticate(&raw, &store, &users, "GET", "/api/server/status"),
        Err(Rejection::Invalid)
    ));
    assert!(Store::open(dir.join("api_keys.json"))
        .unwrap()
        .keys
        .is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn storage_failure_freezes_authentication_until_restart() {
    let (dir, store, users, raw) = fixture();
    std::fs::remove_file(dir.join("api_keys.json")).unwrap();
    std::fs::create_dir(dir.join("api_keys.json")).unwrap();
    assert!(matches!(
        authenticate(&raw, &store, &users, "GET", "/api/server/status"),
        Err(Rejection::Storage)
    ));
    std::fs::remove_dir(dir.join("api_keys.json")).unwrap();
    assert!(matches!(
        authenticate(&raw, &store, &users, "GET", "/api/server/status"),
        Err(Rejection::Storage)
    ));
    assert!(store.lock().unwrap().failed);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn duplicate_ids_and_symlink_storage_rejected() {
    let (dir, store, _, _) = fixture();
    let key = store.lock().unwrap().keys[0].clone();
    std::fs::write(
        dir.join("duplicate"),
        serde_json::to_vec(&vec![key.clone(), key]).unwrap(),
    )
    .unwrap();
    assert!(Store::open(dir.join("duplicate")).is_err());
    std::os::unix::fs::symlink(dir.join("api_keys.json"), dir.join("link")).unwrap();
    assert!(Store::open(dir.join("link")).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
