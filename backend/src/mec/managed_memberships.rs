//! Explicit ownership-to-policy associations and conservative automatic revocation.
//! Grants are created only with durable policy intents; existing grants require explicit association.
#[path = "managed_membership_inventory.rs"]
mod inventory_api;
pub use inventory_api::{inventory, pending_inventory};
use inventory_api::{no_store, pending};

#[path = "managed_membership_association.rs"]
mod association;
#[cfg(test)]
use association::associate;
pub use association::attach;

#[path = "managed_membership_grant.rs"]
mod granting;
pub use granting::grant;

use super::{
    db::{audit_store::AuditStore, identity_store, membership_store, DbHandle},
    services::{rancher_service::ProjectRoleBinding, ServiceError, ServiceResult},
    MecState,
};
use crate::{
    jwt_sessions::SessionStore,
    models::{
        mec::{AuditStatus, MecAuditLog, TenantStatus},
        NabimanUser, ScopeKind, UserRole,
    },
    users::UserStore,
};
use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize)]
pub struct ManagedBinding {
    pub operation_id: String,
    pub tenant_id: String,
    pub realm: String,
    pub project_id: String,
    pub remote_user_id: String,
    pub role_template: String,
    pub binding_id: String,
    pub ownership_state: String,
    pub ownership_updated_at: String,
    pub local_user_id: String,
    pub source_role: String,
    pub attached_by: String,
    pub attached_at: String,
}
const SELECT: &str = "SELECT o.operation_id,o.tenant_id,o.realm,o.project_id,o.remote_user_id,o.role_template,o.binding_id,o.state,m.local_user_id,m.source_role,m.attached_by,m.attached_at,o.updated_at FROM managed_project_bindings m JOIN owned_project_bindings o ON o.operation_id=m.operation_id";
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ManagedBinding> {
    Ok(ManagedBinding {
        operation_id: r.get(0)?,
        tenant_id: r.get(1)?,
        realm: r.get(2)?,
        project_id: r.get(3)?,
        remote_user_id: r.get(4)?,
        role_template: r.get(5)?,
        binding_id: r.get(6)?,
        ownership_state: r.get(7)?,
        local_user_id: r.get(8)?,
        source_role: r.get(9)?,
        attached_by: r.get(10)?,
        attached_at: r.get(11)?,
        ownership_updated_at: r.get(12)?,
    })
}
pub fn list(db: &DbHandle, tenant: Option<&str>) -> ServiceResult<Vec<ManagedBinding>> {
    list_on(&db.lock().unwrap(), tenant)
}
pub(super) fn list_on(
    conn: &rusqlite::Connection,
    tenant: Option<&str>,
) -> ServiceResult<Vec<ManagedBinding>> {
    let mut statement = conn.prepare(&format!(
        "{SELECT} WHERE (?1 IS NULL OR o.tenant_id=?1) ORDER BY m.attached_at"
    ))?;
    let records = statement
        .query_map(params![tenant], row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(records)
}
pub(super) fn source_role(user: &NabimanUser, tenant: &str, remote_role: &str) -> Option<String> {
    if !user.active() {
        return None;
    }
    let membership = user
        .memberships
        .iter()
        .find(|m| m.kind == ScopeKind::Tenant && m.scope_id == tenant && m.accessible())?;
    let permitted = match membership.role {
        UserRole::Viewer => remote_role == "read-only",
        UserRole::Operator => ["read-only", "project-member"].contains(&remote_role),
        UserRole::Admin => ["read-only", "project-member", "project-owner"].contains(&remote_role),
        UserRole::Remote => false,
    };
    permitted.then(|| membership.role.as_str().into())
}
fn denied_policy(
    state: &MecState,
    users: &UserStore,
    tenant: &str,
    local: &str,
    role: &str,
    remote_role: &str,
) -> ServiceResult<bool> {
    let source_allowed = users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == local)
        .is_some_and(|u| source_role(u, tenant, remote_role).as_deref() == Some(role));
    let inactive = state
        .tenants
        .get(tenant)?
        .is_some_and(|t| t.status != TenantStatus::Active);
    Ok(!source_allowed || inactive)
}
/// Confirm only durable candidate identities for already-approved cleanup.
/// A username/role match without a checkpointed ID is never ownership evidence.
async fn recover_denied_candidates(
    state: &MecState,
    users: &UserStore,
    cursor: usize,
) -> ServiceResult<()> {
    let mut requests = pending(&state.db, None)?;
    if !requests.is_empty() {
        let count = requests.len();
        requests.rotate_left(cursor % count);
    }
    for request in requests.into_iter().take(100) {
        let op = request.operation;
        let Some(id) = op
            .binding_id
            .as_deref()
            .filter(|id| membership_store::valid_binding_id(id))
        else {
            continue;
        };
        if !denied_policy(
            state,
            users,
            &op.tenant_id,
            &request.local_user_id,
            &request.source_role,
            &op.role_template,
        )? {
            continue;
        }
        if state.services.rancher.identity_realm().ok().as_deref() != Some(op.realm.as_str())
            || !state
                .tenants
                .get(&op.tenant_id)?
                .is_some_and(|t| t.rancher_project_id.as_deref() == Some(op.project_id.as_str()))
        {
            continue;
        }
        let inventory = match state.services.rancher.list_prtb(&op.project_id).await {
            Ok(rows) => rows,
            Err(_) => continue,
        };
        if inventory.iter().any(|b| b.project_id != op.project_id)
            || !inventory.iter().any(|b| {
                b.id == id && b.user_id == op.remote_user_id && b.role_template == op.role_template
            })
        {
            continue;
        }
        if !denied_policy(
            state,
            users,
            &op.tenant_id,
            &request.local_user_id,
            &request.source_role,
            &op.role_template,
        )? || !state
            .tenants
            .get(&op.tenant_id)?
            .is_some_and(|t| t.rancher_project_id.as_deref() == Some(op.project_id.as_str()))
        {
            continue;
        }
        let observed_at = chrono::Utc::now().to_rfc3339();
        // Snapshot comparison and policy/ownership/audit share the final DB txn.
        // Audit failure keeps the request unresolved; it never authorizes a POST replay.
        if membership_store::reconcile_complete(&state.db,&op,membership_store::Reconciliation{actor:"system:managed-candidate-confirmation",reason:"Confirm durable candidate for explicitly approved revocation after source policy loss",observed_at:&observed_at,source_ip:None,user_agent:None}).is_err() { continue; }
    }
    Ok(())
}
fn unresolved_revocation(
    state: &MecState,
    grant: &ManagedBinding,
) -> ServiceResult<Option<membership_store::Operation>> {
    membership_store::unresolved_system_revocation(
        &state.db,
        &grant.tenant_id,
        &grant.realm,
        &grant.project_id,
        &grant.remote_user_id,
        &grant.role_template,
        &grant.binding_id,
    )
}
fn recover_revocation_absence(
    state: &MecState,
    op: &membership_store::Operation,
) -> ServiceResult<()> {
    let observed_at = chrono::Utc::now().to_rfc3339();
    membership_store::reconcile_complete(&state.db,op,membership_store::Reconciliation{actor:"system:membership-revocation-confirmation",reason:"Read-only confirmation of absence after approved automatic revocation; no deletion replay",observed_at:&observed_at,source_ip:None,user_agent:None})
}
fn revoked_source(users: &UserStore, grant: &ManagedBinding) -> bool {
    !users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == grant.local_user_id)
        .is_some_and(|u| {
            source_role(u, &grant.tenant_id, &grant.role_template).as_deref()
                == Some(grant.source_role.as_str())
        })
}
/// One sequential task per owned DB. Unknown outcomes block replay through the
/// shared durable-intent constraint. Grants are never automatically recreated.
pub fn start(state: web::Data<MecState>, users: UserStore) {
    actix_web::rt::spawn(async move {
        let mut cursor = 0usize;
        loop {
            if let Err(_) = sweep_page(&state, &users, cursor).await {
                eprintln!("Managed membership scan could not complete; no unverified outcome is reported as revoked");
            }
            cursor = cursor.wrapping_add(100);
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}
#[cfg(test)]
pub async fn sweep(state: &MecState, users: &UserStore) -> ServiceResult<usize> {
    sweep_page(state, users, 0).await
}
async fn sweep_page(state: &MecState, users: &UserStore, cursor: usize) -> ServiceResult<usize> {
    recover_denied_candidates(state, users, cursor).await?;
    let mut records = list(&state.db, None)?;
    if !records.is_empty() {
        let count = records.len();
        records.rotate_left(cursor % count);
    }
    let mut completed = 0;
    for grant in records
        .into_iter()
        .take(100)
        .filter(|g| g.ownership_state == "active")
    {
        let tenant = state.tenants.get(&grant.tenant_id)?;
        let needs_revocation = revoked_source(users, &grant)
            || tenant
                .as_ref()
                .is_some_and(|t| t.status != TenantStatus::Active);
        let pending_revocation = unresolved_revocation(state, &grant)?;
        if !needs_revocation && pending_revocation.is_none() {
            continue;
        }
        if state.services.rancher.identity_realm().ok().as_deref() != Some(grant.realm.as_str())
            || !tenant
                .is_some_and(|t| t.rancher_project_id.as_deref() == Some(grant.project_id.as_str()))
        {
            continue;
        }
        let bindings = match state.services.rancher.list_prtb(&grant.project_id).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        if bindings.iter().any(|b| b.project_id != grant.project_id) {
            continue;
        }
        let matching = bindings.iter().find(|b| b.id == grant.binding_id);
        if matching.is_some_and(|b| {
            b.user_id != grant.remote_user_id || b.role_template != grant.role_template
        }) {
            continue;
        }
        if matching.is_none() {
            if let Some(op) = pending_revocation.as_ref() {
                if state.tenants.get(&grant.tenant_id)?.is_some_and(|t| {
                    t.rancher_project_id.as_deref() == Some(grant.project_id.as_str())
                }) {
                    recover_revocation_absence(state, op)?;
                    completed += 1;
                }
                continue;
            }
        }
        // Restoration before dispatch cancels this attempt; restoration afterward
        // does not recreate remote privileges. An administrator must review them.
        let current_tenant = state.tenants.get(&grant.tenant_id)?;
        if !revoked_source(users, &grant)
            && current_tenant
                .as_ref()
                .is_some_and(|t| t.status == TenantStatus::Active)
        {
            continue;
        }
        if !current_tenant
            .is_some_and(|t| t.rancher_project_id.as_deref() == Some(grant.project_id.as_str()))
        {
            continue;
        }
        let binding = ProjectRoleBinding {
            id: grant.binding_id.clone(),
            project_id: grant.project_id.clone(),
            user_id: grant.remote_user_id.clone(),
            role_template: grant.role_template.clone(),
        };
        let op = match membership_store::begin(
            &state.db,
            &grant.tenant_id,
            &grant.realm,
            &binding,
            "delete",
            "system:membership-revocation",
            None,
            None,
        ) {
            Ok(op) => op,
            Err(_) => continue,
        };
        // Inventory absence confirms an already absent direct binding; never
        // interpret an upstream error or different identity as absence.
        if matching.is_some()
            && state
                .services
                .rancher
                .delete_prtb(&grant.binding_id)
                .await
                .is_err()
        {
            membership_store::unknown(
                &state.db,
                &op,
                "Automatic revocation provider outcome was not confirmed",
            )?;
            continue;
        }
        let confirmed = match state.services.rancher.list_prtb(&grant.project_id).await {
            Ok(b) => {
                !b.iter().any(|b| b.id == grant.binding_id)
                    && b.iter().all(|b| b.project_id == grant.project_id)
            }
            Err(_) => false,
        };
        if !confirmed {
            membership_store::unknown(
                &state.db,
                &op,
                "Automatic revocation absence could not be verified",
            )?;
            continue;
        }
        if membership_store::complete(&state.db, &op).is_err() {
            membership_store::unknown(
                &state.db,
                &op,
                "Automatic revocation final checkpoint could not commit",
            )?;
            continue;
        }
        completed += 1;
    }
    Ok(completed)
}

#[cfg(test)]
#[path = "managed_membership_tests.rs"]
mod tests;
