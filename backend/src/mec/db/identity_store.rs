//! Explicit, immutable identity links. Linking records identity, never a role.
use super::{audit_store::AuditStore, DbHandle};
use crate::mec::services::{ServiceError, ServiceResult};
use crate::models::mec::MecAuditLog;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct IdentityLink {
    pub local_user_id: String,
    pub realm: String,
    pub remote_user_id: String,
    pub remote_username: String,
    pub verified_at: String,
    pub linked_by: String,
    pub reason: String,
}

pub fn get(db: &DbHandle, user: &str, realm: &str) -> ServiceResult<Option<IdentityLink>> {
    let conn = db.lock().unwrap();
    Ok(conn.query_row("SELECT local_user_id,realm,remote_user_id,remote_username,verified_at,linked_by,reason FROM external_identity_links WHERE local_user_id=?1 AND realm=?2", params![user,realm], |r| Ok(IdentityLink {
        local_user_id:r.get(0)?, realm:r.get(1)?, remote_user_id:r.get(2)?, remote_username:r.get(3)?, verified_at:r.get(4)?, linked_by:r.get(5)?, reason:r.get(6)?,
    })).optional()?)
}

pub fn insert(db: &DbHandle, link: &IdentityLink, audit: &MecAuditLog) -> ServiceResult<()> {
    let mut conn = db.lock().unwrap();
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM external_identity_links WHERE realm=?1 AND (local_user_id=?2 OR remote_user_id=?3))", params![link.realm,link.local_user_id,link.remote_user_id], |r| r.get(0))?;
    if exists {
        return Err(ServiceError::Conflict("An identity is already linked in this Rancher realm; replacement requires access reconciliation".into()));
    }
    tx.execute("INSERT INTO external_identity_links (local_user_id,realm,remote_user_id,remote_username,verified_at,linked_by,reason) VALUES (?1,?2,?3,?4,?5,?6,?7)", params![link.local_user_id,link.realm,link.remote_user_id,link.remote_username,link.verified_at,link.linked_by,link.reason])?;
    AuditStore::insert_on(&tx, audit)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::mec::AuditStatus;
    fn fixture(user: &str, realm: &str, remote: &str) -> (IdentityLink, MecAuditLog) {
        let now = chrono::Utc::now();
        (
            IdentityLink {
                local_user_id: user.into(),
                realm: realm.into(),
                remote_user_id: remote.into(),
                remote_username: "verified-name".into(),
                verified_at: now.to_rfc3339(),
                linked_by: "admin".into(),
                reason: "Verified owner".into(),
            },
            MecAuditLog {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: now,
                user: "admin".into(),
                action: "external_identity_link".into(),
                resource_type: "member".into(),
                resource_id: user.into(),
                status: AuditStatus::Success,
                duration_ms: 0,
                input: None,
                output: None,
                error: None,
                operations: vec![],
                source_ip: None,
                user_agent: None,
            },
        )
    }
    #[test]
    fn links_survive_restart_and_remain_isolated_by_provider_realm() {
        let dir = std::env::temp_dir().join(format!("nabiman-identity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("identities.db");
        let db = crate::mec::db::open(path.clone()).unwrap();
        let (link, audit) = fixture("local", "https://one.example", "u-1");
        insert(&db, &link, &audit).unwrap();
        let (other, audit) = fixture("local", "https://two.example", "u-1");
        insert(&db, &other, &audit).unwrap();
        drop(db);
        let db = crate::mec::db::open(path).unwrap();
        assert_eq!(
            get(&db, "local", "https://one.example").unwrap().unwrap(),
            link
        );
        assert_eq!(
            get(&db, "local", "https://two.example").unwrap().unwrap(),
            other
        );
        assert!(get(&db, "different-local", "https://one.example")
            .unwrap()
            .is_none());
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn concurrent_links_have_one_owner_and_one_audit_event() {
        let db = crate::mec::db::open_in_memory().unwrap();
        let workers: Vec<_> = (0..8)
            .map(|i| {
                let db = db.clone();
                std::thread::spawn(move || {
                    let (link, audit) =
                        fixture(&format!("local-{i}"), "https://one.example", "u-1");
                    insert(&db, &link, &audit)
                })
            })
            .collect();
        let results: Vec<_> = workers.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|r| matches!(r, Err(ServiceError::Conflict(_))))
                .count(),
            7
        );
        let conn = db.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM external_identity_links", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_logs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
