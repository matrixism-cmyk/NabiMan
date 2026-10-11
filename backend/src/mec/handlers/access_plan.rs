//! Inspect current local approval and provider bindings without granting access.
use super::util::{from_service_error, ok_response};
use crate::mec::{db::identity_store, services::ServiceError, MecState};
use crate::models::{ScopeKind, UserRole};
use crate::users::UserStore;
use actix_web::{web, HttpRequest, HttpResponse};

pub async fn inspect(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    users: web::Data<UserStore>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let Some(grant) = crate::terminal_tickets::ConnectionGrant::from_request(
        &req,
        sessions.get_ref().clone(),
        users.get_ref().clone(),
    )
    .filter(|grant| grant.valid()) else {
        return HttpResponse::Unauthorized().finish();
    };
    let (tenant_id, user_id) = path.into_inner();
    let actor = match crate::membership::actor(&req, &users) {
        Ok(actor) => actor,
        Err(r) => return r,
    };
    if !actor.manages(&ScopeKind::Tenant, &tenant_id) {
        return HttpResponse::Forbidden().finish();
    }
    let target = match users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == user_id)
        .cloned()
    {
        Some(target) => target,
        None => return from_service_error(ServiceError::NotFound("tenant member".into())),
    };
    let membership = target
        .memberships
        .iter()
        .find(|m| m.kind == ScopeKind::Tenant && m.scope_id == tenant_id)
        .cloned();
    // Scoped administrators inspect only lower, approved members of their scope.
    if !actor.super_admin()
        && (target.super_admin()
            || !target.active()
            || target.id == actor.id
            || membership
                .as_ref()
                .is_none_or(|m| m.role == UserRole::Admin))
    {
        return HttpResponse::Forbidden().finish();
    }
    let tenant = match state.tenants.get(&tenant_id) {
        Ok(Some(tenant)) => tenant,
        Ok(None) => return from_service_error(ServiceError::NotFound("tenant".into())),
        Err(e) => return from_service_error(e.into()),
    };
    let source_allowed = target.active() && membership.as_ref().is_some_and(|m| m.accessible());
    let reference_role = if source_allowed {
        membership.as_ref().map(|m| match m.role {
            UserRole::Viewer => "read-only",
            UserRole::Operator => "project-member",
            UserRole::Admin => "project-owner",
        })
    } else {
        None
    };
    let realm = match state.services.rancher.identity_realm() {
        Ok(realm) => realm,
        Err(e) => return from_service_error(e),
    };
    let link = match identity_store::get(&state.db, &user_id, &realm) {
        Ok(link) => link,
        Err(e) => return from_service_error(e),
    };
    let mut blockers = Vec::new();
    if tenant.status != crate::models::mec::TenantStatus::Active {
        blockers.push("tenant_not_active");
    }
    if !source_allowed {
        blockers.push("local_access_denied");
    }
    if tenant.rancher_project_id.is_none() {
        blockers.push("project_unconfigured");
    }
    if link.is_none() {
        blockers.push("identity_unlinked");
    }
    let mut bindings = Vec::new();
    let mut remote_enabled = None;
    if let (Some(project), Some(link)) = (&tenant.rancher_project_id, &link) {
        let remote = match state.services.rancher.get_user(&link.remote_user_id).await {
            Ok(user) => user,
            Err(e) => return from_service_error(e),
        };
        if remote.id != link.remote_user_id {
            return from_service_error(ServiceError::upstream(
                "rancher",
                "Remote identity response does not match the linked account",
            ));
        }
        remote_enabled = Some(remote.enabled);
        if !remote.enabled {
            blockers.push("remote_account_disabled");
        }
        let all = match state.services.rancher.list_prtb(project).await {
            Ok(bindings) => bindings,
            Err(e) => return from_service_error(e),
        };
        if all.iter().any(|binding| &binding.project_id != project) {
            return from_service_error(ServiceError::upstream(
                "rancher",
                "Project member scope could not be verified",
            ));
        }
        bindings = all
            .into_iter()
            .filter(|binding| binding.user_id == link.remote_user_id)
            .collect();
    }
    // Re-read local authority after upstream I/O; a scoped manager can lose its
    // grant while waiting. Do not deliver a stale authorized snapshot.
    if !grant.valid()
        || crate::auth::verified_claims(&req).is_none_or(|claims| {
            claims.exp as u128 <= chrono::Utc::now().timestamp().max(0) as u128
        })
    {
        return HttpResponse::Unauthorized().finish();
    }
    let current_actor = match crate::membership::actor(&req, &users) {
        Ok(actor) => actor,
        Err(r) => return r,
    };
    let current_target = users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == user_id)
        .cloned();
    if !current_actor.manages(&ScopeKind::Tenant, &tenant_id) {
        return HttpResponse::Forbidden().finish();
    }
    if !current_actor.super_admin()
        && (target.super_admin()
            || !target.active()
            || target.id == current_actor.id
            || membership
                .as_ref()
                .is_none_or(|m| m.role == UserRole::Admin))
    {
        return HttpResponse::Forbidden().finish();
    }
    if current_target.as_ref().is_none_or(|u| {
        u.status != target.status || u.role != target.role || u.memberships != target.memberships
    }) {
        return from_service_error(ServiceError::Conflict(
            "Member policy changed during inspection; refresh the report".into(),
        ));
    }
    if source_allowed != (target.active() && membership.as_ref().is_some_and(|m| m.accessible())) {
        return from_service_error(ServiceError::Conflict(
            "Member access expired during inspection; refresh the report".into(),
        ));
    }
    match state.tenants.get(&tenant_id) {
        Ok(Some(current))
            if current.rancher_project_id == tenant.rancher_project_id
                && current.status == tenant.status => {}
        Ok(_) => {
            return from_service_error(ServiceError::Conflict(
                "Tenant project changed during inspection; refresh the report".into(),
            ))
        }
        Err(e) => return from_service_error(e.into()),
    }
    let project_role_matches = blockers.is_empty()
        && bindings.len() == 1
        && reference_role.is_some_and(|role| bindings[0].role_template == role);
    let mut response = ok_response(serde_json::json!({
        "tenant_id":tenant_id,"local_user_id":user_id,"local_username":target.username,
        "inspected_at":chrono::Utc::now(),"source_access_allowed":source_allowed,"membership":membership,
        "realm":realm,"project_id":tenant.rancher_project_id,"identity_link":link.as_ref().map(|link| serde_json::json!({"remote_user_id":link.remote_user_id,"remote_username":link.remote_username,"verified_at":link.verified_at})),
        "remote_account_enabled":remote_enabled,"reference_project_role":reference_role,
        "direct_bindings":bindings,"direct_project_role_matches":project_role_matches,"blockers":blockers,
        "unverified_access_sources":["global_roles","cluster_roles","group_membership","other_project_roles","custom_role_rules"],
        "mutation_supported":false
    }));
    response.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static("no-store"),
    );
    response
}
