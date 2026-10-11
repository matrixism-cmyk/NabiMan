use super::*;
fn binding() -> ProjectRoleBinding {
    ProjectRoleBinding {
        id: "binding-one".into(),
        project_id: "c:p".into(),
        user_id: "u-one".into(),
        role_template: "read-only".into(),
    }
}
fn start(db: &DbHandle, kind: &str) -> ServiceResult<Operation> {
    begin(
        db,
        "tenant",
        "https://rancher.example",
        &binding(),
        kind,
        "admin",
        Some("127.0.0.1".into()),
        Some("fixture".into()),
    )
}
fn owned_count(db: &DbHandle) -> i64 {
    db.lock()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM owned_project_bindings", [], |r| {
            r.get(0)
        })
        .unwrap()
}
#[test]
fn intent_and_audit_are_atomic_before_any_provider_call() {
    let db = crate::mec::db::open_in_memory().unwrap();
    db.lock().unwrap().execute_batch("CREATE TRIGGER reject_audit BEFORE INSERT ON audit_logs BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;").unwrap();
    assert!(start(&db, "create").is_err());
    assert!(list(&db, "tenant").unwrap().is_empty());
    assert_eq!(owned_count(&db), 0);
}
#[test]
fn confirmed_ownership_completion_and_audit_commit_together() {
    let db = crate::mec::db::open_in_memory().unwrap();
    let op = start(&db, "create").unwrap();
    observe(&db, &op, "binding-one").unwrap();
    db.lock().unwrap().execute_batch("CREATE TRIGGER reject_confirm BEFORE INSERT ON audit_logs WHEN NEW.action LIKE '%_confirmed' BEGIN SELECT RAISE(ABORT,'fixture confirmation audit failure'); END;").unwrap();
    assert!(complete(&db, &op).is_err());
    assert_eq!(owned_count(&db), 0);
    let current = list(&db, "tenant").unwrap().remove(0);
    assert_eq!(current.state, "applying");
    assert_eq!(current.binding_id.as_deref(), Some("binding-one"));
    unknown(&db, &op, "Final audit checkpoint did not commit").unwrap();
    assert!(start(&db, "create").is_err());
    let current = list(&db, "tenant").unwrap().remove(0);
    assert_eq!(current.state, "unknown_result");
    let log = crate::mec::db::audit_store::AuditStore::new(db)
        .count_recent(10)
        .unwrap();
    assert_eq!(log.len(), 2);
    assert!(log
        .iter()
        .all(|log| log.source_ip.as_deref() == Some("127.0.0.1")));
    assert!(log
        .iter()
        .any(|log| log.input.as_ref().unwrap()["binding_id"] == "binding-one"));
}
#[test]
fn ownership_is_exact_and_terminal_operations_cannot_be_rewritten() {
    let db = crate::mec::db::open_in_memory().unwrap();
    let op = start(&db, "create").unwrap();
    observe(&db, &op, "binding-one").unwrap();
    complete(&db, &op).unwrap();
    assert_eq!(owned_count(&db), 1);
    assert!(complete(&db, &op).is_err());
    assert!(observe(&db, &op, "other").is_err());
    assert!(unknown(&db, &op, "late failure").is_err());
    assert!(start(&db, "create").is_err());
    let mut foreign = binding();
    foreign.user_id = "someone-else".into();
    assert!(begin(
        &db,
        "tenant",
        "https://rancher.example",
        &foreign,
        "delete",
        "admin",
        None,
        None
    )
    .is_err());
    let delete = start(&db, "delete").unwrap();
    complete(&db, &delete).unwrap();
    let state: String = db
        .lock()
        .unwrap()
        .query_row("SELECT state FROM owned_project_bindings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(state, "revoked");
    assert!(start(&db, "create").is_ok());
}
#[test]
fn concurrent_intents_have_one_writer_and_do_not_evict_unknown_outcomes() {
    let db = crate::mec::db::open_in_memory().unwrap();
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let db = db.clone();
            std::thread::spawn(move || start(&db, "create"))
        })
        .collect();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let op = results.into_iter().find_map(Result::ok).unwrap();
    unknown(&db, &op, "Response lost").unwrap();
    assert!(start(&db, "delete").is_err());
    assert!(begin(
        &db,
        "tenant",
        "https://different.example",
        &binding(),
        "create",
        "admin",
        None,
        None
    )
    .is_ok());
}
#[test]
fn interrupted_mutations_survive_restart_without_replay_or_losing_observed_ids() {
    let dir = std::env::temp_dir().join(format!("nabiman-membership-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("state.db");
    let db = crate::mec::db::open(path.clone()).unwrap();
    let op = start(&db, "create").unwrap();
    observe(&db, &op, "binding-one").unwrap();
    drop(db);
    let owner = crate::mec::db::open_owned(path).unwrap();
    let current = list(&owner.handle, "tenant").unwrap().remove(0);
    assert_eq!(current.state, "unknown_result");
    assert_eq!(current.binding_id.as_deref(), Some("binding-one"));
    assert!(start(&owner.handle, "create").is_err());
    assert_eq!(owned_count(&owner.handle), 0);
    drop(owner);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn interrupted_recovery_audit_failure_aborts_without_partially_changing_state() {
    let db = crate::mec::db::open_in_memory().unwrap();
    start(&db, "create").unwrap();
    db.lock().unwrap().execute_batch("CREATE TRIGGER reject_recovery_audit BEFORE INSERT ON audit_logs BEGIN SELECT RAISE(ABORT,'fixture recovery audit failure'); END;").unwrap();
    assert!(recover_interrupted(&db).is_err());
    assert_eq!(list(&db, "tenant").unwrap()[0].state, "applying");
}
fn review() -> Reconciliation<'static> {
    Reconciliation {
        actor: "reviewer",
        reason: "Verified exact external binding",
        observed_at: "2026-10-05T00:00:00Z",
        source_ip: Some("127.0.0.2".into()),
        user_agent: Some("review fixture".into()),
    }
}
#[test]
fn reconciled_ownership_is_atomic_audited_and_does_not_rewrite_terminal_history() {
    let db = crate::mec::db::open_in_memory().unwrap();
    let op = start(&db, "create").unwrap();
    observe(&db, &op, "binding-one").unwrap();
    unknown(&db, &op, "Readback unavailable").unwrap();
    let current = get(&db, "tenant", &op.id).unwrap();
    assert!(get(&db, "foreign", &op.id).is_err());
    db.lock().unwrap().execute_batch("CREATE TRIGGER reject_reconcile BEFORE INSERT ON audit_logs WHEN NEW.action LIKE '%_reconciled' BEGIN SELECT RAISE(ABORT,'fixture reconciliation audit failure'); END;").unwrap();
    assert!(reconcile_complete(&db, &current, review()).is_err());
    assert_eq!(get(&db, "tenant", &op.id).unwrap().state, "unknown_result");
    assert_eq!(owned_count(&db), 0);
    db.lock()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_reconcile")
        .unwrap();
    let mut changed = current.clone();
    changed.binding_id = Some("foreign".into());
    assert!(reconcile_complete(&db, &changed, review()).is_err());
    changed = current.clone();
    changed.updated_at = "stale".into();
    assert!(reconcile_complete(&db, &changed, review()).is_err());
    reconcile_complete(&db, &current, review()).unwrap();
    assert_eq!(owned_count(&db), 1);
    assert!(get(&db, "tenant", &op.id).unwrap().error.is_none());
    assert!(reconcile_complete(&db, &current, review()).is_err());
    let logs = AuditStore::new(db.clone()).count_recent(10).unwrap();
    let log = logs
        .iter()
        .find(|l| l.action.ends_with("_reconciled"))
        .unwrap();
    assert_eq!(log.user, "reviewer");
    assert_eq!(log.source_ip.as_deref(), Some("127.0.0.2"));
    let output = log.output.as_ref().unwrap();
    assert_eq!(output["original_actor"], "admin");
    assert_eq!(output["provider_write_replayed"], false);
    assert_eq!(output["prior_error"], "Readback unavailable");
    let delete = start(&db, "delete").unwrap();
    unknown(&db, &delete, "Accepted but not yet absent").unwrap();
    reconcile_complete(&db, &get(&db, "tenant", &delete.id).unwrap(), review()).unwrap();
    let status: String = db
        .lock()
        .unwrap()
        .query_row("SELECT state FROM owned_project_bindings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "revoked");
}
#[test]
fn reconciliation_never_adopts_a_binding_without_a_durable_candidate_or_while_applying() {
    let db = crate::mec::db::open_in_memory().unwrap();
    let op = start(&db, "create").unwrap();
    assert!(reconcile_complete(&db, &op, review()).is_err());
    unknown(&db, &op, "Response lost before ID checkpoint").unwrap();
    assert!(reconcile_complete(&db, &get(&db, "tenant", &op.id).unwrap(), review()).is_err());
    assert!(start(&db, "create").is_err());
    assert_eq!(owned_count(&db), 0);
}
