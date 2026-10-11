use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantRequest {
    #[serde(default)]
    pub expected_delegation_version: Option<String>,
    pub expected_realm: String,
    pub expected_remote_user_id: String,
    pub local_user_id: String,
    pub role_template: String,
    pub reason: String,
    pub automatic_revocation: bool,
    pub acknowledge_project_role_scope: bool,
}
pub async fn grant(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<GrantRequest>,
    state: web::Data<MecState>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    use super::super::handlers::util::*;
    let actor = match super::super::access_delegation::current_actor(&req, &users, &sessions) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let tenant = path.into_inner();
    if !actor.super_admin() && !actor.manages(&ScopeKind::Tenant, &tenant) {
        return HttpResponse::Forbidden().finish();
    }
    let reason = body.reason.trim();
    if !body.automatic_revocation
        || !body.acknowledge_project_role_scope
        || reason.len() < 3
        || reason.len() > 1024
        || reason.chars().any(char::is_control)
    {
        return from_service_error(ServiceError::InvalidInput("Explicit automatic revocation, project role scope review and a bounded reason are required".into()));
    }
    let source = users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == body.local_user_id)
        .cloned();
    if !actor.super_admin()
        && !source.as_ref().is_some_and(|target| {
            super::super::access_delegation::lower_member(&actor, target, &tenant)
        })
    {
        return HttpResponse::Forbidden().finish();
    }
    let Some(role) = source
        .as_ref()
        .and_then(|u| source_role(u, &tenant, &body.role_template))
    else {
        return from_service_error(ServiceError::Conflict(
            "Approved active tenant membership and explicit role ceiling are required".into(),
        ));
    };
    let realm = match state.services.rancher.identity_realm() {
        Ok(r) => r,
        Err(e) => return from_service_error(e),
    };
    let link = match identity_store::get(&state.db, &body.local_user_id, &realm) {
        Ok(Some(l)) => l,
        Ok(None) => {
            return from_service_error(ServiceError::Conflict(
                "Explicit provider identity is required before creating access".into(),
            ))
        }
        Err(e) => return from_service_error(e),
    };
    if body.expected_realm != realm || body.expected_remote_user_id != link.remote_user_id {
        return from_service_error(ServiceError::Conflict(
            "Reviewed provider identity changed; reload before granting access".into(),
        ));
    }
    let profile = match state.tenants.get(&tenant) {
        Ok(Some(t)) => t,
        Ok(None) => return from_service_error(ServiceError::NotFound("tenant".into())),
        Err(e) => return from_service_error(e.into()),
    };
    let Some(project) = profile
        .rancher_project_id
        .filter(|_| profile.status == TenantStatus::Active)
    else {
        return from_service_error(ServiceError::Conflict(
            "An active tenant project is required".into(),
        ));
    };
    if let Err(e) = super::super::access_delegation::authorize(
        &state.db,
        &actor,
        source.as_ref().unwrap(),
        &tenant,
        &realm,
        &project,
        &body.role_template,
        body.expected_delegation_version.as_deref(),
    ) {
        return from_service_error(e);
    }
    let remote = match state.services.rancher.get_user(&link.remote_user_id).await {
        Ok(u) => u,
        Err(_) => {
            return from_service_error(ServiceError::upstream(
                "rancher",
                "Linked remote account could not be verified",
            ))
        }
    };
    if !remote.enabled || remote.id != link.remote_user_id {
        return from_service_error(ServiceError::Conflict(
            "Linked remote account is inactive or its identity changed".into(),
        ));
    }
    let bindings = match state.services.rancher.list_prtb(&project).await {
        Ok(b) => b,
        Err(_) => {
            return from_service_error(ServiceError::upstream(
                "rancher",
                "Existing project bindings could not be verified",
            ))
        }
    };
    if bindings.iter().any(|b| b.project_id != project) {
        return from_service_error(ServiceError::upstream(
            "rancher",
            "Project inventory scope could not be verified",
        ));
    }
    if bindings.iter().any(|b| b.user_id == link.remote_user_id) {
        return from_service_error(ServiceError::Conflict(
            "Existing direct access must be reviewed before creating another role".into(),
        ));
    }
    if let Err(r) = super::super::access_delegation::current_actor(&req, &users, &sessions) {
        return r;
    }
    let expected = ProjectRoleBinding {
        id: String::new(),
        project_id: project.clone(),
        user_id: link.remote_user_id.clone(),
        role_template: body.role_template.clone(),
    };
    let op = {
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
        let Some(current_actor) = current.iter().find(|u| {
            u.username == claims.sub
                && Some(u.id.as_str()) == crate::auth::verified_user_id(&req).as_deref()
                && u.active()
        }) else {
            return HttpResponse::Forbidden().finish();
        };
        let Some(current_target) = current.iter().find(|u| u.id == body.local_user_id) else {
            return HttpResponse::Forbidden().finish();
        };
        let delegation = match super::super::access_delegation::authorize(
            &state.db,
            current_actor,
            current_target,
            &tenant,
            &realm,
            &project,
            &body.role_template,
            body.expected_delegation_version.as_deref(),
        ) {
            Ok(d) => d,
            Err(e) => return from_service_error(e),
        };
        if !current
            .iter()
            .find(|u| u.id == body.local_user_id)
            .is_some_and(|u| {
                source_role(u, &tenant, &body.role_template).as_deref() == Some(role.as_str())
            })
        {
            return from_service_error(ServiceError::Conflict(
                "Source approval changed before dispatch".into(),
            ));
        }
        match membership_store::begin_managed(
            &state.db,
            &tenant,
            &realm,
            &expected,
            &claims.sub,
            source_ip(&req),
            user_agent(&req),
            membership_store::ManagedIntent {
                delegation: delegation
                    .as_ref()
                    .map(|d| membership_store::DelegatedIntent {
                        manager_id: &current_actor.id,
                        version: &d.version,
                    }),
                local_user_id: &body.local_user_id,
                source_role: &role,
                reason,
            },
        ) {
            Ok(op) => op,
            Err(e) => return from_service_error(e),
        }
    };
    let outcome = async {
        let binding = state
            .services
            .rancher
            .create_prtb(&project, &link.remote_user_id, &body.role_template)
            .await
            .map_err(|_| "Provider creation outcome was not confirmed")?;
        if binding.project_id != project
            || binding.user_id != link.remote_user_id
            || binding.role_template != body.role_template
            || !membership_store::valid_binding_id(&binding.id)
        {
            return Err("Provider returned a conflicting binding identity");
        }
        membership_store::observe(&state.db, &op, &binding.id)
            .map_err(|_| "Created binding ID could not be checkpointed")?;
        let inventory = state
            .services
            .rancher
            .list_prtb(&project)
            .await
            .map_err(|_| "Created binding could not be read back")?;
        if inventory.iter().any(|b| b.project_id != project)
            || !inventory.iter().any(|b| {
                b.id == binding.id
                    && b.user_id == link.remote_user_id
                    && b.role_template == body.role_template
            })
        {
            return Err("Created binding was not confirmed in project inventory");
        }
        membership_store::complete(&state.db, &op)
            .map_err(|_| "Owned binding, managed policy and audit could not commit")?;
        Ok(binding.id)
    }
    .await;
    let binding_id = match outcome {
        Ok(id) => id,
        Err(message) => {
            if membership_store::unknown(&state.db, &op, message).is_err() {
                return from_service_error(ServiceError::Internal(format!(
                    "Managed grant {} requires review; final state could not persist",
                    op.id
                )));
            }
            return from_service_error(ServiceError::upstream(
                "rancher",
                format!(
                    "Managed grant {} has an unknown result; do not replay creation",
                    op.id
                ),
            ));
        }
    };
    // Outcome tracking survives caller or target revocation during provider IO.
    let receipt_actor =
        match super::super::access_delegation::current_actor(&req, &users, &sessions) {
            Ok(a) => a,
            Err(r) => return r,
        };
    let receipt_target = users
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == body.local_user_id)
        .cloned();
    if !receipt_actor.super_admin() {
        let Some(target) = receipt_target.as_ref() else {
            return HttpResponse::Forbidden().finish();
        };
        if let Err(e) = super::super::access_delegation::authorize(
            &state.db,
            &receipt_actor,
            target,
            &tenant,
            &realm,
            &project,
            &body.role_template,
            body.expected_delegation_version.as_deref(),
        ) {
            return from_service_error(e);
        }
    }
    let tenant_context_allowed = state
        .tenants
        .get(&tenant)
        .ok()
        .flatten()
        .is_some_and(|current| {
            current.status == TenantStatus::Active
                && current.rancher_project_id.as_deref() == Some(project.as_str())
        });
    let currently_allowed = tenant_context_allowed
        && users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.id == body.local_user_id)
            .is_some_and(|u| {
                source_role(u, &tenant, &body.role_template).as_deref() == Some(role.as_str())
            });
    let mut response = created(
        serde_json::json!({"operation_id":op.id,"local_user_id":body.local_user_id,"binding_id":binding_id,"automatic_revocation":true,"source_access_currently_allowed":currently_allowed,"state":"completed"}),
    );
    if !currently_allowed {
        *response.status_mut() = actix_web::http::StatusCode::ACCEPTED;
    }
    no_store(response)
}
