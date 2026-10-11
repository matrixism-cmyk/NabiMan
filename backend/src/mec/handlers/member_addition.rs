use super::*;

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default)]
    pub create_if_missing: bool,
}

pub(super) fn default_role() -> String {
    "project-member".into()
}

pub async fn add(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<AddMemberRequest>,
    state: web::Data<MecState>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let user = match current_user(&req) {
        Ok(user) => user,
        Err(response) => return response,
    };
    if let Err(r) = current_writer(&req, &users, &sessions) {
        return r;
    }
    if !["project-owner", "project-member", "read-only"].contains(&body.role.as_str()) {
        return from_service_error(crate::mec::services::ServiceError::InvalidInput(
            "Unsupported project role".into(),
        ));
    }
    let tenant_id = path.into_inner();
    let audit = super::super::audit_helper::begin_audit(
        &req,
        &state,
        &user,
        "tenant_member_add",
        "tenant",
        &tenant_id,
    )
    .with_input(serde_json::json!({
        "username": body.username,
        "role": body.role,
        "create_if_missing": body.create_if_missing,
    }));
    let tenant = match state.tenants.get(&tenant_id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return from_service_error(crate::mec::services::ServiceError::NotFound(format!(
                "tenant {}",
                tenant_id
            )));
        }
        Err(e) => return from_service_error(e.into()),
    };
    let Some(project_id) = tenant.rancher_project_id.clone() else {
        let err = crate::mec::services::ServiceError::InvalidInput(
            "tenant has no rancher project configured".into(),
        );
        audit.failure(err.to_string());
        return from_service_error(err);
    };
    if tenant.status != crate::models::mec::TenantStatus::Active {
        return from_service_error(ServiceError::Conflict(
            "Tenant must be active before granting project membership".into(),
        ));
    }

    // Find existing user or (optionally) create.
    let existing = match state.services.rancher.list_users().await {
        Ok(users) => users,
        Err(e) => {
            audit.failure(e.to_string());
            return from_service_error(e);
        }
    };
    let mut initial_credentials = None;
    let user_id = if let Some(found) = existing.iter().find(|u| u.username == body.username) {
        if !found.enabled {
            return from_service_error(ServiceError::Conflict(
                "Rancher account is inactive".into(),
            ));
        }
        found.id.clone()
    } else if body.create_if_missing {
        let password = match &body.password {
            Some(password) => {
                if let Err(e) = super::super::super::credentials::validate_password(password) {
                    audit.failure(e.to_string());
                    return from_service_error(e);
                }
                password.clone()
            }
            None => match super::super::super::credentials::generate_password() {
                Ok(p) => p,
                Err(e) => {
                    audit.failure(e.to_string());
                    return from_service_error(e);
                }
            },
        };
        initial_credentials = Some(super::super::super::credentials::InitialCredentials {
            username: body.username.clone(),
            password: password.clone(),
        });
        if let Err(r) = current_writer(&req, &users, &sessions) {
            return r;
        }
        if let Err(e) = current_project(&state, &tenant_id, &project_id, true) {
            return from_service_error(e);
        }
        match state
            .services
            .rancher
            .create_user(&body.username, &password)
            .await
        {
            Ok(u)
                if u.enabled
                    && u.username == body.username
                    && membership_store::valid_binding_id(&u.id) =>
            {
                u.id
            }
            Ok(_) => {
                return from_service_error(ServiceError::upstream(
                    "rancher",
                    "Created user identity could not be verified",
                ))
            }
            Err(e) => {
                let e = super::super::super::credentials::safe_error(e);
                audit.failure(e.to_string());
                return from_service_error(e);
            }
        }
    } else {
        let err = crate::mec::services::ServiceError::NotFound(format!(
            "rancher user {} (set create_if_missing=true to create)",
            body.username
        ));
        audit.failure(err.to_string());
        return from_service_error(err);
    };

    let realm = match state.services.rancher.identity_realm() {
        Ok(realm) => realm,
        Err(e) => return from_service_error(e),
    };
    let bindings = match state.services.rancher.list_prtb(&project_id).await {
        Ok(bindings) => bindings,
        Err(e) => return from_service_error(e),
    };
    if bindings.iter().any(|b| b.project_id != project_id) {
        return from_service_error(ServiceError::upstream(
            "rancher",
            "Project scope could not be verified",
        ));
    }
    if bindings.iter().any(|b| b.user_id == user_id) {
        return from_service_error(ServiceError::Conflict(
            "The account already has a direct project binding; review it before changing roles"
                .into(),
        ));
    }
    if let Err(r) = current_writer(&req, &users, &sessions) {
        return r;
    }
    if let Err(e) = current_project(&state, &tenant_id, &project_id, true) {
        return from_service_error(e);
    }
    let expected = ProjectRoleBinding {
        id: String::new(),
        project_id: project_id.clone(),
        user_id: user_id.clone(),
        role_template: body.role.clone(),
    };
    let operation = match membership_store::begin(
        &state.db,
        &tenant_id,
        &realm,
        &expected,
        "create",
        &user,
        super::super::util::source_ip(&req),
        super::super::util::user_agent(&req),
    ) {
        Ok(op) => op,
        Err(e) => return from_service_error(e),
    };
    let binding = match state
        .services
        .rancher
        .create_prtb(&project_id, &user_id, &body.role)
        .await
    {
        Ok(binding) => binding,
        Err(_) => {
            return uncertain(
                &state,
                &operation,
                "Provider create outcome was not confirmed",
            )
        }
    };
    if binding.project_id != project_id
        || binding.user_id != user_id
        || binding.role_template != body.role
        || !membership_store::valid_binding_id(&binding.id)
    {
        return uncertain(
            &state,
            &operation,
            "Provider create returned a conflicting binding identity",
        );
    }
    if membership_store::observe(&state.db, &operation, &binding.id).is_err() {
        return persistence_failure(&state, &operation);
    }
    let observed = match state.services.rancher.list_prtb(&project_id).await {
        Ok(bindings) => bindings,
        Err(_) => return uncertain(&state, &operation, "Created binding could not be read back"),
    };
    if !observed.iter().any(|b| {
        b.id == binding.id
            && b.project_id == project_id
            && b.user_id == user_id
            && b.role_template == body.role
    }) {
        return uncertain(
            &state,
            &operation,
            "Created binding was not confirmed by provider inventory",
        );
    }
    if membership_store::complete(&state.db, &operation).is_err() {
        return persistence_failure(&state, &operation);
    }
    // Persist ownership even if the initiating account was revoked during the
    // provider call; then withhold the credential receipt from that account.
    if let Err(r) = current_writer(&req, &users, &sessions) {
        return r;
    }
    let mut response = created(serde_json::json!({
        "operation_id": operation.id, "initial_credentials": initial_credentials,
        "id": binding.id, "user_id": binding.user_id, "username": body.username,
        "role_template": binding.role_template,
    }));
    response.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static("no-store"),
    );
    response
}
