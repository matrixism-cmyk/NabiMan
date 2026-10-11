use super::*;
use crate::models::mec::{Job, JobKind, JobStatus, JobStep, TenantStatus};
use chrono::Utc;

fn job(id: &str, status: JobStatus) -> Job {
    Job {
        id: id.into(),
        kind: JobKind::TenantDelete {
            tenant_id: "test".into(),
        },
        status,
        steps: vec![JobStep::pending("cleanup")],
        created_by: "operator".into(),
        started_at: Utc::now(),
        completed_at: None,
        result: Some(serde_json::json!({"known_resource":"project-1"})),
        error: None,
    }
}
fn profile(handle: &DbHandle, status: &TenantStatus) {
    handle.lock().unwrap().execute("INSERT INTO tenant_profiles(id,display_name,namespace,allocation_json,quota_json,status,created_at,updated_at,rancher_project_id) VALUES('test','Test','test','{}','{}',?1,'2026-10-05T00:00:00Z','2026-10-05T00:00:00Z','project-1')", [serde_json::to_string(status).unwrap()]).unwrap();
}

#[test]
fn interrupted_jobs_preserve_evidence_and_are_not_replayed() {
    let db = open_in_memory().unwrap();
    let store = job_store::JobStore::new(db.clone());
    for (id, status) in [
        ("pending", JobStatus::Pending),
        ("running", JobStatus::Running),
        ("done", JobStatus::Completed),
        (
            "failed",
            JobStatus::Failed {
                error: "prior failure".into(),
            },
        ),
    ] {
        store.insert(&job(id, status)).unwrap();
    }
    let before = serde_json::to_value(store.get("done").unwrap().unwrap()).unwrap();
    profile(&db, &TenantStatus::Terminating);
    assert_eq!(store.recover_interrupted().unwrap(), 2);
    for id in ["pending", "running"] {
        let recovered = store.get(id).unwrap().unwrap();
        assert!(matches!(recovered.status, JobStatus::UnknownResult { .. }));
        assert!(recovered.status.is_terminal());
        assert!(recovered.completed_at.is_none());
        assert_eq!(recovered.result.unwrap()["known_resource"], "project-1");
        assert_eq!(recovered.steps.len(), 1);
        assert!(store.start(id).is_err());
        assert!(store.finish(id, &JobStatus::Completed, None, None).is_err());
        assert!(store
            .finish(
                id,
                &JobStatus::Failed {
                    error: "late".into()
                },
                None,
                Some("late")
            )
            .is_err());
    }
    assert_eq!(
        serde_json::to_value(store.get("done").unwrap().unwrap()).unwrap(),
        before
    );
    let (status, project): (String, String) = db
        .lock()
        .unwrap()
        .query_row(
            "SELECT status,rancher_project_id FROM tenant_profiles WHERE id='test'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(matches!(
        serde_json::from_str::<TenantStatus>(&status).unwrap(),
        TenantStatus::Failed { .. }
    ));
    assert_eq!(project, "project-1");
    assert_eq!(store.recover_interrupted().unwrap(), 0);
}

#[test]
fn failed_recovery_rolls_back_all_jobs_and_profiles() {
    let db = open_in_memory().unwrap();
    let store = job_store::JobStore::new(db.clone());
    store.insert(&job("running", JobStatus::Running)).unwrap();
    profile(&db, &TenantStatus::Provisioning);
    db.lock().unwrap().execute_batch("CREATE TRIGGER block_profile BEFORE UPDATE ON tenant_profiles BEGIN SELECT RAISE(ABORT, 'injected recovery failure'); END;").unwrap();
    assert!(store.recover_interrupted().is_err());
    assert_eq!(
        store.get("running").unwrap().unwrap().status,
        JobStatus::Running
    );
    let status: String = db
        .lock()
        .unwrap()
        .query_row(
            "SELECT status FROM tenant_profiles WHERE id='test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<TenantStatus>(&status).unwrap(),
        TenantStatus::Provisioning
    );
}

#[test]
fn corrupt_active_job_aborts_recovery_without_partial_changes() {
    let db = open_in_memory().unwrap();
    let store = job_store::JobStore::new(db.clone());
    store.insert(&job("valid", JobStatus::Running)).unwrap();
    store.insert(&job("corrupt", JobStatus::Pending)).unwrap();
    db.lock()
        .unwrap()
        .execute("UPDATE jobs SET steps='bad' WHERE id='corrupt'", [])
        .unwrap();
    assert!(store.recover_interrupted().is_err());
    assert_eq!(
        store.get("valid").unwrap().unwrap().status,
        JobStatus::Running
    );
}

#[test]
fn owner_lock_blocks_aliases_and_releases_on_drop() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("nabiman-owner-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("state.db");
    let owned = open_owned(path.clone()).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(matches!(open_owned(path.clone()), Err(e) if e.kind()==std::io::ErrorKind::WouldBlock));
    let alias = dir.join("alias.db");
    std::fs::hard_link(&path, &alias).unwrap();
    assert!(matches!(open_owned(alias), Err(e) if e.kind()==std::io::ErrorKind::WouldBlock));
    drop(owned);
    drop(open_owned(path).unwrap());
    std::fs::remove_dir_all(dir).unwrap();
}
