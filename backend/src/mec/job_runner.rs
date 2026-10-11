use super::db::job_store::JobStore;
use super::services::{ServiceError, ServiceResult};
use crate::models::mec::{Job, JobKind, JobStatus, JobStep};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

const STREAM_CAPACITY: usize = 64;

pub type ProgressTx = broadcast::Sender<JobEvent>;
pub type ProgressRx = broadcast::Receiver<JobEvent>;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum JobEvent {
    Started {
        job_id: String,
    },
    StepStarted {
        job_id: String,
        step: String,
    },
    StepCompleted {
        job_id: String,
        step: String,
        duration_ms: u64,
        message: Option<String>,
    },
    #[allow(dead_code)]
    StepFailed {
        job_id: String,
        step: String,
        reason: String,
    },
    Completed {
        job_id: String,
        result: Option<serde_json::Value>,
    },
    Failed {
        job_id: String,
        error: String,
    },
}

pub struct JobRunner {
    store: Arc<JobStore>,
    channels: Mutex<HashMap<String, ProgressTx>>,
}

impl JobRunner {
    pub fn new(store: Arc<JobStore>) -> Self {
        Self {
            store,
            channels: Mutex::new(HashMap::new()),
        }
    }

    pub fn create(
        &self,
        kind: JobKind,
        created_by: &str,
        steps: Vec<String>,
    ) -> ServiceResult<Job> {
        let job = Job {
            id: format!("job-{}", uuid::Uuid::new_v4().simple()),
            kind,
            status: JobStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
            created_by: created_by.to_string(),
            steps: steps.iter().map(|s| JobStep::pending(s)).collect(),
            result: None,
            error: None,
        };
        self.store.insert(&job)?;
        let (tx, _rx) = broadcast::channel(STREAM_CAPACITY);
        self.channels.lock().unwrap().insert(job.id.clone(), tx);
        Ok(job)
    }

    pub fn subscribe(&self, job_id: &str) -> Option<ProgressRx> {
        self.channels
            .lock()
            .unwrap()
            .get(job_id)
            .map(|tx| tx.subscribe())
    }

    pub fn mark_started(&self, job_id: &str) -> ServiceResult<()> {
        self.store.start(job_id)?;
        self.emit(
            job_id,
            JobEvent::Started {
                job_id: job_id.into(),
            },
        );
        Ok(())
    }

    pub fn mark_step_started(
        &self,
        job_id: &str,
        steps: &mut [JobStep],
        name: &str,
    ) -> ServiceResult<()> {
        let mut next = steps.to_vec();
        if let Some(s) = next.iter_mut().find(|s| s.name == name) {
            s.mark_started();
        }
        if !next.iter().any(|s| s.name == name) {
            return Err(ServiceError::InvalidInput("Unknown job step".into()));
        }
        self.store.update_steps(job_id, &next)?;
        steps.clone_from_slice(&next);
        self.emit(
            job_id,
            JobEvent::StepStarted {
                job_id: job_id.into(),
                step: name.into(),
            },
        );
        Ok(())
    }

    pub fn mark_step_done(
        &self,
        job_id: &str,
        steps: &mut [JobStep],
        name: &str,
        message: Option<String>,
    ) -> ServiceResult<()> {
        let mut next = steps.to_vec();
        let dur = if let Some(s) = next.iter_mut().find(|s| s.name == name) {
            s.mark_completed(message.clone());
            s.duration_ms.unwrap_or(0)
        } else {
            0
        };
        if !next.iter().any(|s| s.name == name) {
            return Err(ServiceError::InvalidInput("Unknown job step".into()));
        }
        self.store.update_steps(job_id, &next)?;
        steps.clone_from_slice(&next);
        self.emit(
            job_id,
            JobEvent::StepCompleted {
                job_id: job_id.into(),
                step: name.into(),
                duration_ms: dur,
                message,
            },
        );
        Ok(())
    }

    #[allow(dead_code)]
    pub fn mark_step_failed(
        &self,
        job_id: &str,
        steps: &mut [JobStep],
        name: &str,
        reason: String,
    ) -> ServiceResult<()> {
        let mut next = steps.to_vec();
        if let Some(s) = next.iter_mut().find(|s| s.name == name) {
            s.mark_failed(reason.clone());
        }
        if !next.iter().any(|s| s.name == name) {
            return Err(ServiceError::InvalidInput("Unknown job step".into()));
        }
        self.store.update_steps(job_id, &next)?;
        steps.clone_from_slice(&next);
        self.emit(
            job_id,
            JobEvent::StepFailed {
                job_id: job_id.into(),
                step: name.into(),
                reason,
            },
        );
        Ok(())
    }

    #[allow(dead_code)]
    pub fn complete(&self, job_id: &str, result: Option<serde_json::Value>) -> ServiceResult<()> {
        self.store
            .finish(job_id, &JobStatus::Completed, result.as_ref(), None)?;
        self.emit_completed(job_id, result);
        Ok(())
    }

    pub fn complete_tenant(
        &self,
        job_id: &str,
        change: super::db::job_store::TenantCompletion<'_>,
        result: Option<serde_json::Value>,
    ) -> ServiceResult<()> {
        self.store
            .complete_tenant(job_id, change, result.as_ref())?;
        self.emit_completed(job_id, result);
        Ok(())
    }

    fn emit_completed(&self, job_id: &str, result: Option<serde_json::Value>) {
        self.emit(
            job_id,
            JobEvent::Completed {
                job_id: job_id.into(),
                result,
            },
        );
        self.channels.lock().unwrap().remove(job_id);
    }

    pub fn fail(&self, job_id: &str, error: String) -> ServiceResult<()> {
        self.fail_with_result(job_id, error, None)
    }

    pub fn fail_with_result(
        &self,
        job_id: &str,
        error: String,
        result: Option<serde_json::Value>,
    ) -> ServiceResult<()> {
        self.store.finish(
            job_id,
            &JobStatus::Failed {
                error: error.clone(),
            },
            result.as_ref(),
            Some(&error),
        )?;
        self.emit(
            job_id,
            JobEvent::Failed {
                job_id: job_id.into(),
                error,
            },
        );
        self.channels.lock().unwrap().remove(job_id);
        Ok(())
    }

    fn emit(&self, job_id: &str, event: JobEvent) {
        if let Some(tx) = self.channels.lock().unwrap().get(job_id) {
            let _ = tx.send(event);
        }
    }

    #[allow(dead_code)]
    pub fn store(&self) -> &JobStore {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mec::db;

    #[tokio::test]
    async fn create_and_complete() {
        let db = db::open_in_memory().unwrap();
        let store = Arc::new(JobStore::new(db));
        let runner = JobRunner::new(store.clone());
        let job = runner
            .create(
                JobKind::TenantDelete {
                    tenant_id: "x".into(),
                },
                "admin",
                vec!["cleanup".into()],
            )
            .unwrap();
        let mut rx = runner.subscribe(&job.id).unwrap();
        runner.mark_started(&job.id).unwrap();
        let mut steps = job.steps.clone();
        runner
            .mark_step_started(&job.id, &mut steps, "cleanup")
            .unwrap();
        runner
            .mark_step_done(&job.id, &mut steps, "cleanup", None)
            .unwrap();
        runner
            .complete(&job.id, Some(serde_json::json!({"ok": true})))
            .unwrap();

        let mut events = Vec::new();
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }
        assert!(!events.is_empty());
        let stored = store.get(&job.id).unwrap().unwrap();
        assert_eq!(stored.status, JobStatus::Completed);
    }
}

#[cfg(test)]
#[path = "job_runner_failure_tests.rs"]
mod failure_tests;
