//! Read-only evidence gathering. This report never authorizes replay or deletion.
use super::services::{ServiceBundle, ServiceError, ServiceResult};
use super::MecState;
use crate::models::mec::{validate_tenant_id, JobKind, JobStatus, Tenant, TenantStatus};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationState {
    Present,
    Absent,
    Unknown,
    NotRecorded,
    Candidates,
}
#[derive(Debug, Serialize)]
pub struct Observation {
    pub resource: &'static str,
    pub target: Option<String>,
    pub state: ObservationState,
    pub expected_identity_matches: Option<bool>,
    pub candidates: Vec<String>,
    pub error_code: Option<&'static str>,
}
#[derive(Debug, Serialize, PartialEq)]
pub struct RecoveryRecord {
    pub namespace: String,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub status: TenantStatus,
}
impl From<&Tenant> for RecoveryRecord {
    fn from(t: &Tenant) -> Self {
        Self {
            namespace: t.namespace.clone(),
            project_id: t.rancher_project_id.clone(),
            user_id: t.rancher_user_id.clone(),
            status: t.status.clone(),
        }
    }
}
#[derive(Debug, Serialize)]
pub struct RecoveryReport {
    pub job_id: String,
    pub tenant_id: String,
    pub operation: &'static str,
    pub checked_at: DateTime<Utc>,
    pub configured_mode: String,
    pub profile: Option<RecoveryRecord>,
    pub job_snapshot: Option<RecoveryRecord>,
    pub records_conflict: bool,
    pub observations: Vec<Observation>,
    pub automatic_replay: bool,
    pub manual_verification_required: bool,
}
fn observation(
    resource: &'static str,
    target: Option<String>,
    state: ObservationState,
) -> Observation {
    Observation {
        resource,
        target,
        state,
        expected_identity_matches: None,
        candidates: vec![],
        error_code: None,
    }
}
fn unknown(resource: &'static str, target: Option<String>, error: ServiceError) -> Observation {
    let mut o = observation(resource, target, ObservationState::Unknown);
    // Upstream bodies may include credentials; expose only the classified code.
    o.error_code = Some(error.code());
    o
}

