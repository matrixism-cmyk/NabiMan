use super::*;

pub(super) fn begin_with_policy(
    db: &DbHandle,
    tenant: &str,
    realm: &str,
    binding: &ProjectRoleBinding,
    kind: &str,
    actor: &str,
    source_ip: Option<String>,
    user_agent: Option<String>,
    policy: Option<ManagedIntent<'_>>,
    revocation: Option<RevocationIntent<'_>>,
) -> ServiceResult<Operation> {
    if !["create", "delete"].contains(&kind) {
        return Err(ServiceError::InvalidInput(
            "Invalid membership operation".into(),
        ));
    }
    let now = chrono::Utc::now().to_rfc3339();
    let op = Operation {
        id: format!("membership-{}", uuid::Uuid::new_v4().simple()),
        operation: kind.into(),
        tenant_id: tenant.into(),
        realm: realm.into(),
        project_id: binding.project_id.clone(),
        remote_user_id: binding.user_id.clone(),
        role_template: binding.role_template.clone(),
        binding_id: if kind == "delete" {
            Some(binding.id.clone())
        } else {
            None
        },
        state: "applying".into(),
        error: None,
        actor: actor.into(),
        created_at: now.clone(),
        updated_at: now,
        source_ip,
        user_agent,
    };
    let mut conn = db.lock().unwrap();
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let unresolved:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM membership_operations WHERE realm=?1 AND project_id=?2 AND remote_user_id=?3 AND state!='completed')",params![realm,binding.project_id,binding.user_id],|r|r.get(0))?;
    if unresolved {
        return Err(ServiceError::Conflict(
            "An unresolved membership operation requires reconciliation before retry".into(),
        ));
    }
    if kind == "create" {
        let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM owned_project_bindings WHERE realm=?1 AND project_id=?2 AND remote_user_id=?3 AND state='active')",params![realm,binding.project_id,binding.user_id],|r|r.get(0))?;
        if active {
            return Err(ServiceError::Conflict(
                "A recorded active binding requires reconciliation before replacement".into(),
            ));
        }
    } else {
        let recorded:Option<(String,String,String,String)>=tx.query_row("SELECT project_id,remote_user_id,role_template,state FROM owned_project_bindings WHERE realm=?1 AND binding_id=?2",params![realm,binding.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        if recorded.is_some_and(|r| {
            r.0 != binding.project_id
                || r.1 != binding.user_id
                || r.2 != binding.role_template
                || r.3 != "active"
        }) {
            return Err(ServiceError::Conflict(
                "Provider binding identity conflicts with recorded ownership".into(),
            ));
        }
    }
    tx.execute("INSERT INTO membership_operations(id,operation,tenant_id,realm,project_id,remote_user_id,role_template,binding_id,state,error,actor,created_at,updated_at,source_ip,user_agent) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,NULL,?10,?11,?11,?12,?13)",params![op.id,op.operation,op.tenant_id,op.realm,op.project_id,op.remote_user_id,op.role_template,op.binding_id,op.state,op.actor,op.created_at,op.source_ip,op.user_agent])?;
    AuditStore::insert_on(&tx, &audit(&op, "intent", AuditStatus::Partial))?;
    if let Some(policy) = policy {
        let linked:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM external_identity_links WHERE local_user_id=?1 AND realm=?2 AND remote_user_id=?3)",params![policy.local_user_id,realm,binding.user_id],|r|r.get(0))?;
        let profile: Option<(Option<String>, String)> = tx
            .query_row(
                "SELECT rancher_project_id,status FROM tenant_profiles WHERE id=?1",
                params![tenant],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if !linked
            || !profile.is_some_and(|(project, status)| {
                project.as_deref() == Some(binding.project_id.as_str())
                    && serde_json::from_str::<crate::models::mec::TenantStatus>(&status).ok()
                        == Some(crate::models::mec::TenantStatus::Active)
            })
        {
            return Err(ServiceError::Conflict(
                "Managed identity or active tenant project changed before dispatch".into(),
            ));
        }
        if let Some(delegated) = policy.delegation.as_ref() {
            let approval = crate::mec::access_delegation::read_on(&tx, tenant)?;
            if !approval.is_some_and(|p| {
                p.version == delegated.version
                    && p.permits(
                        delegated.manager_id,
                        realm,
                        &binding.project_id,
                        &binding.role_template,
                    )
            }) {
                return Err(ServiceError::Conflict(
                    "Delegation changed before durable dispatch".into(),
                ));
            }
            tx.execute("INSERT INTO requested_delegated_grants(operation_id,manager_id,delegation_version) VALUES (?1,?2,?3)",params![op.id,delegated.manager_id,delegated.version])?;
        }
        tx.execute("INSERT INTO requested_managed_grants(operation_id,local_user_id,source_role,reason) VALUES (?1,?2,?3,?4)",params![op.id,policy.local_user_id,policy.source_role,policy.reason])?;
        let mut log = audit(&op, "managed_request", AuditStatus::Partial);
        log.input = Some(
            serde_json::json!({"operation_id":op.id,"local_user_id":policy.local_user_id,"source_role":policy.source_role,"remote_role":binding.role_template,"reason":policy.reason,"automatic_revocation":true,"delegation":policy.delegation.as_ref().map(|d|serde_json::json!({"manager_id":d.manager_id,"version":d.version}))}),
        );
        AuditStore::insert_on(&tx, &log)?;
    }
    if let Some(review) = revocation {
        let exact:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM owned_project_bindings o JOIN managed_project_bindings m ON m.operation_id=o.operation_id JOIN tenant_profiles t ON t.id=o.tenant_id WHERE o.operation_id=?1 AND o.tenant_id=?2 AND o.realm=?3 AND o.project_id=?4 AND o.remote_user_id=?5 AND o.role_template=?6 AND o.binding_id=?7 AND o.state='active' AND o.updated_at=?8 AND m.local_user_id=?9 AND t.rancher_project_id=o.project_id)",params![review.grant_operation_id,tenant,realm,binding.project_id,binding.user_id,binding.role_template,binding.id,review.expected_updated_at,review.local_user_id],|r|r.get(0))?;
        if !exact {
            return Err(ServiceError::Conflict(
                "Managed ownership, source identity or project changed before revocation".into(),
            ));
        }
        if let Some(version) = review.delegation_version {
            let approval = crate::mec::access_delegation::read_on(&tx, tenant)?;
            if !approval.is_some_and(|p| {
                p.version == version
                    && p.permits_action(
                        review.manager_id,
                        realm,
                        &binding.project_id,
                        &binding.role_template,
                        "revoke",
                    )
            }) {
                return Err(ServiceError::Conflict(
                    "Revocation delegation changed before durable dispatch".into(),
                ));
            }
        }
        tx.execute("INSERT INTO delegated_membership_revocations(operation_id,grant_operation_id,local_user_id,manager_id,delegation_version,reason) VALUES (?1,?2,?3,?4,?5,?6)",params![op.id,review.grant_operation_id,review.local_user_id,review.manager_id,review.delegation_version,review.reason])?;
        let mut log = audit(&op, "managed_revoke_request", AuditStatus::Partial);
        log.input = Some(
            serde_json::json!({"operation_id":op.id,"grant_operation_id":review.grant_operation_id,"local_user_id":review.local_user_id,"binding_id":binding.id,"manager_id":review.manager_id,"delegation_version":review.delegation_version,"reason":review.reason,"scope":"owned_direct_project_binding_only"}),
        );
        AuditStore::insert_on(&tx, &log)?;
    }
    tx.commit()?;
    Ok(op)
}
