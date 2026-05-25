use super::DbHandle;
use crate::models::mec::{Job, JobKind, JobStatus, JobStep};
use chrono::{DateTime, Utc};
use rusqlite::params;

pub struct JobStore {
    db: DbHandle,
}

impl JobStore {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }

    pub fn insert(&self, job: &Job) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO jobs
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

    pub fn update_status(&self, id: &str, status: &JobStatus) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
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
        Ok(())
    }

    pub fn update_steps(&self, id: &str, steps: &[JobStep]) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET steps = ?1 WHERE id = ?2",
            params![serde_json::to_string(steps).unwrap_or_default(), id],
        )?;
        Ok(())
    }

    pub fn set_result(&self, id: &str, result: &serde_json::Value) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET result = ?1 WHERE id = ?2",
            params![result.to_string(), id],
        )?;
        Ok(())
    }

    pub fn set_error(&self, id: &str, error: &str) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET error = ?1 WHERE id = ?2",
            params![error, id],
        )?;
        Ok(())
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

    let kind: JobKind = serde_json::from_str(&kind_str).unwrap_or(JobKind::TenantDelete {
        tenant_id: "unknown".into(),
    });
    let status: JobStatus =
        serde_json::from_str(&status_str).unwrap_or(JobStatus::Failed {
            error: "corrupt status".into(),
        });
    let steps: Vec<JobStep> = steps_str
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let result: Option<serde_json::Value> =
        result_str.and_then(|s| serde_json::from_str(&s).ok());

    Ok(Job {
        id: row.get(0)?,
        kind,
        status,
        steps,
        result,
        error: row.get(5)?,
        created_by: row.get(6)?,
        started_at: parse_time(&started),
        completed_at: completed.as_deref().map(parse_time),
    })
}

fn parse_time(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
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
            kind: JobKind::TenantDelete { tenant_id: "foo".into() },
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
            kind: JobKind::TenantDelete { tenant_id: "x".into() },
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
