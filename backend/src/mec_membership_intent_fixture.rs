use super::*;

pub(super) struct Fixture {
    pub secret: auth::JwtSecret,
    pub sessions: jwt_sessions::SessionStore,
    pub users: users::UserStore,
    pub db: mec::db::DbHandle,
    pub state: web::Data<mec::MecState>,
    pub token: String,
    pub delegation_version: Option<String>,
    pub provider: provider::Provider,
}
pub(super) fn setup(mode: &'static str) -> Fixture {
    let secret: auth::JwtSecret = Arc::new("membership-wire-fixture".into());
    let sessions = jwt_sessions::new_session_store();
    jwt_sessions::register_session(&sessions, "sid", "admin", "admin", "127.0.0.1");
    let users = Arc::new(Mutex::new(vec![NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: "admin".into(),
        username: "admin".into(),
        password_hash: "unused".into(),
        role: UserRole::Admin,
        status: AccountStatus::Active,
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
    let db = mec::db::open_in_memory().unwrap();
    if mode.starts_with("managed_") {
        let mut source = users.lock().unwrap()[0].clone();
        source.id = "local".into();
        source.username = "local-alice".into();
        source.role = UserRole::Viewer;
        source.memberships = vec![crate::models::Membership {
            kind: crate::models::ScopeKind::Tenant,
            scope_id: "tenant".into(),
            role: UserRole::Viewer,
            enabled: true,
            allow_exec: None,
            expires_at: None,
        }];
        users.lock().unwrap().push(source);
    }
    if mode.starts_with("managed_delegated") {
        let mut current = users.lock().unwrap();
        current[0].role = UserRole::Viewer;
        current[0].scope_restricted = true;
        current[0].memberships = vec![crate::models::Membership {
            kind: crate::models::ScopeKind::Tenant,
            scope_id: "tenant".into(),
            role: UserRole::Admin,
            enabled: true,
            allow_exec: None,
            expires_at: None,
        }];
        if mode == "managed_delegated_foreign_target" {
            current[1].memberships[0].scope_id = "foreign".into();
        }
        if mode == "managed_delegated_peer" {
            current[1].memberships[0].role = UserRole::Admin;
        }
    }
    let provider = provider::start(mode, &db, &users, &sessions);
    let address = provider.address;
    let mut bundle = ServiceBundle::mocks();
    bundle.rancher = Arc::new(
        RancherReal::new(RancherRealConfig {
            base_url: format!("http://{address}"),
            token: "fixture-only-provider-key".into(),
            cluster_id: "c".into(),
            insecure_tls: false,
        })
        .unwrap(),
    );
    let state = web::Data::new(mec::MecState::new(db.clone(), bundle));
    let now = chrono::Utc::now();
    state
        .tenants
        .upsert(
            &Tenant {
                id: "tenant".into(),
                display_name: "Tenant".into(),
                task_name: None,
                contact_email: None,
                namespace: "tenant".into(),
                rancher_project_id: Some("c:p".into()),
                rancher_user_id: None,
                allocation: NodeAllocation {
                    alloc_type: AllocationType::Shared,
                    node: None,
                    gpu_label: None,
                    tier: None,
                },
                quota: ResourceQuota::standard(),
                starter_kit: None,
                created_at: now,
                updated_at: now,
                status: TenantStatus::Active,
            },
            "admin",
        )
        .unwrap();
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &auth::Claims {
            sub: "admin".into(),
            role: if mode.starts_with("managed_delegated") {
                "viewer"
            } else {
                "admin"
            }
            .into(),
            sid: "sid".into(),
            iat: now.timestamp() as usize,
            exp: (now.timestamp() + 300) as usize,
        },
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();
    if mode.starts_with("managed_") && mode != "managed_missing_identity" {
        db.lock().unwrap().execute("INSERT INTO external_identity_links(local_user_id,realm,remote_user_id,remote_username,verified_at,linked_by,reason) VALUES ('local',?1,'u-one','alice',?2,'admin','Verified fixture')",rusqlite::params![format!("http://{address}"),chrono::Utc::now().to_rfc3339()]).unwrap();
    }
    let delegation_version = if mode.starts_with("managed_delegated")
        && mode != "managed_delegated_no_policy"
    {
        let policy = mec::access_delegation::save(
            &db,
            "tenant",
            &mec::access_delegation::UpdateRequest {
                expected_version: None,
                expected_realm: format!("http://{address}"),
                expected_project_id: "c:p".into(),
                confirm_tenant_id: "tenant".into(),
                enabled: true,
                manager_ids: vec![if mode == "managed_delegated_foreign_manager" {
                    "other"
                } else {
                    "admin"
                }
                .into()],
                allowed_actions: vec!["grant".into()],
                allowed_roles: vec!["read-only".into()],
                expires_at: Some((chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339()),
                reason: "Highest approved tenant delegation".into(),
                acknowledge_project_role_scope: true,
            },
            "highest-fixture",
            None,
            None,
        )
        .unwrap();
        if mode == "managed_delegated_expired" {
            db.lock().unwrap().execute("UPDATE tenant_access_delegations SET expires_at='2000-01-01T00:00:00Z' WHERE tenant_id='tenant'",[]).unwrap();
        }
        if mode == "managed_delegated_intent_audit_failure" {
            db.lock().unwrap().execute_batch("CREATE TRIGGER reject_delegated_intent BEFORE INSERT ON audit_logs WHEN NEW.action LIKE '%_managed_request' BEGIN SELECT RAISE(ABORT,'intent audit fault'); END;").unwrap();
        }
        Some(policy.version)
    } else {
        None
    };
    Fixture {
        secret,
        sessions,
        users,
        db,
        state,
        token,
        delegation_version,
        provider,
    }
}
