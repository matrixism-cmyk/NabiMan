use super::DbHandle;
use crate::models::mec::{Job, JobKind, JobStatus, JobStep, JobStepStatus, Tenant};
use chrono::{DateTime, Utc};
use rusqlite::params;

pub enum TenantCompletion<'a> {
    Create(&'a Tenant),
    Delete(&'a str),
}

pub struct JobStore {
    db: DbHandle,
}

impl JobStore {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }

    /// Only called by the exclusive persistent-database owner before serving
    /// requests. Never replay an external operation whose outcome is unknown.
    pub(super) fn recover_interrupted(&self) -> rusqlite::Result<usize> {
        let mut conn = self.db.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let pending = serde_json::to_string(&JobStatus::Pending).unwrap();
        let running = serde_json::to_string(&JobStatus::Running).unwrap();
        let jobs: Vec<Job> = {
            let mut stmt = tx.prepare("SELECT id, kind, status, steps, result, error, created_by, started_at, completed_at FROM jobs WHERE status IN (?1,?2)")?;
            let rows = stmt.query_map(params![pending, running], row_to_job)?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let reason = "Server stopped before the operation was finalized. Verify external resources before reconciliation; automatic replay is disabled.";
        let status = JobStatus::UnknownResult {
            reason: reason.into(),
            detected_at: Utc::now(),
        };
        for job in &jobs {
            require_row(tx.execute(
                "UPDATE jobs SET status=?1,error=?2 WHERE id=?3 AND status IN (?4,?5)",
                params![
                    serde_json::to_string(&status).unwrap(),
                    reason,
                    job.id,
                    pending,
                    running
                ],
            )?)?;
        }
        let failed = crate::models::mec::TenantStatus::Failed {
            reason: reason.into(),
        };
        tx.execute(
            "UPDATE tenant_profiles SET status=?1,updated_at=?2 WHERE status IN (?3,?4)",
            params![
                serde_json::to_string(&failed).unwrap(),
                Utc::now().to_rfc3339(),
                serde_json::to_string(&crate::models::mec::TenantStatus::Provisioning).unwrap(),
                serde_json::to_string(&crate::models::mec::TenantStatus::Terminating).unwrap()
            ],
        )?;
        tx.commit()?;
        Ok(jobs.len())
    }

    pub fn insert(&self, job: &Job) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT INTO jobs
                (id, kind, status, spec, steps, result, error,
                 created_by, started_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                job.id,
                serde_json::to_string(&job.kind).unwrap_or_default(),
                serde_json::to_string(&job.status).unwrap_or_default(),
                job.kind_spec_json(),
                serde_json::to_string(&job.steps).unwrap_or_default(),
                job.result.as_ref().map(|v| v.to_string()),
                job.error.clone(),
                job.created_by,
                job.started_at.to_rfc3339(),
                job.completed_at.map(|d| d.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub fn update_status(&self, id: &str, status: &JobStatus) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        let changed = conn.execute(
            "UPDATE jobs SET status = ?1, completed_at = ?2 WHERE id = ?3",
            params![
                serde_json::to_string(status).unwrap_or_default(),
                if status.is_terminal() {
                    Some(Utc::now().to_rfc3339())
                } else {
                    None
                },
                id,
            ],
        )?;
        require_row(changed)
    }

    pub fn start(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        require_row(conn.execute(
            "UPDATE jobs SET status=?1 WHERE id=?2 AND status=?3",
            params![
                serde_json::to_string(&JobStatus::Running).unwrap(),
                id,
                serde_json::to_string(&JobStatus::Pending).unwrap()
            ],
        )?)
    }

    pub fn update_steps(&self, id: &str, steps: &[JobStep]) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        let changed = conn.execute(
            "UPDATE jobs SET steps = ?1 WHERE id = ?2 AND status = ?3",
            params![
                serde_json::to_string(steps).unwrap_or_default(),
                id,
                serde_json::to_string(&JobStatus::Running).unwrap()
            ],
        )?;
        require_row(changed)
    }

    pub fn finish(
        &self,
        id: &str,
        status: &JobStatus,
        result: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> rusqlite::Result<()> {
        if !status.is_terminal() {
            return Err(rusqlite::Error::InvalidParameterName(
                "Terminal job status required".into(),
            ));
        }
        let mut conn = self.db.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if matches!(status, JobStatus::Completed) {
            Self::check_completion(&tx, id)?;
        }
        Self::finish_on(&tx, id, status, result, error)?;
        tx.commit()
    }

    fn finish_on(
        conn: &rusqlite::Connection,
        id: &str,
        status: &JobStatus,
        result: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> rusqlite::Result<()> {
        let changed = conn.execute(
            "UPDATE jobs SET status=?1, result=?2, error=?3, completed_at=?4 WHERE id=?5 AND status IN (?6, ?7)",
            params![serde_json::to_string(status).unwrap(), result.map(|v| v.to_string()), error, Utc::now().to_rfc3339(), id, serde_json::to_string(&JobStatus::Running).unwrap(),
                serde_json::to_string(&if matches!(status, JobStatus::Completed) { JobStatus::Running } else { JobStatus::Pending }).unwrap()],
        )?;
        require_row(changed)
    }

    fn check_completion(conn: &rusqlite::Connection, id: &str) -> rusqlite::Result<Job> {
        let job = conn.query_row(
            "SELECT id, kind, status, steps, result, error, created_by, started_at, completed_at FROM jobs WHERE id=?1",
            params![id], row_to_job,
        )?;
        if job.status != JobStatus::Running
            || job
                .steps
                .iter()
                .any(|s| s.status != JobStepStatus::Completed)
        {
            return Err(rusqlite::Error::InvalidParameterName(
                "Job is not running with completed steps".into(),
            ));
        }
        Ok(job)
    }

    /// Commit resource bookkeeping and job completion together. No external calls
    /// or event publication occur while this transaction is open.
    pub fn complete_tenant(
        &self,
        id: &str,
        change: TenantCompletion<'_>,
        result: Option<&serde_json::Value>,
    ) -> rusqlite::Result<()> {
        let mut conn = self.db.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let job = Self::check_completion(&tx, id)?;
        match (&job.kind, change) {
            (JobKind::TenantCreate { spec }, TenantCompletion::Create(tenant))
                if spec.get("tenant_id").and_then(|v| v.as_str()) == Some(tenant.id.as_str()) =>
            {
                super::tenant_store::TenantStore::upsert_on(&tx, tenant, &job.created_by)?;
            }
            (JobKind::TenantDelete { tenant_id }, TenantCompletion::Delete(id))
                if tenant_id == id =>
            {
                require_row(tx.execute("DELETE FROM tenant_profiles WHERE id=?1", params![id])?)?;
            }
            _ => {
                return Err(rusqlite::Error::InvalidParameterName(
                    "Tenant does not match the job".into(),
                ))
            }
        }
        Self::finish_on(&tx, id, &JobStatus::Completed, result, None)?;
        tx.commit()
    }

    pub fn get(&self, id: &str) -> rusqlite::Result<Option<Job>> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, kind, status, steps, result, error, created_by, started_at, completed_at
             FROM jobs WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_job(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_recent(&self, limit: u32) -> rusqlite::Result<Vec<Job>> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, kind, status, steps, result, error, created_by, started_at, completed_at
             FROM jobs ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], row_to_job)?;
        rows.collect()
    }
}

fn require_row(changed: usize) -> rusqlite::Result<()> {
    if changed == 1 {
        Ok(())
    } else {
        Err(rusqlite::Error::QueryReturnedNoRows)
    }
}

impl Job {
    fn kind_spec_json(&self) -> String {
        serde_json::to_string(&self.kind).unwrap_or_else(|_| "{}".to_string())
    }
}

fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<Job> {
    let kind_str: String = row.get(1)?;
    let status_str: String = row.get(2)?;
    let steps_str: Option<String> = row.get(3)?;
    let result_str: Option<String> = row.get(4)?;
    let started: String = row.get(7)?;
    let completed: Option<String> = row.get(8)?;

    let kind: JobKind = decode_json(&kind_str, 1)?;
    let status: JobStatus = decode_json(&status_str, 2)?;
    let steps: Vec<JobStep> = decode_json(
        &steps_str.ok_or(rusqlite::Error::InvalidColumnType(
            3,
            "steps".into(),
            rusqlite::types::Type::Null,
        ))?,
        3,
    )?;
    let result: Option<serde_json::Value> = result_str
        .as_deref()
        .map(|v| decode_json(v, 4))
        .transpose()?;

    Ok(Job {
        id: row.get(0)?,
        kind,
        status,
        steps,
        result,
        error: row.get(5)?,
        created_by: row.get(6)?,
        started_at: parse_time(&started, 7)?,
        completed_at: completed.as_deref().map(|v| parse_time(v, 8)).transpose()?,
    })
}

fn decode_json<T: serde::de::DeserializeOwned>(s: &str, column: usize) -> rusqlite::Result<T> {
    serde_json::from_str(s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(column, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn parse_time(s: &str, column: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(e),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mec::db;

    #[test]
    fn roundtrip() {
        let db = db::open_in_memory().unwrap();
        let store = JobStore::new(db);
        let job = Job {
            id: "j-1".into(),
            kind: JobKind::TenantDelete {
                tenant_id: "foo".into(),
            },
            status: JobStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
            created_by: "admin".into(),
            steps: vec![JobStep::pending("cleanup")],
            result: None,
            error: None,
        };
        store.insert(&job).unwrap();
        let got = store.get("j-1").unwrap().unwrap();
        assert_eq!(got.id, "j-1");
        assert_eq!(got.status, JobStatus::Pending);
        assert_eq!(got.steps.len(), 1);
    }

    #[test]
    fn update_status_sets_completed_at() {
        let db = db::open_in_memory().unwrap();
        let store = JobStore::new(db);
        let job = Job {
            id: "j-2".into(),
            kind: JobKind::TenantDelete {
                tenant_id: "x".into(),
            },
            status: JobStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            created_by: "admin".into(),
            steps: vec![],
            result: None,
            error: None,
        };
        store.insert(&job).unwrap();
        store.update_status("j-2", &JobStatus::Completed).unwrap();
        let got = store.get("j-2").unwrap().unwrap();
        assert!(got.completed_at.is_some());
        assert_eq!(got.status, JobStatus::Completed);
    }
}
