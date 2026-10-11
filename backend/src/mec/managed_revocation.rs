//! Scoped read of durable ownership and explicitly delegated exact-binding revocation.
use super::{
    access_delegation::{self, Delegation},
    db::{membership_store, DbHandle},
    handlers::util::*,
    managed_memberships::{self, ManagedBinding},
    services::{rancher_service::ProjectRoleBinding, ServiceError, ServiceResult},
    MecState,
};
use crate::{
    jwt_sessions::SessionStore,
    models::{mec::TenantStatus, NabimanUser, ScopeKind},
    users::UserStore,
};
use actix_web::{web, HttpRequest, HttpResponse};
use rusqlite::OptionalExtension;
use serde::Deserialize;
fn no_store(mut r: HttpResponse) -> HttpResponse {
    r.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static("no-store"),
    );
    r
}
fn target_visible(actor: &NabimanUser, target: Option<&NabimanUser>, tenant: &str) -> bool {
    actor.super_admin()
        || target.is_some_and(|u| access_delegation::lower_association(actor, u, tenant))
}
fn authorization(
    db: &DbHandle,
    actor: &NabimanUser,
    target: Option<&NabimanUser>,
    grant: &ManagedBinding,
    version: Option<&str>,
) -> ServiceResult<Option<Delegation>> {
    if actor.super_admin() {
        return Ok(None);
    }
    if !target_visible(actor, target, &grant.tenant_id) {
        return Err(ServiceError::Forbidden(
            "Current lower-member association is required".into(),
        ));
    }
    let policy = access_delegation::read(db, &grant.tenant_id)?.ok_or_else(|| {
        ServiceError::Forbidden("Explicit revocation delegation is required".into())
    })?;
    if !policy.permits_action(
        &actor.id,
        &grant.realm,
        &grant.project_id,
        &grant.role_template,
        "revoke",
    ) {
        return Err(ServiceError::Forbidden(
            "Revocation is outside the approved manager, role or expiry".into(),
        ));
    }
    if version != Some(policy.version.as_str()) {
        return Err(ServiceError::Conflict(
            "Revocation delegation changed; refresh and review again".into(),
        ));
    }
    Ok(Some(policy))
}
fn binding(state: &MecState, tenant: &str, id: &str) -> ServiceResult<ManagedBinding> {
    managed_memberships::list(&state.db, Some(tenant))?
        .into_iter()
        .find(|g| g.operation_id == id)
        .ok_or_else(|| ServiceError::NotFound("managed binding".into()))
}
fn context_matches(state: &MecState, g: &ManagedBinding) -> ServiceResult<bool> {
    Ok(state.services.rancher.identity_realm()? == g.realm
        && state
            .tenants
            .get(&g.tenant_id)?
            .is_some_and(|t| t.rancher_project_id.as_deref() == Some(g.project_id.as_str())))
}
pub async fn inventory(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let actor = match access_delegation::current_actor(&req, &users, &sessions) {
        Ok(a) => a,
        Err(r) => return r,
    };
    if !actor.super_admin() && !actor.manages(&ScopeKind::Tenant, &path) {
        return HttpResponse::Forbidden().finish();
    }
    let result = (|| {
        let realm = state.services.rancher.identity_realm()?;
        let current = users.lock().unwrap();
        let caller = current
            .iter()
            .find(|u| u.id == actor.id && u.active())
            .ok_or_else(|| ServiceError::Forbidden("Actor changed".into()))?;
        if !caller.super_admin() && !caller.manages(&ScopeKind::Tenant, &path) {
            return Err(ServiceError::Forbidden(
                "Tenant manager scope changed".into(),
            ));
        }
        // One connection guard keeps ownership, pending state, tenant mapping
        // and policy metadata consistent against all application writers.
        let conn = state.db.lock().unwrap();
        let profile: Option<(Option<String>, String)> = conn
            .query_row(
                "SELECT rancher_project_id,status FROM tenant_profiles WHERE id=?1",
                [path.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (project, profile_status) =
            profile.ok_or_else(|| ServiceError::NotFound("tenant".into()))?;
        let status: TenantStatus = serde_json::from_str(&profile_status)?;
        let grants = managed_memberships::list_on(&conn, Some(&path))?;
        let policy = access_delegation::read_on(&conn, &path)?;
        let mut rows = Vec::new();
        for g in grants {
            let target = current.iter().find(|u| u.id == g.local_user_id);
            if !target_visible(caller, target, &path) {
                continue;
            }
            let matches = realm == g.realm && project.as_deref() == Some(g.project_id.as_str());
            let source_allowed = matches
                && status == TenantStatus::Active
                && target.is_some_and(|u| {
                    managed_memberships::source_role(u, &path, &g.role_template).as_deref()
                        == Some(g.source_role.as_str())
                });
            let pending = membership_store::unresolved_member_change_on(
                &conn,
                &g.realm,
                &g.project_id,
                &g.remote_user_id,
            )?;
            let latest = membership_store::latest_binding_change_on(
                &conn,
                &path,
                &g.realm,
                &g.project_id,
                &g.remote_user_id,
                &g.binding_id,
            )?;
            let delegated = policy.as_ref().is_some_and(|p| {
                p.permits_action(
                    &caller.id,
                    &g.realm,
                    &g.project_id,
                    &g.role_template,
                    "revoke",
                )
            });
            rows.push(serde_json::json!({"operation_id":g.operation_id,"local_user_id":g.local_user_id,"username":target.map(|u|u.username.as_str()),"realm":g.realm,"project_id":g.project_id,"remote_user_id":g.remote_user_id,"role_template":g.role_template,"binding_id":g.binding_id,"ownership_state":g.ownership_state,"ownership_updated_at":g.ownership_updated_at,"source_access_currently_allowed":source_allowed,"context_matches":matches,"unresolved_change":pending,"can_revoke":matches && !pending && g.ownership_state=="active" && (caller.super_admin() || delegated),"delegation_version":if caller.super_admin(){None}else{policy.as_ref().map(|p|p.version.as_str())},"latest_change":latest.map(|op|serde_json::json!({"id":op.id,"operation":op.operation,"state":op.state,"updated_at":op.updated_at}))}));
        }
        Ok::<_, ServiceError>(rows)
    })();
    match result {
        Ok(rows) => no_store(list_response(rows)),
        Err(e) => no_store(from_service_error(e)),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokeRequest {
    pub confirm_operation_id: String,
    pub expected_binding_id: String,
    pub expected_updated_at: String,
    pub expected_delegation_version: Option<String>,
    pub reason: String,
    pub acknowledge_direct_binding_only: bool,
}
pub async fn revoke(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Json<RevokeRequest>,
    state: web::Data<MecState>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let actor = match access_delegation::current_actor(&req, &users, &sessions) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let (tenant, id) = path.into_inner();
    if !actor.super_admin() && !actor.manages(&ScopeKind::Tenant, &tenant) {
        return HttpResponse::Forbidden().finish();
    }
    let reason = body.reason.trim();
    if body.confirm_operation_id != id
        || reason.len() < 3
        || reason.len() > 1024
        || reason.chars().any(char::is_control)
        || !body.acknowledge_direct_binding_only
    {
        return no_store(from_service_error(ServiceError::InvalidInput(
            "Confirm the managed operation ID, bounded reason and direct-binding scope".into(),
        )));
    }
    let grant = match binding(&state, &tenant, &id) {
        Ok(g) => g,
        Err(e) => return no_store(from_service_error(e)),
    };
    let target = users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == grant.local_user_id)
        .cloned();
    if let Err(e) = authorization(
        &state.db,
        &actor,
        target.as_ref(),
        &grant,
        body.expected_delegation_version.as_deref(),
    ) {
        return no_store(from_service_error(e));
    }
    if grant.ownership_state != "active"
        || grant.binding_id != body.expected_binding_id
        || grant.ownership_updated_at != body.expected_updated_at
    {
        return no_store(from_service_error(ServiceError::Conflict(
            "Managed binding changed; do not infer a replacement".into(),
        )));
    }
    match membership_store::unresolved_member_change_on(&state.db.lock().unwrap(), &grant.realm, &grant.project_id, &grant.remote_user_id) {
        Ok(true) => return no_store(from_service_error(ServiceError::Conflict("An unresolved member change requires highest-administrator confirmation before another revocation".into()))),
        Ok(false) => (),
        Err(e) => return no_store(from_service_error(e)),
    }
    match context_matches(&state, &grant) {
        Ok(true) => (),
        Ok(false) => {
            return no_store(from_service_error(ServiceError::Conflict(
                "Provider realm or tenant project changed".into(),
            )))
        }
        Err(e) => return no_store(from_service_error(e)),
    }
    let observed = match state.services.rancher.list_prtb(&grant.project_id).await {
        Ok(rows) => rows,
        Err(_) => {
            return no_store(from_service_error(ServiceError::upstream(
                "rancher",
                "Project inventory could not be verified before revocation",
            )))
        }
    };
    if observed.iter().any(|b| b.project_id != grant.project_id)
        || observed
            .iter()
            .find(|b| b.id == grant.binding_id)
            .is_some_and(|b| {
                b.user_id != grant.remote_user_id || b.role_template != grant.role_template
            })
    {
        return no_store(from_service_error(ServiceError::Conflict(
            "Provider binding identity or project scope changed".into(),
        )));
    }
    let present = observed.iter().any(|b| b.id == grant.binding_id);
    if let Err(r) = access_delegation::current_actor(&req, &users, &sessions) {
        return r;
    }
    let op = {
        let current = users.lock().unwrap();
        let live = sessions.lock().unwrap();
        let Some(claims) = crate::auth::verified_claims(&req) else {
            return HttpResponse::Unauthorized().finish();
        };
        if claims.exp as u128 <= chrono::Utc::now().timestamp().max(0) as u128
            || !live.get(&claims.sid).is_some_and(|s| {
                s.username == claims.sub
                    && Some(s.user_id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
            })
        {
            return HttpResponse::Unauthorized().finish();
        }
        let Some(caller) = current.iter().find(|u| {
            u.username == claims.sub
                && Some(u.id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
                && u.active()
        }) else {
            return HttpResponse::Forbidden().finish();
        };
        let target = current.iter().find(|u| u.id == grant.local_user_id);
        let policy = match authorization(
            &state.db,
            caller,
            target,
            &grant,
            body.expected_delegation_version.as_deref(),
        ) {
            Ok(p) => p,
            Err(e) => return no_store(from_service_error(e)),
        };
        let exact = ProjectRoleBinding {
            id: grant.binding_id.clone(),
            project_id: grant.project_id.clone(),
            user_id: grant.remote_user_id.clone(),
            role_template: grant.role_template.clone(),
        };
        match membership_store::begin_managed_revocation(
            &state.db,
            &tenant,
            &grant.realm,
            &exact,
            &claims.sub,
            source_ip(&req),
            user_agent(&req),
            membership_store::RevocationIntent {
                grant_operation_id: &grant.operation_id,
                local_user_id: &grant.local_user_id,
                expected_updated_at: &body.expected_updated_at,
                manager_id: &caller.id,
                delegation_version: policy.as_ref().map(|p| p.version.as_str()),
                reason,
            },
        ) {
            Ok(op) => op,
            Err(e) => return no_store(from_service_error(e)),
        }
    };
    let outcome = async {
        if present {
            state
                .services
                .rancher
                .delete_prtb(&grant.binding_id)
                .await
                .map_err(|_| "Managed revocation provider outcome was not confirmed")?;
        }
        let rows = state
            .services
            .rancher
            .list_prtb(&grant.project_id)
            .await
            .map_err(|_| "Managed revocation absence could not be read back")?;
        if rows
            .iter()
            .any(|b| b.id == grant.binding_id || b.project_id != grant.project_id)
        {
            return Err("Managed revocation absence was not verified");
        }
        membership_store::complete(&state.db, &op)
            .map_err(|_| "Managed revocation audit or ownership checkpoint could not commit")?;
        Ok(())
    }
    .await;
    if let Err(message) = outcome {
        if membership_store::unknown(&state.db, &op, message).is_err() {
            return no_store(from_service_error(ServiceError::Internal(format!(
                "Revocation {} requires review; final state could not persist",
                op.id
            ))));
        }
        return no_store(from_service_error(ServiceError::upstream(
            "rancher",
            format!(
                "Revocation {} has an unknown result; do not replay deletion",
                op.id
            ),
        )));
    }
    let caller = match access_delegation::current_actor(&req, &users, &sessions) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let target = users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == grant.local_user_id)
        .cloned();
    if let Err(e) = authorization(
        &state.db,
        &caller,
        target.as_ref(),
        &grant,
        body.expected_delegation_version.as_deref(),
    ) {
        return no_store(from_service_error(e));
    }
    no_store(ok_response(
        serde_json::json!({"operation_id":op.id,"grant_operation_id":grant.operation_id,"binding_id":grant.binding_id,"state":"completed","ownership_state":"revoked","direct_binding_absence_confirmed":true,"all_external_access_revoked":false,"provider_delete_dispatched":present}),
    ))
}
