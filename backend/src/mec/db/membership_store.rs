//! Durable intents and exact provider binding ownership; no automatic replay.
#[path = "membership_intent.rs"]
mod intent;
use intent::begin_with_policy;

use super::{audit_store::AuditStore, DbHandle};
use crate::mec::services::{rancher_service::ProjectRoleBinding, ServiceError, ServiceResult};
use crate::models::mec::{AuditStatus, MecAuditLog};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Operation {
    pub id: String,
    pub operation: String,
    pub tenant_id: String,
    pub realm: String,
    pub project_id: String,
    pub remote_user_id: String,
    pub role_template: String,
    pub binding_id: Option<String>,
    pub state: String,
    pub error: Option<String>,
    pub actor: String,
    pub created_at: String,
    pub updated_at: String,
    pub source_ip: Option<String>,
    pub user_agent: Option<String>,
}
const SELECT: &str = "SELECT id,operation,tenant_id,realm,project_id,remote_user_id,role_template,binding_id,state,error,actor,created_at,updated_at,source_ip,user_agent FROM membership_operations";
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Operation> {
    Ok(Operation {
        id: r.get(0)?,
        operation: r.get(1)?,
        tenant_id: r.get(2)?,
        realm: r.get(3)?,
        project_id: r.get(4)?,
        remote_user_id: r.get(5)?,
        role_template: r.get(6)?,
        binding_id: r.get(7)?,
        state: r.get(8)?,
        error: r.get(9)?,
        actor: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
        source_ip: r.get(13)?,
        user_agent: r.get(14)?,
    })
}
fn audit(op: &Operation, phase: &str, status: AuditStatus) -> MecAuditLog {
    MecAuditLog {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now(),
        user: op.actor.clone(),
        action: format!("tenant_member_{}_{}", op.operation, phase),
        resource_type: "tenant".into(),
        resource_id: op.tenant_id.clone(),
        status,
        duration_ms: 0,
        input: Some(
            serde_json::json!({"operation_id":op.id,"realm":op.realm,"project_id":op.project_id,"remote_user_id":op.remote_user_id,"role_template":op.role_template,"binding_id":op.binding_id}),
        ),
        output: None,
        error: op.error.clone(),
        operations: vec![],
        source_ip: op.source_ip.clone(),
        user_agent: op.user_agent.clone(),
    }
}
pub fn begin(
    db: &DbHandle,
    tenant: &str,
    realm: &str,
    binding: &ProjectRoleBinding,
    kind: &str,
    actor: &str,
    source_ip: Option<String>,
    user_agent: Option<String>,
) -> ServiceResult<Operation> {
    begin_with_policy(
        db, tenant, realm, binding, kind, actor, source_ip, user_agent, None, None,
    )
}
pub struct DelegatedIntent<'a> {
    pub manager_id: &'a str,
    pub version: &'a str,
}
pub struct ManagedIntent<'a> {
    pub delegation: Option<DelegatedIntent<'a>>,
    pub local_user_id: &'a str,
    pub source_role: &'a str,
    pub reason: &'a str,
}
pub fn begin_managed(
    db: &DbHandle,
    tenant: &str,
    realm: &str,
    binding: &ProjectRoleBinding,
    actor: &str,
    source_ip: Option<String>,
    user_agent: Option<String>,
    policy: ManagedIntent<'_>,
) -> ServiceResult<Operation> {
    begin_with_policy(
        db,
        tenant,
        realm,
        binding,
        "create",
        actor,
        source_ip,
        user_agent,
        Some(policy),
        None,
    )
}
pub struct RevocationIntent<'a> {
    pub grant_operation_id: &'a str,
    pub local_user_id: &'a str,
    pub expected_updated_at: &'a str,
    pub manager_id: &'a str,
    pub delegation_version: Option<&'a str>,
    pub reason: &'a str,
}
pub fn begin_managed_revocation(
    db: &DbHandle,
    tenant: &str,
    realm: &str,
    binding: &ProjectRoleBinding,
    actor: &str,
    source_ip: Option<String>,
    user_agent: Option<String>,
    review: RevocationIntent<'_>,
) -> ServiceResult<Operation> {
    begin_with_policy(
        db,
        tenant,
        realm,
        binding,
        "delete",
        actor,
        source_ip,
        user_agent,
        None,
        Some(review),
    )
}
pub fn observe(db: &DbHandle, op: &Operation, binding_id: &str) -> ServiceResult<()> {
    if !valid_binding_id(binding_id) {
        return Err(ServiceError::InvalidInput(
            "Invalid observed binding identifier".into(),
        ));
    }
    let conn = db.lock().unwrap();
    let changed=conn.execute("UPDATE membership_operations SET binding_id=?1,updated_at=?2 WHERE id=?3 AND state='applying' AND (binding_id IS NULL OR binding_id=?1)",params![binding_id,chrono::Utc::now().to_rfc3339(),op.id])?;
    if changed != 1 {
        return Err(ServiceError::Conflict(
            "Membership operation is no longer applying".into(),
        ));
    }
    Ok(())
}
pub fn valid_binding_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b))
}
pub struct Reconciliation<'a> {
    pub actor: &'a str,
    pub reason: &'a str,
    pub observed_at: &'a str,
    pub source_ip: Option<String>,
    pub user_agent: Option<String>,
}
pub fn get(db: &DbHandle, tenant: &str, id: &str) -> ServiceResult<Operation> {
    db.lock()
        .unwrap()
        .query_row(
            &format!("{SELECT} WHERE tenant_id=?1 AND id=?2"),
            params![tenant, id],
            row,
        )
        .optional()?
        .ok_or_else(|| ServiceError::NotFound("membership operation".into()))
}
pub fn complete(db: &DbHandle, op: &Operation) -> ServiceResult<()> {
    finish(db, op, None)
}
pub fn reconcile_complete(
    db: &DbHandle,
    op: &Operation,
    review: Reconciliation<'_>,
) -> ServiceResult<()> {
    finish(db, op, Some(review))
}
fn finish(db: &DbHandle, op: &Operation, review: Option<Reconciliation<'_>>) -> ServiceResult<()> {
    let mut conn = db.lock().unwrap();
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let current = tx.query_row(&format!("{SELECT} WHERE id=?1"), [&op.id], row)?;
    if current.state
        != if review.is_some() {
            "unknown_result"
        } else {
            "applying"
        }
        || (review.is_some()
            && (current.updated_at != op.updated_at || current.binding_id != op.binding_id))
        || current.operation != op.operation
        || current.realm != op.realm
        || current.project_id != op.project_id
        || current.remote_user_id != op.remote_user_id
        || current.role_template != op.role_template
        || current.tenant_id != op.tenant_id
        || current.actor != op.actor
    {
        return Err(ServiceError::Conflict(
            "Membership operation identity or state changed".into(),
        ));
    }
    let binding = current
        .binding_id
        .as_deref()
        .filter(|id| valid_binding_id(id))
        .ok_or_else(|| ServiceError::Conflict("Binding identity was not checkpointed".into()))?;
    let now = chrono::Utc::now().to_rfc3339();
    if current.operation == "create" {
        tx.execute("INSERT INTO owned_project_bindings(operation_id,tenant_id,realm,project_id,remote_user_id,role_template,binding_id,state,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,'active',?8)",params![current.id,current.tenant_id,current.realm,current.project_id,current.remote_user_id,current.role_template,binding,now])?;
        let policy:Option<(String,String,String)>=tx.query_row("SELECT local_user_id,source_role,reason FROM requested_managed_grants WHERE operation_id=?1",params![current.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((local, role, reason)) = policy {
            tx.execute("INSERT INTO managed_project_bindings(operation_id,local_user_id,source_role,attached_by,attached_at,reason) VALUES (?1,?2,?3,?4,?5,?6)",params![current.id,local,role,current.actor,now,reason])?;
            let mut log = audit(&current, "managed_confirmed", AuditStatus::Success);
            log.input = Some(
                serde_json::json!({"operation_id":current.id,"local_user_id":local,"source_role":role,"binding_id":binding,"automatic_revocation":true}),
            );
            log.error = None;
            AuditStore::insert_on(&tx, &log)?;
        }
    } else {
        tx.execute("UPDATE owned_project_bindings SET state='revoked',updated_at=?1 WHERE realm=?2 AND binding_id=?3 AND project_id=?4 AND remote_user_id=?5 AND role_template=?6 AND state='active'",params![now,current.realm,binding,current.project_id,current.remote_user_id,current.role_template])?;
    }
    tx.execute("UPDATE membership_operations SET state='completed',error=NULL,updated_at=?1 WHERE id=?2 AND state=?3",params![now,current.id,current.state])?;
    let mut log = audit(
        &current,
        if review.is_some() {
            "reconciled"
        } else {
            "confirmed"
        },
        AuditStatus::Success,
    );
    log.error = None;
    if let Some(review) = review {
        log.user = review.actor.into();
        log.source_ip = review.source_ip;
        log.user_agent = review.user_agent;
        log.output = Some(
            serde_json::json!({"reason":review.reason,"observed_at":review.observed_at,"original_actor":current.actor,"prior_error":current.error,"evidence_scope":"direct_project_binding_only","provider_write_replayed":false}),
        );
    }
    AuditStore::insert_on(&tx, &log)?;
    tx.commit()?;
    Ok(())
}
pub fn unknown(db: &DbHandle, op: &Operation, reason: &str) -> ServiceResult<()> {
    let mut conn = db.lock().unwrap();
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let changed=tx.execute("UPDATE membership_operations SET state='unknown_result',error=?1,updated_at=?2 WHERE id=?3 AND state='applying'",params![reason,chrono::Utc::now().to_rfc3339(),op.id])?;
    if changed != 1 {
        return Err(ServiceError::Conflict(
            "Membership operation state changed".into(),
        ));
    }
    let record = tx.query_row(&format!("{SELECT} WHERE id=?1"), [&op.id], row)?;
    AuditStore::insert_on(&tx, &audit(&record, "unknown", AuditStatus::Partial))?;
    tx.commit()?;
    Ok(())
}
pub fn recover_interrupted(db: &DbHandle) -> rusqlite::Result<usize> {
    let mut conn = db.lock().unwrap();
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let records = {
        let mut statement = tx.prepare(&format!("{SELECT} WHERE state='applying'"))?;
        let result = statement
            .query_map([], row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };
    for mut record in records.clone() {
        record.error = Some("Server stopped before provider outcome was durably confirmed".into());
        tx.execute("UPDATE membership_operations SET state='unknown_result',error=?1,updated_at=?2 WHERE id=?3 AND state='applying'",params![record.error,chrono::Utc::now().to_rfc3339(),record.id])?;
        AuditStore::insert_on(&tx, &audit(&record, "interrupted", AuditStatus::Partial))?;
    }
    tx.commit()?;
    Ok(records.len())
}
pub fn list(db: &DbHandle, tenant: &str) -> ServiceResult<Vec<Operation>> {
    let conn = db.lock().unwrap();
    let mut statement = conn.prepare(&format!(
        "{SELECT} WHERE tenant_id=?1 ORDER BY created_at DESC LIMIT 100"
    ))?;
    let result = statement
        .query_map([tenant], row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(result)
}

#[cfg(test)]
#[path = "membership_store_tests.rs"]
mod tests;

pub fn unresolved_system_revocation(
    db: &DbHandle,
    tenant: &str,
    realm: &str,
    project: &str,
    user: &str,
    role: &str,
    binding: &str,
) -> ServiceResult<Option<Operation>> {
    let conn = db.lock().unwrap();
    Ok(conn.query_row(&format!("{SELECT} WHERE tenant_id=?1 AND realm=?2 AND project_id=?3 AND remote_user_id=?4 AND role_template=?5 AND binding_id=?6 AND operation='delete' AND state='unknown_result' AND state!='completed' AND actor='system:membership-revocation'"),params![tenant,realm,project,user,role,binding],row).optional()?)
}

pub fn latest_binding_change_on(
    conn: &rusqlite::Connection,
    tenant: &str,
    realm: &str,
    project: &str,
    user: &str,
    binding: &str,
) -> ServiceResult<Option<Operation>> {
    Ok(conn.query_row(&format!("{SELECT} WHERE tenant_id=?1 AND realm=?2 AND project_id=?3 AND remote_user_id=?4 AND (binding_id=?5 OR state!='completed') ORDER BY CASE WHEN state!='completed' THEN 0 ELSE 1 END,created_at DESC,id DESC LIMIT 1"),params![tenant,realm,project,user,binding],row).optional()?)
}
pub fn unresolved_member_change_on(
    conn: &rusqlite::Connection,
    realm: &str,
    project: &str,
    user: &str,
) -> ServiceResult<bool> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM membership_operations WHERE realm=?1 AND project_id=?2 AND remote_user_id=?3 AND state!='completed')",params![realm,project,user],|r|r.get(0))?)
}
