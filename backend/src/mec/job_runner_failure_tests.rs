use super::*;
use crate::mec::db;

fn fixture() -> (db::DbHandle, Arc<JobStore>, JobRunner) {
    let db = db::open_in_memory().unwrap();
    let store = Arc::new(JobStore::new(db.clone()));
    let runner = JobRunner::new(store.clone());
    (db, store, runner)
}
fn job(runner: &JobRunner) -> Job {
    runner
        .create(
            JobKind::TenantDelete {
                tenant_id: "test".into(),
            },
            "operator",
            vec!["cleanup".into()],
        )
        .unwrap()
}
#[test]
fn failed_creation_never_publishes_or_replaces_jobs() {
    let (db, store, runner) = fixture();
    db.lock()
        .unwrap()
        .execute_batch("PRAGMA query_only=ON")
        .unwrap();
    assert!(runner
        .create(
            JobKind::TenantDelete {
                tenant_id: "test".into()
            },
            "operator",
            vec![]
        )
        .is_err());
    assert!(store.list_recent(10).unwrap().is_empty());
    assert!(runner.channels.lock().unwrap().is_empty());
}
#[test]
fn failed_checkpoint_preserves_memory_storage_and_event_stream() {
    let (db, store, runner) = fixture();
    let job = job(&runner);
    runner.mark_started(&job.id).unwrap();
    let mut steps = job.steps.clone();
    let before = serde_json::to_value(&steps).unwrap();
    let mut rx = runner.subscribe(&job.id).unwrap();
    db.lock()
        .unwrap()
        .execute_batch("PRAGMA query_only=ON")
        .unwrap();
    assert!(runner
        .mark_step_started(&job.id, &mut steps, "cleanup")
        .is_err());
    assert_eq!(serde_json::to_value(&steps).unwrap(), before);
    assert_eq!(
        serde_json::to_value(store.get(&job.id).unwrap().unwrap().steps).unwrap(),
        before
    );
    assert!(matches!(
        rx.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));
}
#[test]
fn completion_is_atomic_and_cannot_be_restarted_or_overwritten() {
    let (db, store, runner) = fixture();
    let job = job(&runner);
    runner.mark_started(&job.id).unwrap();
    let mut steps = job.steps.clone();
    assert!(runner.complete(&job.id, None).is_err());
    runner
        .mark_step_started(&job.id, &mut steps, "cleanup")
        .unwrap();
    runner
        .mark_step_done(&job.id, &mut steps, "cleanup", None)
        .unwrap();
    let mut rx = runner.subscribe(&job.id).unwrap();
    db.lock().unwrap().execute_batch("CREATE TRIGGER block_finish BEFORE UPDATE OF completed_at ON jobs BEGIN SELECT RAISE(ABORT, 'injected storage failure'); END;").unwrap();
    assert!(runner
        .complete(&job.id, Some(serde_json::json!({"ok":true})))
        .is_err());
    let stored = store.get(&job.id).unwrap().unwrap();
    assert_eq!(stored.status, JobStatus::Running);
    assert!(stored.result.is_none() && stored.completed_at.is_none());
    assert!(matches!(
        rx.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));
    db.lock()
        .unwrap()
        .execute_batch("DROP TRIGGER block_finish")
        .unwrap();
    runner
        .complete(&job.id, Some(serde_json::json!({"ok":true})))
        .unwrap();
    assert!(runner.mark_started(&job.id).is_err());
    assert!(runner.fail(&job.id, "late failure".into()).is_err());
    assert_eq!(
        store.get(&job.id).unwrap().unwrap().status,
        JobStatus::Completed
    );
}
#[test]
fn duplicate_identifiers_do_not_replace_original_work() {
    let (_db, store, runner) = fixture();
    let mut duplicate = job(&runner);
    duplicate.created_by = "forged-admin".into();
    assert!(store.insert(&duplicate).is_err());
    assert_eq!(
        store.get(&duplicate.id).unwrap().unwrap().created_by,
        "operator"
    );
    assert!(runner.mark_started("missing-job").is_err());
}