pub async fn inspect(state: &MecState, job_id: &str) -> ServiceResult<RecoveryReport> {
    let job = state
        .job_store
        .get(job_id)?
        .ok_or_else(|| ServiceError::NotFound("Job not found".into()))?;
    if !matches!(
        job.status,
        JobStatus::UnknownResult { .. } | JobStatus::Failed { .. }
    ) {
        return Err(ServiceError::Conflict(
            "Recovery evidence is available for interrupted or failed jobs".into(),
        ));
    }
    let (tenant_id, operation) = match &job.kind {
        JobKind::TenantCreate { spec } => (
            spec.get("tenant_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ServiceError::InvalidInput("Job tenant ID is missing".into()))?
                .to_string(),
            "tenant_create",
        ),
        JobKind::TenantDelete { tenant_id } => (tenant_id.clone(), "tenant_delete"),
        _ => {
            return Err(ServiceError::InvalidInput(
                "Recovery inspection for this job kind is not supported".into(),
            ))
        }
    };
    validate_tenant_id(&tenant_id).map_err(|e| ServiceError::InvalidInput(e.into()))?;
    let profile = state
        .tenants
        .get(&tenant_id)?
        .as_ref()
        .map(RecoveryRecord::from);
    let snapshot = job.result.as_ref().and_then(|v| v.get("tenant"));
    let job_snapshot = match snapshot {
        Some(value) => {
            let t: Tenant = serde_json::from_value(value.clone())?;
            if t.id != tenant_id {
                return Err(ServiceError::Conflict(
                    "Job snapshot tenant does not match the job".into(),
                ));
            }
            Some(RecoveryRecord::from(&t))
        }
        None => None,
    };
    let records_conflict = match (&profile, &job_snapshot) {
        (Some(a), Some(b)) => {
            a.namespace != b.namespace
                || matches!((&a.project_id, &b.project_id), (Some(x),Some(y)) if x!=y)
                || matches!((&a.user_id, &b.user_id), (Some(x),Some(y)) if x!=y)
        }
        _ => false,
    };
    let namespace = job_snapshot
        .as_ref()
        .or(profile.as_ref())
        .map(|r| r.namespace.clone())
        .unwrap_or_else(|| tenant_id.clone());
    // Snapshot evidence belongs to this job; the current profile may have changed.
    let project = job_snapshot
        .as_ref()
        .and_then(|r| r.project_id.clone())
        .or_else(|| profile.as_ref().and_then(|r| r.project_id.clone()));
    let user = job_snapshot
        .as_ref()
        .and_then(|r| r.user_id.clone())
        .or_else(|| profile.as_ref().and_then(|r| r.user_id.clone()));
    let (namespace, project, user) = tokio::join!(
        bounded(
            "namespace",
            Some(namespace.clone()),
            inspect_namespace(&state.services, &namespace, &tenant_id)
        ),
        bounded(
            "project",
            project.clone(),
            inspect_project(&state.services, project, &tenant_id)
        ),
        bounded(
            "user",
            user.clone(),
            inspect_user(&state.services, user, &tenant_id)
        ),
    );
    Ok(RecoveryReport {
        job_id: job.id,
        tenant_id,
        operation,
        checked_at: Utc::now(),
        configured_mode: std::env::var("NABIMAN_MEC_MODE").unwrap_or_else(|_| "auto".into()),
        profile,
        job_snapshot,
        records_conflict,
        observations: vec![namespace, project, user],
        automatic_replay: false,
        manual_verification_required: true,
    })
}
async fn bounded(
    resource: &'static str,
    target: Option<String>,
    future: impl std::future::Future<Output = Observation>,
) -> Observation {
    bounded_for(resource, target, future, std::time::Duration::from_secs(10)).await
}
async fn bounded_for(
    resource: &'static str,
    target: Option<String>,
    future: impl std::future::Future<Output = Observation>,
    duration: std::time::Duration,
) -> Observation {
    match tokio::time::timeout(duration, future).await {
        Ok(value) => value,
        Err(_) => {
            let mut o = observation(resource, target, ObservationState::Unknown);
            o.error_code = Some("LOOKUP_TIMEOUT");
            o
        }
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_: .".contains(&c))
        && !id.contains(' ')
}
fn invalid_id(resource: &'static str, id: String) -> Observation {
    let mut o = observation(resource, Some(id), ObservationState::Unknown);
    o.error_code = Some("INVALID_RECORDED_ID");
    o
}
async fn inspect_namespace(
    services: &ServiceBundle,
    namespace: &str,
    tenant_id: &str,
) -> Observation {
    match services.kube.list_namespaces().await {
        Ok(namespaces) => match namespaces.iter().find(|n| n.name == namespace) {
            Some(n) => {
                let mut o = observation(
                    "namespace",
                    Some(namespace.into()),
                    ObservationState::Present,
                );
                o.expected_identity_matches = Some(
                    n.labels.get("tenant").map(String::as_str) == Some(tenant_id)
                        && n.labels.get("managed-by").map(String::as_str) == Some("nabiman"),
                );
                o
            }
            None => observation(
                "namespace",
                Some(namespace.into()),
                ObservationState::Absent,
            ),
        },
        Err(e) => unknown("namespace", Some(namespace.into()), e),
    }
}
async fn inspect_project(
    services: &ServiceBundle,
    id: Option<String>,
    tenant_id: &str,
) -> Observation {
    if let Some(id) = id {
        if !valid_id(&id) {
            return invalid_id("project", id);
        }
        return match services.rancher.get_project(&id).await {
            Ok(p) => {
                let mut o = observation("project", Some(id.clone()), ObservationState::Present);
                o.expected_identity_matches =
                    Some(p.id == id && p.name == tenant_id && p.cluster_id == "c-local");
                o
            }
            Err(ServiceError::NotFound(_)) => {
                observation("project", Some(id), ObservationState::Absent)
            }
            Err(e) => unknown("project", Some(id), e),
        };
    }
    match services.rancher.list_projects("c-local").await {
        Ok(projects) => {
            let mut o = observation("project", None, ObservationState::NotRecorded);
            o.candidates = projects
                .into_iter()
                .filter(|p| p.name == tenant_id && p.cluster_id == "c-local")
                .map(|p| p.id)
                .collect();
            if !o.candidates.is_empty() {
                o.state = ObservationState::Candidates;
            }
            o
        }
        Err(e) => unknown("project", None, e),
    }
}
async fn inspect_user(
    services: &ServiceBundle,
    id: Option<String>,
    tenant_id: &str,
) -> Observation {
    let username = format!("{tenant_id}-dev");
    if let Some(id) = id {
        if !valid_id(&id) {
            return invalid_id("user", id);
        }
        return match services.rancher.get_user(&id).await {
            Ok(u) => {
                let mut o = observation("user", Some(id.clone()), ObservationState::Present);
                o.expected_identity_matches = Some(u.id == id && u.username == username);
                o
            }
            Err(ServiceError::NotFound(_)) => {
                observation("user", Some(id), ObservationState::Absent)
            }
            Err(e) => unknown("user", Some(id), e),
        };
    }
    match services.rancher.list_users().await {
        Ok(users) => {
            let mut o = observation("user", None, ObservationState::NotRecorded);
            o.candidates = users
                .into_iter()
                .filter(|u| u.username == username)
                .map(|u| u.id)
                .collect();
            if !o.candidates.is_empty() {
                o.state = ObservationState::Candidates;
            }
            o
        }
        Err(e) => unknown("user", None, e),
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
