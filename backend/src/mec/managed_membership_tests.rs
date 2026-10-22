#[path = "managed_membership_recovery_tests.rs"]
mod recovery_tests;

use super::*;
use crate::models::{AccountStatus, Membership};
use std::sync::{Arc, Mutex};
fn user() -> NabimanUser {
    NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: "local".into(),
        username: "alice".into(),
        password_hash: "unused".into(),
        role: UserRole::Viewer,
        status: AccountStatus::Active,
        memberships: vec![Membership {
            kind: ScopeKind::Tenant,
            scope_id: "tenant".into(),
            role: UserRole::Viewer,
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
        remote_permissions: Default::default(),
    }
}
async fn fixture() -> (MecState, UserStore, membership_store::Operation, String) {
    let state = MecState::mocks();
    let users = Arc::new(Mutex::new(vec![user()]));
    let db = &state.db;
    // A minimal tenant profile with all normal required fields.
    db.lock().unwrap().execute("INSERT INTO tenant_profiles(id,display_name,namespace,rancher_project_id,allocation_json,quota_json,status,created_at,created_by,updated_at) VALUES ('tenant','Tenant','tenant','c:p','{\"type\":\"shared\"}','{\"cpu_requests\":\"1\",\"cpu_limits\":\"1\",\"memory_requests\":\"1Gi\",\"memory_limits\":\"1Gi\",\"gpu\":0,\"storage\":\"1Gi\",\"pods\":1,\"pvcs\":1,\"lb_services\":1}','{\"state\":\"active\"}',?1,'admin',?1)",params![chrono::Utc::now().to_rfc3339()]).unwrap();
    let remote = state
        .services
        .rancher
        .create_user("external-alice", "fixture-only-password-123")
        .await
        .unwrap();
    let binding = state
        .services
        .rancher
        .create_prtb("c:p", &remote.id, "read-only")
        .await
        .unwrap();
    let op = membership_store::begin(
        db,
        "tenant",
        "mock://rancher",
        &binding,
        "create",
        "admin",
        None,
        None,
    )
    .unwrap();
    membership_store::observe(db, &op, &binding.id).unwrap();
    membership_store::complete(db, &op).unwrap();
    db.lock().unwrap().execute("INSERT INTO external_identity_links(local_user_id,realm,remote_user_id,remote_username,verified_at,linked_by,reason) VALUES ('local','mock://rancher',?1,'external-alice',?2,'admin','Verified fixture')",params![remote.id,chrono::Utc::now().to_rfc3339()]).unwrap();
    let current = membership_store::get(db, "tenant", &op.id).unwrap();
    (state, users, current, binding.id)
}
#[actix_web::test]
async fn policy_association_is_immutable_exact_and_atomic_with_audit() {
    let (state, _, op, _) = fixture().await;
    assert!(associate(
        &state.db,
        &op,
        "foreign",
        "viewer",
        "admin",
        "Verified policy",
        None,
        None
    )
    .is_err());
    state.db.lock().unwrap().execute_batch("CREATE TRIGGER reject_attach BEFORE INSERT ON audit_logs WHEN NEW.action='tenant_binding_policy_associated' BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;").unwrap();
    assert!(associate(
        &state.db,
        &op,
        "local",
        "viewer",
        "admin",
        "Verified policy",
        None,
        None
    )
    .is_err());
    assert!(list(&state.db, None).unwrap().is_empty());
    state
        .db
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_attach")
        .unwrap();
    associate(
        &state.db,
        &op,
        "local",
        "viewer",
        "admin",
        "Verified policy",
        None,
        None,
    )
    .unwrap();
    assert!(associate(&state.db, &op, "local", "viewer", "admin", "Retry", None, None).is_err());
    assert_eq!(list(&state.db, Some("tenant")).unwrap().len(), 1);
    assert!(list(&state.db, Some("foreign")).unwrap().is_empty());
}
#[actix_web::test]
async fn automatic_revocation_tracks_policy_loss_and_preserves_unmanaged_bindings() {
    for mode in [
        "expired",
        "disabled",
        "suspended",
        "deleted",
        "role_changed",
        "invalid_expiry",
    ] {
        let (state, users, op, binding) = fixture().await;
        associate(
            &state.db,
            &op,
            "local",
            "viewer",
            "admin",
            "Approved automatic revocation",
            None,
            None,
        )
        .unwrap();
        let foreign = state
            .services
            .rancher
            .create_prtb("c:p", "someone-else", "project-owner")
            .await
            .unwrap();
        assert_eq!(sweep(&state, &users).await.unwrap(), 0);
        {
            let mut current = users.lock().unwrap();
            match mode {
                "expired" => {
                    current[0].memberships[0].expires_at = Some("2000-01-01T00:00:00Z".into())
                }
                "disabled" => current[0].memberships[0].enabled = false,
                "suspended" => current[0].status = AccountStatus::Suspended,
                "deleted" => current.clear(),
                "role_changed" => current[0].memberships[0].role = UserRole::Operator,
                _ => current[0].memberships[0].expires_at = Some("invalid".into()),
            }
        }
        assert_eq!(sweep(&state, &users).await.unwrap(), 1, "{mode}");
        let bindings = state.services.rancher.list_prtb("c:p").await.unwrap();
        assert!(!bindings.iter().any(|b| b.id == binding));
        assert!(bindings.iter().any(|b| b.id == foreign.id));
        assert_eq!(list(&state.db, None).unwrap()[0].ownership_state, "revoked");
        assert_eq!(sweep(&state, &users).await.unwrap(), 0);
        *users.lock().unwrap() = vec![user()]; // Restoration does not silently regrant.
        assert_eq!(sweep(&state, &users).await.unwrap(), 0);
        assert!(!state
            .services
            .rancher
            .list_prtb("c:p")
            .await
            .unwrap()
            .iter()
            .any(|b| b.id == binding));
    }
}
#[actix_web::test]
async fn policy_loss_does_not_touch_unassociated_owned_grants_or_changed_provider_identity() {
    let (state, users, op, binding) = fixture().await;
    users.lock().unwrap().clear();
    assert_eq!(sweep(&state, &users).await.unwrap(), 0);
    assert!(state
        .services
        .rancher
        .list_prtb("c:p")
        .await
        .unwrap()
        .iter()
        .any(|b| b.id == binding));
    associate(
        &state.db,
        &op,
        "local",
        "viewer",
        "admin",
        "Approved policy",
        None,
        None,
    )
    .unwrap();
    state.db.lock().unwrap().execute("UPDATE owned_project_bindings SET remote_user_id='different-user' WHERE operation_id=?1",params![op.id]).unwrap();
    assert_eq!(sweep(&state, &users).await.unwrap(), 0);
    assert!(state
        .services
        .rancher
        .list_prtb("c:p")
        .await
        .unwrap()
        .iter()
        .any(|b| b.id == binding));
}
#[test]
fn role_ceiling_does_not_infer_privileges_from_matching_usernames() {
    let mut u = user();
    assert!(source_role(&u, "foreign", "read-only").is_none());
    assert!(source_role(&u, "tenant", "project-owner").is_none());
    u.memberships[0].role = UserRole::Operator;
    assert!(source_role(&u, "tenant", "project-owner").is_none());
    u.memberships[0].role = UserRole::Admin;
    assert!(source_role(&u, "tenant", "project-owner").is_some());
    assert!(source_role(&u, "tenant", "custom-unknown-role").is_none());
}

#[actix_web::test]
async fn managed_request_and_approval_audit_are_atomic_before_dispatch() {
    let (state, _, previous, _) = fixture().await;
    state
        .db
        .lock()
        .unwrap()
        .execute(
            "UPDATE owned_project_bindings SET state='revoked' WHERE operation_id=?1",
            params![previous.id],
        )
        .unwrap();
    let binding = ProjectRoleBinding {
        id: String::new(),
        project_id: previous.project_id,
        user_id: previous.remote_user_id,
        role_template: previous.role_template,
    };
    let policy = || membership_store::ManagedIntent {
        delegation: None,
        local_user_id: "local",
        source_role: "viewer",
        reason: "Approve automatic revocation",
    };
    state.db.lock().unwrap().execute_batch("CREATE TRIGGER reject_request BEFORE INSERT ON audit_logs WHEN NEW.action LIKE '%_managed_request' BEGIN SELECT RAISE(ABORT,'fixture managed intent audit failure'); END;").unwrap();
    assert!(membership_store::begin_managed(
        &state.db,
        "tenant",
        "mock://rancher",
        &binding,
        "admin",
        None,
        None,
        policy()
    )
    .is_err());
    let count: i64 = state
        .db
        .lock()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM requested_managed_grants", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        membership_store::list(&state.db, "tenant").unwrap().len(),
        1
    );
    state
        .db
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_request")
        .unwrap();
    let op = membership_store::begin_managed(
        &state.db,
        "tenant",
        "mock://rancher",
        &binding,
        "admin",
        None,
        None,
        policy(),
    )
    .unwrap();
    assert!(membership_store::begin_managed(
        &state.db,
        "tenant",
        "mock://rancher",
        &binding,
        "admin",
        None,
        None,
        policy()
    )
    .is_err());
    membership_store::observe(&state.db, &op, "managed-new-binding").unwrap();
    membership_store::complete(&state.db, &op).unwrap();
    assert_eq!(list(&state.db, None).unwrap()[0].local_user_id, "local");
}
