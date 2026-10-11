use super::*;

pub(super) fn associate(
    db: &DbHandle,
    op: &membership_store::Operation,
    local: &str,
    role: &str,
    actor: &str,
    reason: &str,
    source_ip: Option<String>,
    user_agent: Option<String>,
) -> ServiceResult<()> {
    let mut conn = db.lock().unwrap();
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let owns:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM owned_project_bindings WHERE operation_id=?1 AND tenant_id=?2 AND realm=?3 AND project_id=?4 AND remote_user_id=?5 AND role_template=?6 AND binding_id=?7 AND state='active')",params![op.id,op.tenant_id,op.realm,op.project_id,op.remote_user_id,op.role_template,op.binding_id],|r|r.get(0))?;
    let linked:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM external_identity_links WHERE local_user_id=?1 AND realm=?2 AND remote_user_id=?3)",params![local,op.realm,op.remote_user_id],|r|r.get(0))?;
    let unresolved:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM membership_operations WHERE realm=?1 AND project_id=?2 AND remote_user_id=?3 AND state!='completed')",params![op.realm,op.project_id,op.remote_user_id],|r|r.get(0))?;
    if !owns || !linked || unresolved {
        return Err(ServiceError::Conflict(
            "Ownership, explicit identity or mutation state changed".into(),
        ));
    }
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM managed_project_bindings WHERE operation_id=?1)",
        params![op.id],
        |r| r.get(0),
    )?;
    if exists {
        return Err(ServiceError::Conflict(
            "The association is immutable; it cannot be reassigned".into(),
        ));
    }
    // Verify the tenant mapping in the same transaction as association.
    let tenant_snapshot: Option<(Option<String>, String)> = tx
        .query_row(
            "SELECT rancher_project_id,status FROM tenant_profiles WHERE id=?1",
            params![op.tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if !tenant_snapshot.is_some_and(|(project, status)| {
        project.as_deref() == Some(op.project_id.as_str())
            && serde_json::from_str::<TenantStatus>(&status).ok() == Some(TenantStatus::Active)
    }) {
        return Err(ServiceError::Conflict("Tenant project changed".into()));
    }
    let now = chrono::Utc::now();
    tx.execute("INSERT INTO managed_project_bindings(operation_id,local_user_id,source_role,attached_by,attached_at,reason) VALUES (?1,?2,?3,?4,?5,?6)",params![op.id,local,role,actor,now.to_rfc3339(),reason])?;
    let log = MecAuditLog {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: now,
        user: actor.into(),
        action: "tenant_binding_policy_associated".into(),
        resource_type: "tenant".into(),
        resource_id: op.tenant_id.clone(),
        status: AuditStatus::Success,
        duration_ms: 0,
        input: Some(
            serde_json::json!({"operation_id":op.id,"local_user_id":local,"source_role":role,"binding_id":op.binding_id,"reason":reason,"automatic_revocation":true}),
        ),
        output: None,
        error: None,
        operations: vec![],
        source_ip,
        user_agent,
    };
    AuditStore::insert_on(&tx, &log)?;
    tx.commit()?;
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssociateRequest {
    pub local_user_id: String,
    pub confirm_operation_id: String,
    pub reason: String,
    pub automatic_revocation: bool,
}
pub async fn attach(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Json<AssociateRequest>,
    state: web::Data<MecState>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    use super::super::handlers::{members, util::*};
    if let Err(r) = members::current_writer(&req, &users, &sessions) {
        return r;
    }
    let (tenant, id) = path.into_inner();
    let reason = body.reason.trim();
    if body.confirm_operation_id != id
        || !body.automatic_revocation
        || reason.len() < 3
        || reason.len() > 1024
        || reason.chars().any(char::is_control)
    {
        return from_service_error(ServiceError::InvalidInput(
            "Confirm the exact creation operation, automatic revocation and a bounded reason"
                .into(),
        ));
    }
    let op = match membership_store::get(&state.db, &tenant, &id) {
        Ok(op) => op,
        Err(e) => return from_service_error(e),
    };
    if op.operation != "create" || op.state != "completed" {
        return from_service_error(ServiceError::Conflict(
            "Only a confirmed owned creation can be associated".into(),
        ));
    }
    let source = users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == body.local_user_id)
        .cloned();
    let Some(role) = source
        .as_ref()
        .and_then(|u| source_role(u, &tenant, &op.role_template))
    else {
        return from_service_error(ServiceError::Conflict(
            "Approved active membership and role ceiling are required".into(),
        ));
    };
    let realm = match state.services.rancher.identity_realm() {
        Ok(r) => r,
        Err(e) => return from_service_error(e),
    };
    if realm != op.realm {
        return from_service_error(ServiceError::Conflict("Provider realm changed".into()));
    }
    let identity = match identity_store::get(&state.db, &body.local_user_id, &realm) {
        Ok(link) => link,
        Err(e) => return from_service_error(e),
    };
    if !identity.is_some_and(|link| link.remote_user_id == op.remote_user_id) {
        return from_service_error(ServiceError::Conflict(
            "Explicit external identity does not match owned binding".into(),
        ));
    }
    let bindings = match state.services.rancher.list_prtb(&op.project_id).await {
        Ok(b) => b,
        Err(_) => {
            return from_service_error(ServiceError::upstream(
                "rancher",
                "Binding inventory could not be verified",
            ))
        }
    };
    if bindings.iter().any(|b| b.project_id != op.project_id)
        || !bindings.iter().any(|b| {
            Some(&b.id) == op.binding_id.as_ref()
                && b.project_id == op.project_id
                && b.user_id == op.remote_user_id
                && b.role_template == op.role_template
        })
    {
        return from_service_error(ServiceError::Conflict(
            "Exact owned binding was not confirmed".into(),
        ));
    }
    let tenant_now = match state.tenants.get(&tenant) {
        Ok(Some(t)) => t,
        _ => return from_service_error(ServiceError::Conflict("Tenant is unavailable".into())),
    };
    if tenant_now.status != TenantStatus::Active
        || tenant_now.rancher_project_id.as_deref() != Some(op.project_id.as_str())
    {
        return from_service_error(ServiceError::Conflict(
            "Active tenant project changed".into(),
        ));
    }
    if let Err(r) = members::current_writer(&req, &users, &sessions) {
        return r;
    }
    let current = users.lock().unwrap();
    let active_sessions = sessions.lock().unwrap();
    let Some(claims) = crate::auth::verified_claims(&req) else {
        return HttpResponse::Unauthorized().finish();
    };
    if claims.exp as u128 <= chrono::Utc::now().timestamp().max(0) as u128
        || !active_sessions.get(&claims.sid).is_some_and(|s| {
            s.username == claims.sub
                && Some(s.user_id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
        })
    {
        return HttpResponse::Unauthorized().finish();
    }
    if !current.iter().any(|u| {
        u.username == claims.sub
            && Some(u.id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
            && u.active()
            && u.super_admin()
    }) {
        return HttpResponse::Forbidden().finish();
    }
    if !current
        .iter()
        .find(|u| u.id == body.local_user_id)
        .is_some_and(|u| {
            source_role(u, &tenant, &op.role_template).as_deref() == Some(role.as_str())
        })
    {
        return from_service_error(ServiceError::Conflict(
            "Source policy changed before association".into(),
        ));
    }
    match associate(
        &state.db,
        &op,
        &body.local_user_id,
        &role,
        &claims.sub,
        reason,
        source_ip(&req),
        user_agent(&req),
    ) {
        Ok(()) => no_store(created(
            serde_json::json!({"operation_id":op.id,"local_user_id":body.local_user_id,"automatic_revocation":true}),
        )),
        Err(e) => from_service_error(e),
    }
}
