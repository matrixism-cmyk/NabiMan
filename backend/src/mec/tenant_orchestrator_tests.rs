use super::*;
use crate::mec::state::MecState;
use crate::models::mec::{AllocationType, NodeAllocation, ResourceQuota, TenantOptions};

fn sample_req() -> CreateTenantRequest {
    CreateTenantRequest {
        tenant_id: "testpoc".into(),
        display_name: "Test".into(),
        task_name: None,
        contact_email: None,
        allocation: NodeAllocation {
            alloc_type: AllocationType::Shared,
            node: None,
            gpu_label: None,
            tier: None,
        },
        quota: ResourceQuota::standard(),
        options: TenantOptions::default(),
    }
}

#[tokio::test]
async fn create_roundtrip() {
    let state = MecState::mocks();
    let orch = TenantOrchestrator::new(
        state.services.clone(),
        state.audit.clone(),
        state.jobs.clone(),
        state.tenants.clone(),
    );
    let tenant = orch.create(sample_req(), "admin").await.unwrap();
    assert_eq!(tenant.id, "testpoc");
    assert_eq!(tenant.status, TenantStatus::Active);
    assert!(state.tenants.get("testpoc").unwrap().is_some());
}

#[tokio::test]
async fn delete_removes_tenant() {
    let state = MecState::mocks();
    let orch = TenantOrchestrator::new(
        state.services.clone(),
        state.audit.clone(),
        state.jobs.clone(),
        state.tenants.clone(),
    );
    orch.create(sample_req(), "admin").await.unwrap();
    orch.delete("testpoc", "admin").await.unwrap();
    assert!(state.tenants.get("testpoc").unwrap().is_none());
}
#[tokio::test]
async fn job_storage_failure_prevents_namespace_creation() {
    let state = MecState::mocks();
    state
        .db
        .lock()
        .unwrap()
        .execute_batch("PRAGMA query_only=ON")
        .unwrap();
    let orch = TenantOrchestrator::new(
        state.services.clone(),
        state.audit.clone(),
        state.jobs.clone(),
        state.tenants.clone(),
    );
    assert!(orch.create(sample_req(), "operator").await.is_err());
    assert!(!state
        .services
        .kube
        .list_namespaces()
        .await
        .unwrap()
        .iter()
        .any(|ns| ns.name == "testpoc"));
}

#[tokio::test]
async fn upstream_delete_failure_preserves_tenant_and_reports_failed_job() {
    let state = MecState::mocks();
    let orch = TenantOrchestrator::new(
        state.services.clone(),
        state.audit.clone(),
        state.jobs.clone(),
        state.tenants.clone(),
    );
    orch.create(sample_req(), "operator").await.unwrap();
    let services = super::super::services::ServiceBundle {
        kube: Arc::new(
            super::super::services::unavailable::UnavailableService::new(
                "kubernetes",
                "injected outage",
            ),
        ),
        rancher: state.services.rancher.clone(),
        axgate: state.services.axgate.clone(),
        harbor: state.services.harbor.clone(),
    };
    let failing = TenantOrchestrator::new(
        Arc::new(services),
        state.audit.clone(),
        state.jobs.clone(),
        state.tenants.clone(),
    );
    assert!(failing.delete("testpoc", "operator").await.is_err());
    assert!(matches!(
        state.tenants.get("testpoc").unwrap().unwrap().status,
        TenantStatus::Failed { .. }
    ));
    let jobs = state.job_store.list_recent(10).unwrap();
    let delete = jobs
        .iter()
        .find(|job| matches!(job.kind, JobKind::TenantDelete { .. }))
        .unwrap();
    assert!(matches!(
        delete.status,
        crate::models::mec::JobStatus::Failed { .. }
    ));
}

fn orchestrator(state: &MecState) -> TenantOrchestrator {
    TenantOrchestrator::new(
        state.services.clone(),
        state.audit.clone(),
        state.jobs.clone(),
        state.tenants.clone(),
    )
}

#[tokio::test]
async fn partial_creation_retains_returned_resource_ids() {
    for (blocked_step, expect_user) in [("create_rancher_user", false), ("create_prtb", true)] {
        let state = MecState::mocks();
        let sql = format!("CREATE TRIGGER block_checkpoint BEFORE UPDATE OF steps ON jobs WHEN EXISTS(SELECT 1 FROM json_each(NEW.steps) WHERE json_extract(value, '$.name')='{blocked_step}' AND json_extract(value, '$.status')='running') BEGIN SELECT RAISE(ABORT, 'injected checkpoint failure'); END;");
        state.db.lock().unwrap().execute_batch(&sql).unwrap();
        assert!(orchestrator(&state)
            .create(sample_req(), "operator")
            .await
            .is_err());
        let tenant = state.tenants.get("testpoc").unwrap().unwrap();
        assert!(matches!(tenant.status, TenantStatus::Failed { .. }));
        assert!(tenant.rancher_project_id.is_some(), "{blocked_step}");
        assert_eq!(
            tenant.rancher_user_id.is_some(),
            expect_user,
            "{blocked_step}"
        );
        assert_eq!(tenant.namespace, "testpoc");
        let jobs = state.job_store.list_recent(10).unwrap();
        assert!(matches!(
            jobs[0].status,
            crate::models::mec::JobStatus::Failed { .. }
        ));
    }
}

