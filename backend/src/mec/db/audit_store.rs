use super::DbHandle;
use crate::models::mec::{AuditQuery, AuditStatus, MecAuditLog, OperationLog};
use chrono::{DateTime, Utc};
use rusqlite::{params, params_from_iter};

pub struct AuditStore {
    db: DbHandle,
}

impl AuditStore {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }

    pub fn insert(&self, log: &MecAuditLog) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT INTO audit_logs
                (id, timestamp, user, action, resource_type, resource_id,
                 status, duration_ms, input, output, error, operations,
                 source_ip, user_agent)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                log.id,
                log.timestamp.to_rfc3339(),
                log.user,
                log.action,
                log.resource_type,
                log.resource_id,
                log.status.as_str(),
                log.duration_ms as i64,
                log.input.as_ref().map(|v| v.to_string()),
                log.output.as_ref().map(|v| v.to_string()),
                log.error,
                serde_json::to_string(&log.operations).unwrap_or_else(|_| "[]".into()),
                log.source_ip,
                log.user_agent,
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> rusqlite::Result<Option<MecAuditLog>> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, user, action, resource_type, resource_id, status,
                    duration_ms, input, output, error, operations, source_ip, user_agent
             FROM audit_logs WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_log(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn query(&self, q: &AuditQuery) -> rusqlite::Result<Vec<MecAuditLog>> {
        let conn = self.db.lock().unwrap();
        let (sql, args) = build_query(q);
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args.iter()), row_to_log)?;
        rows.collect()
    }

    pub fn count_recent(&self, limit: u32) -> rusqlite::Result<Vec<MecAuditLog>> {
        let q = AuditQuery {
            limit: Some(limit),
            ..Default::default()
        };
        self.query(&q)
    }
}

fn build_query(q: &AuditQuery) -> (String, Vec<String>) {
    let mut sql = String::from(
        "SELECT id, timestamp, user, action, resource_type, resource_id, status,
                duration_ms, input, output, error, operations, source_ip, user_agent
         FROM audit_logs WHERE 1=1",
    );
    let mut args: Vec<String> = Vec::new();
    push_filter(&mut sql, &mut args, "user", &q.user);
    push_filter(&mut sql, &mut args, "action", &q.action);
    push_filter(&mut sql, &mut args, "resource_type", &q.resource_type);
    push_filter(&mut sql, &mut args, "resource_id", &q.resource_id);
    push_filter(&mut sql, &mut args, "status", &q.status);
    if let Some(from) = q.from {
        sql.push_str(" AND timestamp >= ?");
        args.push(from.to_rfc3339());
    }
    if let Some(to) = q.to {
        sql.push_str(" AND timestamp <= ?");
        args.push(to.to_rfc3339());
    }
    sql.push_str(" ORDER BY timestamp DESC");
    let limit = q.limit.unwrap_or(100).min(1000);
    sql.push_str(&format!(" LIMIT {}", limit));
    if let Some(off) = q.offset {
        sql.push_str(&format!(" OFFSET {}", off));
    }
    (sql, args)
}

fn push_filter(sql: &mut String, args: &mut Vec<String>, col: &str, v: &Option<String>) {
    if let Some(val) = v {
        sql.push_str(&format!(" AND {} = ?", col));
        args.push(val.clone());
    }
}

fn row_to_log(row: &rusqlite::Row) -> rusqlite::Result<MecAuditLog> {
    let timestamp: String = row.get(1)?;
    let ts = DateTime::parse_from_rfc3339(&timestamp)
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let status: String = row.get(6)?;
    let ops_str: Option<String> = row.get(11)?;
    let input_str: Option<String> = row.get(8)?;
    let output_str: Option<String> = row.get(9)?;
    Ok(MecAuditLog {
        id: row.get(0)?,
        timestamp: ts,
        user: row.get(2)?,
        action: row.get(3)?,
        resource_type: row.get(4)?,
        resource_id: row.get(5)?,
        status: AuditStatus::from_str(&status).unwrap_or(AuditStatus::Failed),
        duration_ms: row.get::<_, i64>(7)? as u64,
        input: input_str.and_then(|s| serde_json::from_str(&s).ok()),
        output: output_str.and_then(|s| serde_json::from_str(&s).ok()),
        error: row.get(10)?,
        operations: ops_str
            .and_then(|s| serde_json::from_str::<Vec<OperationLog>>(&s).ok())
            .unwrap_or_default(),
        source_ip: row.get(12)?,
        user_agent: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mec::db;

    fn sample_log(id: &str) -> MecAuditLog {
        MecAuditLog {
            id: id.into(),
            timestamp: Utc::now(),
            user: "admin".into(),
            action: "tenant_create".into(),
            resource_type: "tenant".into(),
            resource_id: "ygram-poc".into(),
            status: AuditStatus::Success,
            duration_ms: 120,
            input: Some(serde_json::json!({"foo": 1})),
            output: None,
            error: None,
            operations: vec![],
            source_ip: Some("127.0.0.1".into()),
            user_agent: Some("curl".into()),
        }
    }

    #[test]
    fn roundtrip() {
        let db = db::open_in_memory().unwrap();
        let store = AuditStore::new(db);
        let log = sample_log("a1");
        store.insert(&log).unwrap();
        let got = store.get("a1").unwrap().unwrap();
        assert_eq!(got.user, "admin");
        assert_eq!(got.status, AuditStatus::Success);
        assert_eq!(got.duration_ms, 120);
    }

    #[test]
    fn query_filters() {
        let db = db::open_in_memory().unwrap();
        let store = AuditStore::new(db);
        for i in 0..3 {
            store.insert(&sample_log(&format!("a{}", i))).unwrap();
        }
        let results = store.query(&AuditQuery {
            user: Some("admin".into()),
            limit: Some(10),
            ..Default::default()
        }).unwrap();
        assert_eq!(results.len(), 3);

        let none = store.query(&AuditQuery {
            user: Some("other".into()),
            ..Default::default()
        }).unwrap();
        assert!(none.is_empty());
    }
}
