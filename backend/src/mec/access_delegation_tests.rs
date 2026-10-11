use super::*;
use crate::models::{AccountStatus, Membership};
fn fixture() -> (DbHandle, UpdateRequest) {
    let db = super::super::db::open_in_memory().unwrap();
    db.lock().unwrap().execute("INSERT INTO tenant_profiles(id,display_name,namespace,rancher_project_id,allocation_json,quota_json,status,created_at,updated_at) VALUES ('tenant','Tenant','tenant','c:p','{}','{}',?1,'now','now')",params![serde_json::to_string(&TenantStatus::Active).unwrap()]).unwrap();
    (
        db,
        UpdateRequest {
            expected_version: None,
            expected_realm: "mock://rancher".into(),
            expected_project_id: "c:p".into(),
            confirm_tenant_id: "tenant".into(),
            enabled: true,
            manager_ids: vec!["manager".into()],
            allowed_actions: vec!["grant".into()],
            allowed_roles: vec!["read-only".into(), "project-member".into()],
            expires_at: Some((chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339()),
            reason: "Explicitly reviewed tenant delegates".into(),
            acknowledge_project_role_scope: true,
        },
    )
}
fn user(id: &str, role: UserRole) -> NabimanUser {
    NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: id.into(),
        username: id.into(),
        password_hash: "unused".into(),
        role: UserRole::Viewer,
        status: AccountStatus::Active,
        memberships: vec![Membership {
            kind: ScopeKind::Tenant,
            scope_id: "tenant".into(),
            role,
            enabled: true,
            allow_exec: None,
            expires_at: None,
        }],
        requested_memberships: vec![],
        scope_restricted: true,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
    }
}
#[test]
fn delegation_requires_expiry_cas_and_atomic_audit() {
    let (db, mut body) = fixture();
    db.lock().unwrap().execute_batch("CREATE TRIGGER reject_delegation BEFORE INSERT ON audit_logs WHEN NEW.action='tenant_access_delegation_updated' BEGIN SELECT RAISE(ABORT,'audit fault'); END;").unwrap();
    assert!(save(&db, "tenant", &body, "highest", None, None).is_err());
    assert!(read(&db, "tenant").unwrap().is_none());
    db.lock()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_delegation")
        .unwrap();
    let saved = save(&db, "tenant", &body, "highest", None, None).unwrap();
    assert!(saved.permits("manager", "mock://rancher", "c:p", "project-member"));
    assert!(!saved.permits("manager", "mock://rancher", "c:p", "project-owner"));
    assert!(save(&db, "tenant", &body, "highest", None, None).is_err());
    body.expected_version = Some(saved.version.clone());
    body.expires_at = Some((chrono::Utc::now() + chrono::Duration::days(31)).to_rfc3339());
    assert!(save(&db, "tenant", &body, "highest", None, None).is_err());
    body.enabled = false;
    let disabled = save(&db, "tenant", &body, "highest", None, None).unwrap();
    assert_ne!(saved.version, disabled.version);
    assert!(!disabled.permits("manager", "mock://rancher", "c:p", "read-only"));
}
#[test]
fn delegation_cannot_escalate_self_peers_foreign_tenants_or_stale_scope() {
    let (db, body) = fixture();
    let policy = save(&db, "tenant", &body, "highest", None, None).unwrap();
    let actor = user("manager", UserRole::Admin);
    let target = user("member", UserRole::Viewer);
    assert!(authorize(
        &db,
        &actor,
        &target,
        "tenant",
        "mock://rancher",
        "c:p",
        "read-only",
        Some(&policy.version)
    )
    .is_ok());
    for bad in [&actor, &user("peer", UserRole::Admin)] {
        assert!(authorize(
            &db,
            &actor,
            bad,
            "tenant",
            "mock://rancher",
            "c:p",
            "read-only",
            Some(&policy.version)
        )
        .is_err());
    }
    for (tenant, realm, project, role, version) in [
        (
            "foreign",
            "mock://rancher",
            "c:p",
            "read-only",
            policy.version.as_str(),
        ),
        (
            "tenant",
            "mock://other",
            "c:p",
            "read-only",
            policy.version.as_str(),
        ),
        (
            "tenant",
            "mock://rancher",
            "c:foreign",
            "read-only",
            policy.version.as_str(),
        ),
        (
            "tenant",
            "mock://rancher",
            "c:p",
            "project-owner",
            policy.version.as_str(),
        ),
        ("tenant", "mock://rancher", "c:p", "read-only", "stale"),
    ] {
        assert!(authorize(
            &db,
            &actor,
            &target,
            tenant,
            realm,
            project,
            role,
            Some(version)
        )
        .is_err());
    }
    let mut expired = actor.clone();
    expired.memberships[0].expires_at = Some("2000-01-01T00:00:00Z".into());
    assert!(authorize(
        &db,
        &expired,
        &target,
        "tenant",
        "mock://rancher",
        "c:p",
        "read-only",
        Some(&policy.version)
    )
    .is_err());
}
#[test]
fn stale_delegation_cannot_commit_a_managed_intent() {
    let (db, body) = fixture();
    let policy = save(&db, "tenant", &body, "highest", None, None).unwrap();
    db.lock().unwrap().execute("INSERT INTO external_identity_links(local_user_id,realm,remote_user_id,remote_username,verified_at,linked_by,reason) VALUES ('member','mock://rancher','remote','member','now','highest','fixture')",[]).unwrap();
    let binding = crate::mec::services::rancher_service::ProjectRoleBinding {
        id: String::new(),
        project_id: "c:p".into(),
        user_id: "remote".into(),
        role_template: "read-only".into(),
    };
    let begin = |version: &str| {
        super::super::db::membership_store::begin_managed(
            &db,
            "tenant",
            "mock://rancher",
            &binding,
            "manager",
            None,
            None,
            super::super::db::membership_store::ManagedIntent {
                local_user_id: "member",
                source_role: "viewer",
                reason: "Approved scoped access",
                delegation: Some(super::super::db::membership_store::DelegatedIntent {
                    manager_id: "manager",
                    version,
                }),
            },
        )
    };
    assert!(begin("stale").is_err());
    assert!(super::super::db::membership_store::list(&db, "tenant")
        .unwrap()
        .is_empty());
    let operation = begin(&policy.version).unwrap();
    let conn = db.lock().unwrap();
    let version: String = conn
        .query_row(
            "SELECT delegation_version FROM requested_delegated_grants WHERE operation_id=?1",
            [operation.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(version, policy.version);
}
#[test]
fn legacy_action_migration_never_adds_revocation_and_missing_checkpoints_fail_closed() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE tenant_access_delegations (tenant_id TEXT PRIMARY KEY,version TEXT NOT NULL,realm TEXT NOT NULL,project_id TEXT NOT NULL,enabled INTEGER NOT NULL,manager_ids TEXT NOT NULL,allowed_roles TEXT NOT NULL,expires_at TEXT,updated_by TEXT NOT NULL,updated_at TEXT NOT NULL,reason TEXT NOT NULL);").unwrap();
    conn.execute(r#"INSERT INTO tenant_access_delegations VALUES ('tenant','legacy','mock://rancher','c:p',1,'["manager"]','["read-only"]',?1,'highest','now','Approved grant only')"#,params![(chrono::Utc::now()+chrono::Duration::days(1)).to_rfc3339()]).unwrap();
    super::super::db::schema::initialize(&conn).unwrap();
    let migrated = read_on(&conn, "tenant").unwrap().unwrap();
    assert_eq!(migrated.allowed_actions, vec!["grant"]);
    assert!(migrated.permits("manager", "mock://rancher", "c:p", "read-only"));
    assert!(!migrated.permits_action("manager", "mock://rancher", "c:p", "read-only", "revoke"));
    conn.execute("DELETE FROM tenant_access_delegation_actions", [])
        .unwrap();
    super::super::db::schema::initialize(&conn).unwrap();
    assert!(read_on(&conn, "tenant").is_err());
}
#[test]
fn failed_action_migration_rolls_back_schema_changes() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE tenant_access_delegations(tenant_id TEXT PRIMARY KEY, realm TEXT); INSERT INTO tenant_access_delegations VALUES ('tenant','old');").unwrap();
    assert!(super::super::db::schema::initialize(&conn).is_err());
    let created:i64=conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('tenant_access_delegation_actions','membership_operations')",[],|r|r.get(0)).unwrap();
    assert_eq!(created, 0);
    assert_eq!(
        conn.query_row(
            "SELECT realm FROM tenant_access_delegations WHERE tenant_id='tenant'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "old"
    );
}
#[test]
fn managed_revocation_intent_requires_exact_ownership_action_version_and_atomic_audit() {
    use super::super::db::membership_store::{self, ManagedIntent, RevocationIntent};
    let (db, mut body) = fixture();
    body.allowed_actions = vec!["grant".into(), "revoke".into()];
    let policy = save(&db, "tenant", &body, "highest", None, None).unwrap();
    db.lock().unwrap().execute("INSERT INTO external_identity_links(local_user_id,realm,remote_user_id,remote_username,verified_at,linked_by,reason) VALUES ('member','mock://rancher','remote','member','now','highest','Verified fixture')",[]).unwrap();
    let binding = super::super::services::rancher_service::ProjectRoleBinding {
        id: "owned".into(),
        project_id: "c:p".into(),
        user_id: "remote".into(),
        role_template: "read-only".into(),
    };
    let creation = membership_store::begin_managed(
        &db,
        "tenant",
        "mock://rancher",
        &binding,
        "highest",
        None,
        None,
        ManagedIntent {
            delegation: None,
            local_user_id: "member",
            source_role: "viewer",
            reason: "Approved managed source",
        },
    )
    .unwrap();
    membership_store::observe(&db, &creation, "owned").unwrap();
    membership_store::complete(&db, &creation).unwrap();
    let stamp: String = db
        .lock()
        .unwrap()
        .query_row(
            "SELECT updated_at FROM owned_project_bindings WHERE operation_id=?1",
            [&creation.id],
            |r| r.get(0),
        )
        .unwrap();
    let begin = |version: &str, updated: &str| {
        membership_store::begin_managed_revocation(
            &db,
            "tenant",
            "mock://rancher",
            &binding,
            "manager",
            None,
            None,
            RevocationIntent {
                grant_operation_id: &creation.id,
                local_user_id: "member",
                expected_updated_at: updated,
                manager_id: "manager",
                delegation_version: Some(version),
                reason: "Reviewed direct binding only",
            },
        )
    };
    assert!(begin("stale", &stamp).is_err());
    assert!(begin(&policy.version, "stale").is_err());
    db.lock().unwrap().execute_batch("CREATE TRIGGER reject_revoke BEFORE INSERT ON audit_logs WHEN NEW.action='tenant_member_delete_managed_revoke_request' BEGIN SELECT RAISE(ABORT,'audit fault'); END;").unwrap();
    assert!(begin(&policy.version, &stamp).is_err());
    assert_eq!(membership_store::list(&db, "tenant").unwrap().len(), 1);
    assert_eq!(
        db.lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM delegated_membership_revocations",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    db.lock()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_revoke")
        .unwrap();
    let revoke = begin(&policy.version, &stamp).unwrap();
    assert_eq!(revoke.state, "applying");
}
