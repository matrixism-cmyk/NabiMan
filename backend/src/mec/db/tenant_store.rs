use super::DbHandle;
use crate::models::mec::{
    NodeAllocation, ResourceQuota, StarterKit, Tenant, TenantStatus,
};
use chrono::{DateTime, Utc};
use rusqlite::params;

pub struct TenantStore {
    db: DbHandle,
}

impl TenantStore {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }

    pub fn upsert(&self, t: &Tenant, created_by: &str) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT INTO tenant_profiles
                (id, display_name, task_name, contact_email, namespace,
                 rancher_project_id, rancher_user_id, allocation_json,
                 quota_json, starter_kit_json, status,
                 created_at, created_by, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                task_name = excluded.task_name,
                contact_email = excluded.contact_email,
                rancher_project_id = excluded.rancher_project_id,
                rancher_user_id = excluded.rancher_user_id,
                allocation_json = excluded.allocation_json,
                quota_json = excluded.quota_json,
                starter_kit_json = excluded.starter_kit_json,
                status = excluded.status,
                updated_at = excluded.updated_at",
            params![
                t.id,
                t.display_name,
                t.task_name,
                t.contact_email,
                t.namespace,
                t.rancher_project_id,
                t.rancher_user_id,
                serde_json::to_string(&t.allocation).unwrap_or_default(),
                serde_json::to_string(&t.quota).unwrap_or_default(),
                t.starter_kit.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default()),
                serde_json::to_string(&t.status).unwrap_or_default(),
                t.created_at.to_rfc3339(),
                created_by,
                t.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> rusqlite::Result<Option<Tenant>> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, display_name, task_name, contact_email, namespace,
                    rancher_project_id, rancher_user_id, allocation_json,
                    quota_json, starter_kit_json, status, created_at, updated_at
             FROM tenant_profiles WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_tenant(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list(&self) -> rusqlite::Result<Vec<Tenant>> {
        let conn = self.db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, display_name, task_name, contact_email, namespace,
                    rancher_project_id, rancher_user_id, allocation_json,
                    quota_json, starter_kit_json, status, created_at, updated_at
             FROM tenant_profiles ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_tenant)?;
        rows.collect()
    }

    pub fn delete(&self, id: &str) -> rusqlite::Result<bool> {
        let conn = self.db.lock().unwrap();
        let n = conn.execute("DELETE FROM tenant_profiles WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    #[allow(dead_code)]
    pub fn set_status(&self, id: &str, status: &TenantStatus) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "UPDATE tenant_profiles SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                serde_json::to_string(status).unwrap_or_default(),
                Utc::now().to_rfc3339(),
                id,
            ],
        )?;
        Ok(())
    }
}

fn row_to_tenant(row: &rusqlite::Row) -> rusqlite::Result<Tenant> {
    let alloc_str: String = row.get(7)?;
    let quota_str: String = row.get(8)?;
    let kit_str: Option<String> = row.get(9)?;
    let status_str: String = row.get(10)?;
    let created: String = row.get(11)?;
    let updated: String = row.get(12)?;

    let allocation: NodeAllocation = serde_json::from_str(&alloc_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let quota: ResourceQuota = serde_json::from_str(&quota_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let starter_kit: Option<StarterKit> =
        kit_str.and_then(|s| serde_json::from_str(&s).ok());
    let status: TenantStatus = serde_json::from_str(&status_str)
        .unwrap_or(TenantStatus::Failed { reason: "unknown".into() });

    Ok(Tenant {
        id: row.get(0)?,
        display_name: row.get(1)?,
        task_name: row.get(2)?,
        contact_email: row.get(3)?,
        namespace: row.get(4)?,
        rancher_project_id: row.get(5)?,
        rancher_user_id: row.get(6)?,
        allocation,
        quota,
        starter_kit,
        created_at: parse_time(&created),
        updated_at: parse_time(&updated),
        status,
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
    use crate::models::mec::{AllocationType, NodeAllocation, ResourceQuota};

    fn sample_tenant(id: &str) -> Tenant {
        Tenant {
            id: id.into(),
            display_name: "와이그램".into(),
            task_name: None,
            contact_email: None,
            namespace: id.into(),
            rancher_project_id: None,
            rancher_user_id: None,
            allocation: NodeAllocation {
                alloc_type: AllocationType::Dedicated,
                node: Some("mec-wn04".into()),
                gpu_label: Some("gh200".into()),
                tier: Some("high".into()),
            },
            quota: ResourceQuota::standard(),
            starter_kit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: TenantStatus::Active,
        }
    }

    #[test]
    fn roundtrip() {
        let db = db::open_in_memory().unwrap();
        let store = TenantStore::new(db);
        let t = sample_tenant("ygram-poc");
        store.upsert(&t, "admin").unwrap();
        let got = store.get("ygram-poc").unwrap().unwrap();
        assert_eq!(got.display_name, "와이그램");
        assert_eq!(got.status, TenantStatus::Active);
    }

    #[test]
    fn list_order() {
        let db = db::open_in_memory().unwrap();
        let store = TenantStore::new(db);
        store.upsert(&sample_tenant("a"), "admin").unwrap();
        store.upsert(&sample_tenant("b"), "admin").unwrap();
        let all = store.list().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn delete_tenant() {
        let db = db::open_in_memory().unwrap();
        let store = TenantStore::new(db);
        store.upsert(&sample_tenant("x"), "admin").unwrap();
        assert!(store.delete("x").unwrap());
        assert!(store.get("x").unwrap().is_none());
    }
}
