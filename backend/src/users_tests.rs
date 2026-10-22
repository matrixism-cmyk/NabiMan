use super::*;

fn admin_store() -> UserStore {
    Arc::new(Mutex::new(vec![NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: "1".into(),
        username: "admin".into(),
        password_hash: "hash".into(),
        role: UserRole::Admin,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
        remote_permissions: Default::default(),
        access: Default::default(),
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: false,
    }]))
}

#[test]
fn disk_format_retains_secrets_while_api_format_omits_them() {
    let user = NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: "1".into(),
        username: "admin".into(),
        password_hash: "hash".into(),
        role: UserRole::Admin,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: true,
        totp_secret: Some("secret".into()),
        mfa: Default::default(),
        remote_permissions: Default::default(),
        access: Default::default(),
        status: Default::default(),
        memberships: vec![],
        requested_memberships: vec![],
        scope_restricted: false,
    };
    let disk = serialize_users(&[user.clone()]).unwrap();
    let restored: Vec<NabimanUser> = serde_json::from_str(&disk).unwrap();
    assert_eq!(restored[0].password_hash, "hash");
    assert_eq!(restored[0].totp_secret.as_deref(), Some("secret"));
    let api = serde_json::to_string(&user).unwrap();
    assert!(!api.contains("hash"));
    assert!(!api.contains("secret"));
}

#[actix_web::test]
async fn last_admin_cannot_be_demoted() {
    let store = admin_store();
    let sessions = crate::jwt_sessions::new_session_store();
    crate::jwt_sessions::register_session(&sessions, "test", "admin", "1", "127.0.0.1");
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .wrap(crate::AuthCheck::new(
                Arc::new("test".into()),
                sessions.clone(),
                store.clone(),
            ))
            .app_data(web::Data::new(store.clone()))
            .app_data(web::Data::new(sessions))
            .route("/api/users/{id}", web::put().to(update_user)),
    )
    .await;
    let claims = crate::auth::Claims {
        sub: "admin".into(),
        role: "admin".into(),
        sid: "test".into(),
        iat: chrono::Utc::now().timestamp() as usize,
        exp: (chrono::Utc::now().timestamp() + 300) as usize,
    };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(b"test"),
    )
    .unwrap();
    let request = actix_web::test::TestRequest::put()
        .uri("/api/users/1")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(serde_json::json!({"role":"viewer"}))
        .to_request();
    let response = actix_web::test::call_service(&app, request).await;
    assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
    assert_eq!(store.lock().unwrap()[0].role, UserRole::Admin);
}

#[test]
fn external_identity_cannot_take_over_local_user() {
    let store = admin_store();
    assert!(ensure_oauth_user(&store, "admin", "admin").is_err());
    assert_eq!(store.lock().unwrap()[0].password_hash, "hash");
}

#[test]
fn published_admin_password_requires_rotation() {
    let mut users = admin_store().lock().unwrap().clone();
    users[0].password_hash = bcrypt::hash("nabiman", 4).unwrap();
    assert!(rotate_published_admin_password(&mut users, None).is_err());
    assert!(
        rotate_published_admin_password(&mut users, Some("unique-password-for-admin")).unwrap()
    );
    assert!(bcrypt::verify("unique-password-for-admin", &users[0].password_hash).unwrap());
    assert!(!bcrypt::verify("nabiman", &users[0].password_hash).unwrap());
}

#[test]
fn share_versions_rotate_on_security_changes_and_survive_disk_roundtrip() {
    let original = admin_store().lock().unwrap().clone();
    let version = original[0].share_authorization_version.clone();
    let mut users = original.clone();
    users[0].last_login = Some("later".into());
    refresh_share_versions(&mut users, &original);
    assert_eq!(users[0].share_authorization_version, version);
    users[0].role = UserRole::Viewer;
    refresh_share_versions(&mut users, &original);
    let demoted = users.clone();
    assert_ne!(demoted[0].share_authorization_version, version);
    users[0].role = UserRole::Admin;
    refresh_share_versions(&mut users, &demoted);
    assert_ne!(users[0].share_authorization_version, version);
    assert_ne!(
        users[0].share_authorization_version,
        demoted[0].share_authorization_version
    );
    let restored: Vec<NabimanUser> =
        serde_json::from_str(&serialize_users(&users).unwrap()).unwrap();
    assert_eq!(
        restored[0].share_authorization_version,
        users[0].share_authorization_version
    );
    assert!(serde_json::to_value(&users[0])
        .unwrap()
        .get("share_authorization_version")
        .is_none());
    for change in 0..4 {
        let mut changed = original.clone();
        match change {
            0 => changed[0].status = crate::models::AccountStatus::Suspended,
            1 => changed[0].password_hash = "new-hash".into(),
            2 => changed[0].totp_enabled = true,
            _ => changed[0].totp_secret = Some("new-secret".into()),
        }
        refresh_share_versions(&mut changed, &original);
        assert_ne!(changed[0].share_authorization_version, version);
    }
}

