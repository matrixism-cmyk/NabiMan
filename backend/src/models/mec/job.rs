use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub kind: JobKind,
    pub status: JobStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub steps: Vec<JobStep>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobKind {
    TenantCreate { spec: serde_json::Value },
    TenantDelete { tenant_id: String },
    StarterKitDeploy { tenant_id: String },
    StarterKitDelete { tenant_id: String },
    GpuModeSwitch { node: String, from_mode: String, to_mode: String },
    DocGenerate { template: String, tenant_ids: Vec<String> },
    FirewallNatAdd { spec: serde_json::Value },
}

impl JobKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TenantCreate { .. } => "tenant_create",
            Self::TenantDelete { .. } => "tenant_delete",
            Self::StarterKitDeploy { .. } => "starter_kit_deploy",
            Self::StarterKitDelete { .. } => "starter_kit_delete",
            Self::GpuModeSwitch { .. } => "gpu_mode_switch",
            Self::DocGenerate { .. } => "doc_generate",
            Self::FirewallNatAdd { .. } => "firewall_nat_add",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed { error: String },
    Cancelled,
}

impl JobStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed { .. } | Self::Cancelled
        )
    }

    pub fn as_tag(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed { .. } => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStep {
    pub name: String,
    pub status: JobStepStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub progress: Option<u32>,
    pub message: Option<String>,
}

impl JobStep {
    pub fn pending(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: JobStepStatus::Pending,
            started_at: None,
            completed_at: None,
            duration_ms: None,
            progress: None,
            message: None,
        }
    }

    pub fn mark_started(&mut self) {
        self.status = JobStepStatus::Running;
        self.started_at = Some(Utc::now());
    }

    pub fn mark_completed(&mut self, message: Option<String>) {
        let now = Utc::now();
        self.status = JobStepStatus::Completed;
        self.completed_at = Some(now);
        self.message = message;
        if let Some(start) = self.started_at {
            let dur = (now - start).num_milliseconds();
            self.duration_ms = Some(dur.max(0) as u64);
        }
    }

    pub fn mark_failed(&mut self, reason: String) {
        let now = Utc::now();
        self.status = JobStepStatus::Failed;
        self.completed_at = Some(now);
        self.message = Some(reason);
        if let Some(start) = self.started_at {
            let dur = (now - start).num_milliseconds();
            self.duration_ms = Some(dur.max(0) as u64);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobStepStatus {
    Pending,
    Running,
    Completed,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatusResponse {
    pub id: String,
    pub kind: String,
    pub status: JobStatus,
    pub steps: Vec<JobStep>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub progress_percent: u32,
}

impl JobStatusResponse {
    pub fn from_job(job: &Job) -> Self {
        let total = job.steps.len().max(1) as u32;
        let completed = job
            .steps
            .iter()
            .filter(|s| matches!(s.status, JobStepStatus::Completed | JobStepStatus::Skipped))
            .count() as u32;
        let progress = (completed * 100) / total;
        Self {
            id: job.id.clone(),
            kind: job.kind.label().to_string(),
            status: job.status.clone(),
            steps: job.steps.clone(),
            started_at: job.started_at,
            completed_at: job.completed_at,
            progress_percent: progress,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_terminal() {
        assert!(JobStatus::Completed.is_terminal());
        assert!(!JobStatus::Running.is_terminal());
    }

    #[test]
    fn step_lifecycle() {
        let mut s = JobStep::pending("create_namespace");
        assert_eq!(s.status, JobStepStatus::Pending);
        s.mark_started();
        assert_eq!(s.status, JobStepStatus::Running);
        std::thread::sleep(std::time::Duration::from_millis(1));
        s.mark_completed(Some("ok".into()));
        assert_eq!(s.status, JobStepStatus::Completed);
        assert!(s.duration_ms.is_some());
    }

    #[test]
    fn progress_calculation() {
        let job = Job {
            id: "j1".into(),
            kind: JobKind::TenantCreate { spec: serde_json::json!({}) },
            status: JobStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            created_by: "admin".into(),
            steps: vec![
                JobStep {
                    name: "a".into(),
                    status: JobStepStatus::Completed,
                    ..JobStep::pending("a")
                },
                JobStep::pending("b"),
                JobStep::pending("c"),
                JobStep::pending("d"),
            ],
            result: None,
            error: None,
        };
        let r = JobStatusResponse::from_job(&job);
        assert_eq!(r.progress_percent, 25);
    }
}
