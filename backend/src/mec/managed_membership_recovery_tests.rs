use super::*;

pub(super) async fn pending_fixture(
    candidate: bool,
) -> (MecState, UserStore, membership_store::Operation, String) {
    let (state, users, previous, old_binding) = fixture().await;
    state
        .services
        .rancher
        .delete_prtb(&old_binding)
        .await
        .unwrap();
    state
        .db
        .lock()
        .unwrap()
        .execute(
            "UPDATE owned_project_bindings SET state='revoked' WHERE operation_id=?1",
            params![previous.id],
        )
        .unwrap();
    let expected = ProjectRoleBinding {
        id: String::new(),
        project_id: previous.project_id,
        user_id: previous.remote_user_id,
        role_template: previous.role_template,
    };
    let op = membership_store::begin_managed(
        &state.db,
        "tenant",
        "mock://rancher",
        &expected,
        "admin",
        None,
        None,
        membership_store::ManagedIntent {
            delegation: None,
            local_user_id: "local",
            source_role: "viewer",
            reason: "Approved automatic revocation",
        },
    )
    .unwrap();
    let binding = state
        .services
        .rancher
        .create_prtb(
            &expected.project_id,
            &expected.user_id,
            &expected.role_template,
        )
        .await
        .unwrap();
    if candidate {
        membership_store::observe(&state.db, &op, &binding.id).unwrap();
    }
    membership_store::unknown(&state.db, &op, "Creation readback not confirmed").unwrap();
    (state, users, op, binding.id)
}
#[actix_web::test]
pub(super) async fn denied_managed_candidate_is_confirmed_then_revoked_without_creation_replay() {
    let (state, users, op, binding) = pending_fixture(true).await;
    let foreign = state
        .services
        .rancher
        .create_prtb("c:p", "foreign-user", "project-owner")
        .await
        .unwrap();
    assert_eq!(sweep(&state, &users).await.unwrap(), 0);
    assert!(list(&state.db, None).unwrap().is_empty());
    users.lock().unwrap()[0].memberships[0].enabled = false;
    assert_eq!(sweep(&state, &users).await.unwrap(), 1);
    assert_eq!(
        membership_store::get(&state.db, "tenant", &op.id)
            .unwrap()
            .state,
        "completed"
    );
    assert_eq!(list(&state.db, None).unwrap()[0].ownership_state, "revoked");
    let bindings = state.services.rancher.list_prtb("c:p").await.unwrap();
    assert!(!bindings.iter().any(|b| b.id == binding));
    assert!(bindings.iter().any(|b| b.id == foreign.id));
    assert!(pending(&state.db, None).unwrap().is_empty());
    let logs = AuditStore::new(state.db.clone()).count_recent(20).unwrap();
    assert!(logs
        .iter()
        .any(|log| log.user == "system:managed-candidate-confirmation"
            && log.action.ends_with("_reconciled")));
}
#[actix_web::test]
pub(super) async fn unknown_candidate_is_not_adopted_by_name_role_or_another_realm() {
    for mode in [
        "missing_id",
        "absent_candidate",
        "realm_changed",
        "role_mismatch",
    ] {
        let (state, users, op, binding) = pending_fixture(mode != "missing_id").await;
        users.lock().unwrap()[0].memberships[0].enabled = false;
        match mode {
            "absent_candidate" => state.services.rancher.delete_prtb(&binding).await.unwrap(),
            "realm_changed" => {
                state.db.lock().unwrap().execute("UPDATE membership_operations SET realm='https://other.example' WHERE id=?1",params![op.id]).unwrap();
            }
            "role_mismatch" => {
                state.db.lock().unwrap().execute("UPDATE membership_operations SET role_template='project-owner' WHERE id=?1",params![op.id]).unwrap();
            }
            _ => {}
        }
        assert_eq!(sweep(&state, &users).await.unwrap(), 0, "{mode}");
        assert!(list(&state.db, None).unwrap().is_empty());
        assert_eq!(
            membership_store::get(&state.db, "tenant", &op.id)
                .unwrap()
                .state,
            "unknown_result"
        );
        if mode != "absent_candidate" {
            assert!(state
                .services
                .rancher
                .list_prtb("c:p")
                .await
                .unwrap()
                .iter()
                .any(|b| b.id == binding));
        }
    }
}
#[actix_web::test]
pub(super) async fn automatic_revocation_absence_can_be_confirmed_after_source_restoration() {
    let (state, users, previous, binding) = fixture().await;
    associate(
        &state.db,
        &previous,
        "local",
        "viewer",
        "admin",
        "Approve automatic revocation",
        None,
        None,
    )
    .unwrap();
    let target = ProjectRoleBinding {
        id: binding.clone(),
        project_id: previous.project_id,
        user_id: previous.remote_user_id,
        role_template: previous.role_template,
    };
    let delete = membership_store::begin(
        &state.db,
        "tenant",
        "mock://rancher",
        &target,
        "delete",
        "system:membership-revocation",
        None,
        None,
    )
    .unwrap();
    state.services.rancher.delete_prtb(&binding).await.unwrap();
    membership_store::unknown(&state.db, &delete, "Provider response was lost").unwrap();
    assert_eq!(sweep(&state, &users).await.unwrap(), 1);
    assert_eq!(
        membership_store::get(&state.db, "tenant", &delete.id)
            .unwrap()
            .state,
        "completed"
    );
    assert_eq!(list(&state.db, None).unwrap()[0].ownership_state, "revoked");
    assert!(state
        .services
        .rancher
        .list_prtb("c:p")
        .await
        .unwrap()
        .is_empty());
}