#[test]
fn account_version_commit_is_durable_and_failure_restores_old_authority() {
    let dir = std::env::temp_dir().join(format!("nabiman-version-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("users.json");
    let previous = admin_store().lock().unwrap().clone();
    let old_version = previous[0].share_authorization_version.clone();
    save_users_at(&previous, &path).unwrap();
    let mut users = previous.clone();
    users[0].role = UserRole::Viewer;
    let blocked = dir.join("not-a-directory");
    std::fs::write(&blocked, "block").unwrap();
    assert!(
        commit_users_with(&mut users, previous.clone(), |u| save_users_at(
            u,
            &blocked.join("users.json")
        )
        .map_err(reject_ambiguous_publication))
        .is_err()
    );
    assert_eq!(users[0].role, UserRole::Admin);
    assert_eq!(users[0].share_authorization_version, old_version);
    let disk: Vec<NabimanUser> = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(disk[0].share_authorization_version, old_version);
    users[0].role = UserRole::Viewer;
    commit_users_with(&mut users, previous, |u| {
        save_users_at(u, &path).map_err(reject_ambiguous_publication)
    })
    .unwrap();
    let disk: Vec<NabimanUser> = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(disk[0].role, UserRole::Viewer);
    assert_eq!(
        disk[0].share_authorization_version,
        users[0].share_authorization_version
    );
    assert_ne!(disk[0].share_authorization_version, old_version);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn post_rename_sync_failure_preserves_candidate_and_marks_uncertainty() {
    let dir = std::env::temp_dir().join(format!("nabiman-user-sync-{}", uuid::Uuid::new_v4()));
    let path = dir.join("users.json");
    let previous = admin_store().lock().unwrap().clone();
    save_users_at(&previous, &path).unwrap();
    let mut changed = previous.clone();
    changed[0].role = UserRole::Viewer;
    refresh_share_versions(&mut changed, &previous);
    let error = save_users_with_sync(&changed, &path, |_| {
        Err(std::io::Error::other("injected directory sync failure"))
    })
    .unwrap_err();
    assert!(error.published);
    let observed: Vec<NabimanUser> = serde_json::from_str(&read_users_at(&path).unwrap()).unwrap();
    assert_eq!(observed[0].role, UserRole::Viewer);
    assert_eq!(
        observed[0].share_authorization_version,
        changed[0].share_authorization_version
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1); // no rollback or leftover temp
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn uncertain_user_publication_terminates_process_without_success_receipt() {
    const CHILD: &str = "NABIMAN_TEST_USER_PUBLICATION_CHILD";
    if let Some(dir) = std::env::var_os(CHILD) {
        let path = std::path::PathBuf::from(dir).join("users.json");
        let previous = admin_store().lock().unwrap().clone();
        save_users_at(&previous, &path).unwrap();
        let mut changed = previous.clone();
        changed[0].role = UserRole::Viewer;
        refresh_share_versions(&mut changed, &previous);
        let error = save_users_with_sync(&changed, &path, |_| {
            Err(std::io::Error::other("injected directory sync failure"))
        })
        .unwrap_err();
        reject_ambiguous_publication(error);
        panic!("Ambiguous publication returned to the caller");
    }
    let dir = std::env::temp_dir().join(format!("nabiman-user-fatal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "users::tests::uncertain_user_publication_terminates_process_without_success_receipt",
            "--nocapture",
        ])
        .env(CHILD, &dir)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(78));
    assert!(String::from_utf8_lossy(&result.stderr).contains("publication is uncertain"));
    let observed: Vec<NabimanUser> =
        serde_json::from_str(&read_users_at(&dir.join("users.json")).unwrap()).unwrap();
    assert_eq!(observed[0].role, UserRole::Viewer);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn startup_user_read_rejects_non_regular_database() {
    let dir = std::env::temp_dir().join(format!("nabiman-user-read-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(read_users_at(&dir).is_err());
    let fifo = dir.join("users.fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(read_users_at(&fifo).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
