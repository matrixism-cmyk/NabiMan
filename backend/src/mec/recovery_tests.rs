use super::*;
use crate::mec::{services::ServiceBundle, tenant_orchestrator::TenantOrchestrator};
use crate::models::mec::{
    AllocationType, CreateTenantRequest, NodeAllocation, ResourceQuota, TenantOptions,
};
use std::sync::Arc;
fn spec() -> CreateTenantRequest {
    CreateTenantRequest {
        tenant_id: "inspect-test".into(),
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
async fn created(state: &MecState) -> Tenant {
    TenantOrchestrator::new(
        state.services.clone(),
        state.audit.clone(),
        state.jobs.clone(),
        state.tenants.clone(),
    )
    .create(spec(), "operator")
    .await
    .unwrap()
}
fn failed_job(state: &MecState) -> String {
    let job = state
        .jobs
        .create(
            JobKind::TenantCreate {
                spec: serde_json::to_value(spec()).unwrap(),
            },
            "operator",
            vec![],
        )
        .unwrap();
    state
        .jobs
        .fail(&job.id, "injected interruption".into())
        .unwrap();
    job.id
}
#[tokio::test]
async fn present_resources_are_observed_without_changing_jobs_or_profiles() {
    let state = MecState::mocks();
    created(&state).await;
    let id = failed_job(&state);
    let before = serde_json::to_value(state.tenants.get("inspect-test").unwrap()).unwrap();
    let job_before = serde_json::to_value(state.job_store.get(&id).unwrap()).unwrap();
    let report = inspect(&state, &id).await.unwrap();
    assert!(report
        .observations
        .iter()
        .all(|o| matches!(o.state, ObservationState::Present)
            && o.expected_identity_matches == Some(true)));
    assert!(report.manual_verification_required && !report.automatic_replay);
    assert_eq!(
        serde_json::to_value(state.tenants.get("inspect-test").unwrap()).unwrap(),
        before
    );
    assert_eq!(
        serde_json::to_value(state.job_store.get(&id).unwrap()).unwrap(),
        job_before
    );
}
#[tokio::test]
async fn unavailable_sources_are_unknown_and_preflight_does_not_hide_errors() {
    let state = MecState::mocks();
    created(&state).await;
    let id = failed_job(&state);
    let unavailable = Arc::new(crate::mec::services::unavailable::UnavailableService::new(
        "source", "injected",
    ));
    let state = MecState::new(
        state.db.clone(),
        ServiceBundle {
            kube: unavailable.clone(),
            rancher: unavailable,
            axgate: state.services.axgate.clone(),
            harbor: state.services.harbor.clone(),
        },
    );
    let report = inspect(&state, &id).await.unwrap();
    assert!(report.observations.iter().all(
        |o| matches!(o.state, ObservationState::Unknown) && o.error_code == Some("UNAVAILABLE")
    ));
    let response = crate::mec::handlers::preflight::tenant_delete(
        actix_web::web::Path::from("inspect-test".to_string()),
        actix_web::web::Data::new(state),
    )
    .await;
    assert_eq!(response.status(), 503);
}
#[tokio::test]
async fn missing_id_candidates_do_not_establish_ownership_or_absence() {
    let state = MecState::mocks();
    let id = failed_job(&state);
    state
        .services
        .rancher
        .create_project("c-local", "inspect-test")
        .await
        .unwrap();
    state
        .services
        .rancher
        .create_user("inspect-test-dev", "test-only")
        .await
        .unwrap();
    let report = inspect(&state, &id).await.unwrap();
    assert!(matches!(
        report.observations[0].state,
        ObservationState::Absent
    ));
    for o in &report.observations[1..] {
        assert!(matches!(o.state, ObservationState::Candidates));
        assert!(o.target.is_none() && o.expected_identity_matches.is_none());
        assert_eq!(o.candidates.len(), 1);
    }
}
#[tokio::test]
async fn changed_profile_and_mismatched_resource_identity_are_explicit() {
    let state = MecState::mocks();
    let mut tenant = created(&state).await;
    let foreign = state
        .services
        .rancher
        .create_project("c-local", "different-tenant")
        .await
        .unwrap();
    tenant.rancher_project_id = Some(foreign.id);
    let job = state
        .jobs
        .create(
            JobKind::TenantCreate {
                spec: serde_json::to_value(spec()).unwrap(),
            },
            "operator",
            vec![],
        )
        .unwrap();
    state
        .jobs
        .fail_with_result(
            &job.id,
            "failed".into(),
            Some(serde_json::json!({"tenant":tenant})),
        )
        .unwrap();
    let report = inspect(&state, &job.id).await.unwrap();
    assert!(report.records_conflict);
    assert_eq!(
        report.observations[1].expected_identity_matches,
        Some(false)
    );
    assert!(matches!(
        report.observations[1].state,
        ObservationState::Present
    ));
}
#[tokio::test]
async fn timeout_and_invalid_ids_are_unknown_without_exposing_upstream_secrets() {
    let o = bounded_for(
        "project",
        None,
        std::future::pending(),
        std::time::Duration::from_millis(1),
    )
    .await;
    assert!(matches!(o.state, ObservationState::Unknown));
    assert_eq!(o.error_code, Some("LOOKUP_TIMEOUT"));
    for id in ["../users?token=secret", ".", ".."] {
        let o = inspect_project(&ServiceBundle::mocks(), Some(id.into()), "inspect-test").await;
        assert_eq!(o.error_code, Some("INVALID_RECORDED_ID"));
    }
    let o = unknown(
        "project",
        None,
        ServiceError::Upstream {
            source: "rancher".into(),
            message: "secret-credential".into(),
        },
    );
    assert!(!serde_json::to_string(&o)
        .unwrap()
        .contains("secret-credential"));
}
#[tokio::test]
async fn recorded_id_absence_differs_from_no_id_and_completed_jobs_are_rejected() {
    let state = MecState::mocks();
    let t = created(&state).await;
    let complete = state.job_store.list_recent(1).unwrap().remove(0);
    assert!(matches!(
        inspect(&state, &complete.id).await,
        Err(ServiceError::Conflict(_))
    ));
    state
        .services
        .rancher
        .delete_project(t.rancher_project_id.as_deref().unwrap())
        .await
        .unwrap();
    let id = failed_job(&state);
    let report = inspect(&state, &id).await.unwrap();
    assert!(matches!(
        report.observations[1].state,
        ObservationState::Absent
    ));
    let empty = MecState::mocks();
    let id = failed_job(&empty);
    let report = inspect(&empty, &id).await.unwrap();
    assert!(report.observations[1..]
        .iter()
        .all(|o| matches!(o.state, ObservationState::NotRecorded)));
}
