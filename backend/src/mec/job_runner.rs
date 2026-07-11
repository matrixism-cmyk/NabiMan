use super::db::job_store::JobStore;
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

    pub fn create(&self, kind: JobKind, created_by: &str, steps: Vec<String>) -> Job {
        let job = Job {
            id: format!("job-{}", &uuid::Uuid::new_v4().to_string()[..8]),
            kind,
            status: JobStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
            created_by: created_by.to_string(),
            steps: steps.iter().map(|s| JobStep::pending(s)).collect(),
            result: None,
            error: None,
        };
        let _ = self.store.insert(&job);
        let (tx, _rx) = broadcast::channel(STREAM_CAPACITY);
        self.channels.lock().unwrap().insert(job.id.clone(), tx);
        job
    }

    pub fn subscribe(&self, job_id: &str) -> Option<ProgressRx> {
        self.channels.lock().unwrap().get(job_id).map(|tx| tx.subscribe())
    }

    pub fn mark_started(&self, job_id: &str) {
        let _ = self.store.update_status(job_id, &JobStatus::Running);
        self.emit(job_id, JobEvent::Started { job_id: job_id.into() });
    }

    pub fn mark_step_started(&self, job_id: &str, steps: &mut [JobStep], name: &str) {
        if let Some(s) = steps.iter_mut().find(|s| s.name == name) {
            s.mark_started();
        }
        let _ = self.store.update_steps(job_id, steps);
        self.emit(
            job_id,
            JobEvent::StepStarted {
                job_id: job_id.into(),
                step: name.into(),
            },
        );
    }

    pub fn mark_step_done(&self, job_id: &str, steps: &mut [JobStep], name: &str, message: Option<String>) {
        let dur = if let Some(s) = steps.iter_mut().find(|s| s.name == name) {
            s.mark_completed(message.clone());
            s.duration_ms.unwrap_or(0)
        } else {
            0
        };
        let _ = self.store.update_steps(job_id, steps);
        self.emit(
            job_id,
            JobEvent::StepCompleted {
                job_id: job_id.into(),
                step: name.into(),
                duration_ms: dur,
                message,
            },
        );
    }

    #[allow(dead_code)]
    pub fn mark_step_failed(&self, job_id: &str, steps: &mut [JobStep], name: &str, reason: String) {
        if let Some(s) = steps.iter_mut().find(|s| s.name == name) {
            s.mark_failed(reason.clone());
        }
        let _ = self.store.update_steps(job_id, steps);
        self.emit(
            job_id,
            JobEvent::StepFailed {
                job_id: job_id.into(),
                step: name.into(),
                reason,
            },
        );
    }

    pub fn complete(&self, job_id: &str, result: Option<serde_json::Value>) {
        if let Some(v) = &result {
            let _ = self.store.set_result(job_id, v);
        }
        let _ = self.store.update_status(job_id, &JobStatus::Completed);
        self.emit(
            job_id,
            JobEvent::Completed {
                job_id: job_id.into(),
                result,
            },
        );
        self.channels.lock().unwrap().remove(job_id);
    }

    pub fn fail(&self, job_id: &str, error: String) {
        let _ = self.store.set_error(job_id, &error);
        let _ = self.store.update_status(
            job_id,
            &JobStatus::Failed {
                error: error.clone(),
            },
        );
        self.emit(
            job_id,
            JobEvent::Failed {
                job_id: job_id.into(),
                error,
            },
        );
        self.channels.lock().unwrap().remove(job_id);
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
        let job = runner.create(
            JobKind::TenantDelete {
                tenant_id: "x".into(),
            },
            "admin",
            vec!["cleanup".into()],
        );
        let mut rx = runner.subscribe(&job.id).unwrap();
        runner.mark_started(&job.id);
        let mut steps = job.steps.clone();
        runner.mark_step_started(&job.id, &mut steps, "cleanup");
        runner.mark_step_done(&job.id, &mut steps, "cleanup", None);
        runner.complete(&job.id, Some(serde_json::json!({"ok": true})));

        let mut events = Vec::new();
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }
        assert!(!events.is_empty());
        let stored = store.get(&job.id).unwrap().unwrap();
        assert_eq!(stored.status, JobStatus::Completed);
    }
}
