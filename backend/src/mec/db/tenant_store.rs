use super::DbHandle;
use crate::models::mec::{NodeAllocation, ResourceQuota, StarterKit, Tenant, TenantStatus};
use chrono::{DateTime, Utc};
use rusqlite::params;

pub struct TenantStore {
    db: DbHandle,
}

impl TenantStore {
    pub fn new(db: DbHandle) -> Self {
        Self { db }
    }

    /// Reserve a tenant ID without replacing an existing profile, including a
    /// failed/provisioning record that may own partially created resources.
    pub fn reserve(&self, t: &Tenant, created_by: &str) -> rusqlite::Result<bool> {
        let mut conn = self.db.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM tenant_profiles WHERE id=?1)",
            params![t.id],
            |r| r.get(0),
        )?;
        if exists {
            return Ok(false);
        }
        Self::upsert_on(&tx, t, created_by)?;
        tx.commit()?;
        Ok(true)
    }

    pub fn upsert(&self, t: &Tenant, created_by: &str) -> rusqlite::Result<()> {
        let conn = self.db.lock().unwrap();
        Self::upsert_on(&conn, t, created_by)
    }

    pub(super) fn upsert_on(
        conn: &rusqlite::Connection,
        t: &Tenant,
        created_by: &str,
    ) -> rusqlite::Result<()> {
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
                t.starter_kit
                    .as_ref()
                    .map(|s| serde_json::to_string(s).unwrap_or_default()),
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

    #[cfg(test)]
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
    let starter_kit: Option<StarterKit> = kit_str.and_then(|s| serde_json::from_str(&s).ok());
    let status: TenantStatus = serde_json::from_str(&status_str).unwrap_or(TenantStatus::Failed {
        reason: "unknown".into(),
    });

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

    #[test]
    fn concurrent_connections_reserve_only_one_owner_and_preserve_it() {
        let dir = std::env::temp_dir().join(format!("nabiman-reserve-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("state.db");
        let first = db::open_persistent(path.clone()).unwrap();
        let second = db::open_persistent(path.clone()).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = [first, second]
            .into_iter()
            .enumerate()
            .map(|(i, handle)| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let store = TenantStore::new(handle);
                    let mut tenant = sample_tenant("reserved");
                    let owner = format!("owner-{i}");
                    tenant.display_name = owner.clone();
                    tenant.status = TenantStatus::Provisioning;
                    barrier.wait();
                    (store.reserve(&tenant, &owner).unwrap(), owner)
                })
            })
            .collect();
        let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|(won, _)| *won).count(), 1);
        let winner = &results.iter().find(|(won, _)| *won).unwrap().1;
        let handle = db::open_persistent(path).unwrap();
        let store = TenantStore::new(handle.clone());
        assert_eq!(
            &store.get("reserved").unwrap().unwrap().display_name,
            winner
        );
        assert!(!store
            .reserve(&sample_tenant("reserved"), "replacement")
            .unwrap());
        let creator: String = handle
            .lock()
            .unwrap()
            .query_row(
                "SELECT created_by FROM tenant_profiles WHERE id='reserved'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(&creator, winner);
        drop(store);
        drop(handle);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