#[tokio::test]
async fn failed_reservation_prevents_all_resource_creation() {
    let state = MecState::mocks();
    state.db.lock().unwrap().execute_batch("CREATE TRIGGER block_reservation BEFORE INSERT ON tenant_profiles BEGIN SELECT RAISE(ABORT, 'injected reservation failure'); END;").unwrap();
    assert!(orchestrator(&state)
        .create(sample_req(), "operator")
        .await
        .is_err());
    assert!(!state
        .services
        .kube
        .list_namespaces()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "testpoc"));
    assert!(state.tenants.get("testpoc").unwrap().is_none());
    assert!(matches!(
        state.job_store.list_recent(1).unwrap()[0].status,
        crate::models::mec::JobStatus::Failed { .. }
    ));
}

#[tokio::test]
async fn existing_profile_is_not_overwritten_even_when_namespace_is_absent() {
    let state = MecState::mocks();
    let orch = orchestrator(&state);
    let mut tenant = orch.create(sample_req(), "operator").await.unwrap();
    state
        .services
        .kube
        .delete_namespace("testpoc")
        .await
        .unwrap();
    tenant.status = TenantStatus::Failed {
        reason: "Earlier operation requires reconciliation".into(),
    };
    state.tenants.upsert(&tenant, "operator").unwrap();
    let before = serde_json::to_value(&tenant).unwrap();
    assert!(matches!(
        orch.create(sample_req(), "other-user").await,
        Err(super::super::services::ServiceError::Conflict(_))
    ));
    assert_eq!(
        serde_json::to_value(state.tenants.get("testpoc").unwrap().unwrap()).unwrap(),
        before
    );
    assert!(!state
        .services
        .kube
        .list_namespaces()
        .await
        .unwrap()
        .iter()
        .any(|n| n.name == "testpoc"));
}

#[tokio::test]
async fn deletion_rejects_in_progress_profiles_before_creating_a_job() {
    let state = MecState::mocks();
    let orch = orchestrator(&state);
    let mut tenant = orch.create(sample_req(), "operator").await.unwrap();
    for status in [TenantStatus::Provisioning, TenantStatus::Terminating] {
        tenant.status = status;
        state.tenants.upsert(&tenant, "operator").unwrap();
        let count = state.job_store.list_recent(100).unwrap().len();
        assert!(matches!(
            orch.delete("testpoc", "operator").await,
            Err(super::super::services::ServiceError::Conflict(_))
        ));
        assert_eq!(state.job_store.list_recent(100).unwrap().len(), count);
        assert!(state
            .services
            .kube
            .list_namespaces()
            .await
            .unwrap()
            .iter()
            .any(|n| n.name == "testpoc"));
    }
}

#[tokio::test]
async fn profile_checkpoint_failure_preserves_known_ids_in_failed_job() {
    let state = MecState::mocks();
    state.db.lock().unwrap().execute_batch("CREATE TRIGGER block_project_checkpoint BEFORE UPDATE ON tenant_profiles WHEN NEW.rancher_project_id IS NOT NULL BEGIN SELECT RAISE(ABORT, 'injected profile checkpoint failure'); END;").unwrap();
    assert!(orchestrator(&state)
        .create(sample_req(), "operator")
        .await
        .is_err());
    let job = state.job_store.list_recent(1).unwrap().remove(0);
    assert!(matches!(
        job.status,
        crate::models::mec::JobStatus::Failed { .. }
    ));
    let snapshot = &job.result.unwrap()["tenant"];
    assert!(snapshot["rancher_project_id"].as_str().is_some());
    assert!(snapshot["rancher_user_id"].is_null());
    assert_eq!(snapshot["status"]["state"], "failed");
    assert_eq!(
        state.tenants.get("testpoc").unwrap().unwrap().status,
        TenantStatus::Provisioning
    );
    assert!(job
        .error
        .unwrap()
        .contains("resource checkpoint could not be saved"));
}

#[tokio::test]
async fn bootstrap_secret_is_returned_without_persisting_it_in_records() {
    let state = MecState::mocks();
    let created = orchestrator(&state)
        .create_with_credentials(sample_req(), "operator")
        .await
        .unwrap();
    let password = &created.initial_credentials.password;
    assert_eq!(password.len(), 48);
    assert_eq!(created.initial_credentials.username, "testpoc-dev");
    let records = serde_json::json!({"tenants":state.tenants.list().unwrap(), "jobs":state.job_store.list_recent(100).unwrap(), "audit":state.audit_store.count_recent(100).unwrap()});
    assert!(!records.to_string().contains(password));
    let user = state
        .services
        .rancher
        .get_user(created.tenant.rancher_user_id.as_deref().unwrap())
        .await
        .unwrap();
    assert!(user.must_change_password);
}
