use super::*;

pub fn save(
    db: &DbHandle,
    tenant: &str,
    body: &UpdateRequest,
    actor: &str,
    source_ip: Option<String>,
    user_agent: Option<String>,
) -> ServiceResult<Delegation> {
    let now = chrono::Utc::now();
    let reason = body.reason.trim();
    if body.confirm_tenant_id != tenant
        || reason.len() < 3
        || reason.len() > 1024
        || reason.chars().any(char::is_control)
        || !body.acknowledge_project_role_scope
        || body.manager_ids.len() > 32
        || body.allowed_roles.len() > 2
        || body.allowed_actions.len() > 2
        || body
            .allowed_actions
            .iter()
            .any(|a| !["grant", "revoke"].contains(&a.as_str()))
        || body
            .manager_ids
            .iter()
            .any(|id| id.is_empty() || id.len() > 128)
        || body
            .allowed_roles
            .iter()
            .any(|r| !["read-only", "project-member"].contains(&r.as_str()))
    {
        return Err(ServiceError::InvalidInput(
            "Confirm the tenant, role scope and bounded delegation settings".into(),
        ));
    }
    let mut managers = body.manager_ids.clone();
    managers.sort();
    managers.dedup();
    let mut roles = body.allowed_roles.clone();
    roles.sort();
    roles.dedup();
    let mut actions = body.allowed_actions.clone();
    actions.sort();
    actions.dedup();
    if managers.len() != body.manager_ids.len()
        || roles.len() != body.allowed_roles.len()
        || actions.len() != body.allowed_actions.len()
    {
        return Err(ServiceError::InvalidInput(
            "Duplicate delegation managers or roles".into(),
        ));
    }
    if body.enabled
        && (managers.is_empty()
            || roles.is_empty()
            || actions.is_empty()
            || !body
                .expires_at
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .is_some_and(|t| t > now && t <= now + chrono::Duration::days(30)))
    {
        return Err(ServiceError::InvalidInput(
            "Enabled delegation requires managers, roles and expiry within 30 days".into(),
        ));
    }
    let mut conn = db.lock().unwrap();
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let current = read_on(&tx, tenant)?;
    if current.as_ref().map(|p| p.version.as_str()) != body.expected_version.as_deref() {
        return Err(ServiceError::Conflict("Delegation version changed".into()));
    }
    let profile: Option<(Option<String>, String)> = tx
        .query_row(
            "SELECT rancher_project_id,status FROM tenant_profiles WHERE id=?1",
            [tenant],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if !profile.is_some_and(|(p, s)| {
        p.as_deref() == Some(body.expected_project_id.as_str())
            && (!body.enabled
                || serde_json::from_str::<TenantStatus>(&s).ok() == Some(TenantStatus::Active))
    }) {
        return Err(ServiceError::Conflict(
            "Tenant project or lifecycle changed".into(),
        ));
    }
    let policy = Delegation {
        tenant_id: tenant.into(),
        version: uuid::Uuid::new_v4().to_string(),
        realm: body.expected_realm.clone(),
        project_id: body.expected_project_id.clone(),
        enabled: body.enabled,
        manager_ids: managers,
        allowed_roles: roles,
        allowed_actions: actions,
        expires_at: body.expires_at.clone(),
        updated_by: actor.into(),
        updated_at: now.to_rfc3339(),
        reason: reason.into(),
    };
    tx.execute("INSERT INTO tenant_access_delegations(tenant_id,version,realm,project_id,enabled,manager_ids,allowed_roles,expires_at,updated_by,updated_at,reason) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(tenant_id) DO UPDATE SET version=excluded.version,realm=excluded.realm,project_id=excluded.project_id,enabled=excluded.enabled,manager_ids=excluded.manager_ids,allowed_roles=excluded.allowed_roles,expires_at=excluded.expires_at,updated_by=excluded.updated_by,updated_at=excluded.updated_at,reason=excluded.reason",params![tenant,policy.version,policy.realm,policy.project_id,policy.enabled,serde_json::to_string(&policy.manager_ids).unwrap(),serde_json::to_string(&policy.allowed_roles).unwrap(),policy.expires_at,actor,policy.updated_at,reason])?;
    tx.execute("INSERT INTO tenant_access_delegation_actions(tenant_id,version,actions) VALUES (?1,?2,?3) ON CONFLICT(tenant_id) DO UPDATE SET version=excluded.version,actions=excluded.actions",params![tenant,policy.version,serde_json::to_string(&policy.allowed_actions)?])?;
    let log = MecAuditLog {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: now,
        user: actor.into(),
        action: "tenant_access_delegation_updated".into(),
        resource_type: "tenant".into(),
        resource_id: tenant.into(),
        status: AuditStatus::Success,
        duration_ms: 0,
        input: Some(
            serde_json::json!({"policy":policy,"previous_version":body.expected_version,"affects_future_grants_only":true}),
        ),
        output: None,
        error: None,
        operations: vec![],
        source_ip,
        user_agent,
    };
    AuditStore::insert_on(&tx, &log)?;
    tx.commit()?;
    Ok(policy)
}