fn tenant() -> crate::models::mec::Tenant {
    use crate::models::mec::*;
    Tenant {
        id: "test".into(),
        display_name: "Test".into(),
        task_name: None,
        contact_email: None,
        namespace: "test".into(),
        rancher_project_id: Some("project-1".into()),
        rancher_user_id: Some("user-1".into()),
        allocation: NodeAllocation {
            alloc_type: AllocationType::Shared,
            node: None,
            gpu_label: None,
            tier: None,
        },
        quota: ResourceQuota::standard(),
        starter_kit: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        status: TenantStatus::Active,
    }
}

#[test]
fn tenant_creation_and_completion_commit_or_rollback_together() {
    use crate::mec::db::{job_store::TenantCompletion, tenant_store::TenantStore};
    let (db, store, runner) = fixture();
    let tenants = TenantStore::new(db.clone());
    let job = runner
        .create(
            JobKind::TenantCreate {
                spec: serde_json::json!({"tenant_id":"test"}),
            },
            "operator",
            vec![],
        )
        .unwrap();
    runner.mark_started(&job.id).unwrap();
    let mut rx = runner.subscribe(&job.id).unwrap();
    db.lock().unwrap().execute_batch("CREATE TRIGGER block_finish BEFORE UPDATE OF completed_at ON jobs BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
    assert!(runner
        .complete_tenant(&job.id, TenantCompletion::Create(&tenant()), None)
        .is_err());
    assert!(tenants.get("test").unwrap().is_none());
    assert_eq!(
        store.get(&job.id).unwrap().unwrap().status,
        JobStatus::Running
    );
    assert!(matches!(
        rx.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));
    db.lock()
        .unwrap()
        .execute_batch("DROP TRIGGER block_finish")
        .unwrap();
    runner
        .complete_tenant(
            &job.id,
            TenantCompletion::Create(&tenant()),
            Some(serde_json::json!({"tenant_id":"test"})),
        )
        .unwrap();
    assert!(tenants.get("test").unwrap().is_some());
    assert_eq!(
        store.get(&job.id).unwrap().unwrap().status,
        JobStatus::Completed
    );
    assert!(matches!(rx.try_recv().unwrap(), JobEvent::Completed { .. }));
    assert!(runner.subscribe(&job.id).is_none());
}

#[test]
fn tenant_deletion_and_completion_commit_or_rollback_together() {
    use crate::mec::db::{job_store::TenantCompletion, tenant_store::TenantStore};
    let (db, store, runner) = fixture();
    let tenants = TenantStore::new(db.clone());
    tenants.upsert(&tenant(), "operator").unwrap();
    let job = runner
        .create(
            JobKind::TenantDelete {
                tenant_id: "test".into(),
            },
            "operator",
            vec![],
        )
        .unwrap();
    runner.mark_started(&job.id).unwrap();
    let mut rx = runner.subscribe(&job.id).unwrap();
    db.lock().unwrap().execute_batch("CREATE TRIGGER block_finish BEFORE UPDATE OF completed_at ON jobs BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
    assert!(runner
        .complete_tenant(&job.id, TenantCompletion::Delete("test"), None)
        .is_err());
    assert_eq!(
        tenants.get("test").unwrap().unwrap().status,
        crate::models::mec::TenantStatus::Active
    );
    assert_eq!(
        store.get(&job.id).unwrap().unwrap().status,
        JobStatus::Running
    );
    assert!(matches!(
        rx.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));
    db.lock()
        .unwrap()
        .execute_batch("DROP TRIGGER block_finish")
        .unwrap();
    runner
        .complete_tenant(&job.id, TenantCompletion::Delete("test"), None)
        .unwrap();
    assert!(tenants.get("test").unwrap().is_none());
    assert_eq!(
        store.get(&job.id).unwrap().unwrap().status,
        JobStatus::Completed
    );
}

#[test]
fn rejected_tenant_write_never_completes_the_job() {
    use crate::mec::db::job_store::TenantCompletion;
    let (db, store, runner) = fixture();
    let job = runner
        .create(
            JobKind::TenantCreate {
                spec: serde_json::json!({"tenant_id":"test"}),
            },
            "operator",
            vec![],
        )
        .unwrap();
    runner.mark_started(&job.id).unwrap();
    db.lock().unwrap().execute_batch("CREATE TRIGGER block_tenant BEFORE INSERT ON tenant_profiles BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
    assert!(runner
        .complete_tenant(&job.id, TenantCompletion::Create(&tenant()), None)
        .is_err());
    assert_eq!(
        store.get(&job.id).unwrap().unwrap().status,
        JobStatus::Running
    );
}

#[test]
fn completion_rejects_unrelated_tenant_or_operation() {
    use crate::mec::db::job_store::TenantCompletion;
    let (_db, store, runner) = fixture();
    let job = runner
        .create(
            JobKind::TenantCreate {
                spec: serde_json::json!({"tenant_id":"different"}),
            },
            "operator",
            vec![],
        )
        .unwrap();
    runner.mark_started(&job.id).unwrap();
    assert!(runner
        .complete_tenant(&job.id, TenantCompletion::Create(&tenant()), None)
        .is_err());
    assert!(runner
        .complete_tenant(&job.id, TenantCompletion::Delete("different"), None)
        .is_err());
    assert_eq!(
        store.get(&job.id).unwrap().unwrap().status,
        JobStatus::Running
    );
}

#[test]
fn corrupt_persisted_jobs_never_become_empty_completed_work() {
    for (column, value) in [
        ("kind", "bad"),
        ("status", "bad"),
        ("steps", "bad"),
        ("result", "bad"),
        ("started_at", "bad"),
        ("completed_at", "bad"),
    ] {
        let (db, store, runner) = fixture();
        let job = job(&runner);
        runner.mark_started(&job.id).unwrap();
        db.lock()
            .unwrap()
            .execute(
                &format!("UPDATE jobs SET {column}=?1 WHERE id=?2"),
                rusqlite::params![value, job.id],
            )
            .unwrap();
        assert!(store.get(&job.id).is_err(), "{column}");
        assert!(store.list_recent(10).is_err(), "{column}");
        assert!(runner.complete(&job.id, None).is_err(), "{column}");
    }
}

#[test]
fn committed_and_rolled_back_tenant_changes_survive_database_reopening() {
    use crate::mec::db::{job_store::TenantCompletion, tenant_store::TenantStore};
    let dir = std::env::temp_dir().join(format!("nabiman-tenant-atomic-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("state.db");
    let create_id;
    let delete_id;
    {
        let db = db::open_persistent(path.clone()).unwrap();
        let store = Arc::new(JobStore::new(db.clone()));
        let runner = JobRunner::new(store);
        let create = runner
            .create(
                JobKind::TenantCreate {
                    spec: serde_json::json!({"tenant_id":"test"}),
                },
                "operator",
                vec![],
            )
            .unwrap();
        create_id = create.id.clone();
        runner.mark_started(&create.id).unwrap();
        runner
            .complete_tenant(&create.id, TenantCompletion::Create(&tenant()), None)
            .unwrap();
        let delete = runner
            .create(
                JobKind::TenantDelete {
                    tenant_id: "test".into(),
                },
                "operator",
                vec![],
            )
            .unwrap();
        delete_id = delete.id.clone();
        runner.mark_started(&delete.id).unwrap();
        db.lock().unwrap().execute_batch("CREATE TRIGGER block_finish BEFORE UPDATE OF completed_at ON jobs BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
        assert!(runner
            .complete_tenant(&delete.id, TenantCompletion::Delete("test"), None)
            .is_err());
    }
    {
        let db = db::open_persistent(path).unwrap();
        let store = JobStore::new(db.clone());
        let tenants = TenantStore::new(db);
        assert_eq!(
            store.get(&create_id).unwrap().unwrap().status,
            JobStatus::Completed
        );
        assert_eq!(
            store.get(&delete_id).unwrap().unwrap().status,
            JobStatus::Running
        );
        assert_eq!(
            tenants
                .get("test")
                .unwrap()
                .unwrap()
                .rancher_project_id
                .as_deref(),
            Some("project-1")
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}
