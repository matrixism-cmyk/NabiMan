#[path = "member_reconciliation.rs"]
mod reconciliation;
pub use reconciliation::{operations, reconcile};

#[path = "member_addition.rs"]
mod addition;
pub use addition::add;

use super::util::{created, current_user, from_service_error, list_response, no_content};
use crate::jwt_sessions::SessionStore;
use crate::mec::db::membership_store;
use crate::mec::services::{rancher_service::ProjectRoleBinding, ServiceError};
use crate::mec::MecState;
use crate::users::UserStore;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

pub async fn list(path: web::Path<String>, state: web::Data<MecState>) -> HttpResponse {
    let tenant_id = path.into_inner();
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
    let Some(project_id) = tenant.rancher_project_id else {
        return list_response(Vec::<serde_json::Value>::new());
    };
    match state.services.rancher.list_prtb(&project_id).await {
        Ok(bindings) => {
            // Enrich with username lookup.
            let mut out = Vec::with_capacity(bindings.len());
            for b in bindings {
                let user = state.services.rancher.get_user(&b.user_id).await.ok();
                out.push(serde_json::json!({
                    "id": b.id,
                    "user_id": b.user_id,
                    "username": user.as_ref().map(|u| &u.username),
                    "display_name": user.as_ref().and_then(|u| u.display_name.clone()),
                    "role_template": b.role_template,
                }));
            }
            list_response(out)
        }
        Err(e) => from_service_error(e),
    }
}

pub async fn remove(
    req: HttpRequest,
    path: web::Path<(String, String)>,
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
    let (tenant_id, binding_id) = path.into_inner();
    let audit = super::audit_helper::begin_audit(
        &req,
        &state,
        &user,
        "tenant_member_remove",
        "tenant",
        &tenant_id,
    )
    .with_input(serde_json::json!({ "binding_id": binding_id.clone() }));
    // The binding ID is globally addressable upstream. A tenant in the URL is
    // not proof that the binding belongs to it: establish that relationship
    // before making any destructive request.
    let tenant = match state.tenants.get(&tenant_id) {
        Ok(Some(tenant)) => tenant,
        Ok(None) => {
            return from_service_error(crate::mec::services::ServiceError::NotFound(
                "tenant".into(),
            ))
        }
        Err(e) => return from_service_error(e.into()),
    };
    let Some(project_id) = tenant.rancher_project_id else {
        return from_service_error(crate::mec::services::ServiceError::InvalidInput(
            "tenant has no rancher project configured".into(),
        ));
    };
    let bindings = match state.services.rancher.list_prtb(&project_id).await {
        Ok(bindings) => bindings,
        Err(e) => {
            audit.failure(e.to_string());
            return from_service_error(e);
        }
    };
    if !bindings
        .iter()
        .any(|b| b.id == binding_id && b.project_id == project_id)
    {
        let err = crate::mec::services::ServiceError::NotFound("tenant member binding".into());
        audit.failure(err.to_string());
        return from_service_error(err);
    }
    let binding = bindings
        .iter()
        .find(|b| b.id == binding_id && b.project_id == project_id)
        .unwrap();
    let realm = match state.services.rancher.identity_realm() {
        Ok(realm) => realm,
        Err(e) => return from_service_error(e),
    };
    if let Err(r) = current_writer(&req, &users, &sessions) {
        return r;
    }
    if let Err(e) = current_project(&state, &tenant_id, &project_id, false) {
        return from_service_error(e);
    }
    let operation = match membership_store::begin(
        &state.db,
        &tenant_id,
        &realm,
        binding,
        "delete",
        &user,
        super::util::source_ip(&req),
        super::util::user_agent(&req),
    ) {
        Ok(op) => op,
        Err(e) => return from_service_error(e),
    };
    if state
        .services
        .rancher
        .delete_prtb(&binding_id)
        .await
        .is_err()
    {
        return uncertain(
            &state,
            &operation,
            "Provider delete outcome was not confirmed",
        );
    }
    let observed = match state.services.rancher.list_prtb(&project_id).await {
        Ok(bindings) => bindings,
        Err(_) => {
            return uncertain(
                &state,
                &operation,
                "Deleted binding absence could not be read back",
            )
        }
    };
    if observed.iter().any(|b| b.id == binding_id) {
        return uncertain(
            &state,
            &operation,
            "Provider still reports the binding after deletion",
        );
    }
    if membership_store::complete(&state.db, &operation).is_err() {
        return persistence_failure(&state, &operation);
    }
    if let Err(r) = current_writer(&req, &users, &sessions) {
        return r;
    }
    no_content()
}

pub(crate) fn current_writer(
    req: &HttpRequest,
    users: &UserStore,
    sessions: &SessionStore,
) -> Result<(), HttpResponse> {
    crate::membership::require_super(req, users)?;
    let claims =
        crate::auth::verified_claims(req).ok_or_else(|| HttpResponse::Unauthorized().finish())?;
    if claims.exp as u128 <= chrono::Utc::now().timestamp().max(0) as u128
        || !crate::terminal_tickets::ConnectionGrant::from_request(
            req,
            sessions.clone(),
            users.clone(),
        )
        .is_some_and(|grant| grant.valid())
    {
        return Err(HttpResponse::Unauthorized().finish());
    }
    Ok(())
}
fn current_project(
    state: &MecState,
    tenant: &str,
    project: &str,
    granting: bool,
) -> Result<(), ServiceError> {
    if state.tenants.get(tenant)?.is_some_and(|t| {
        t.rancher_project_id.as_deref() == Some(project)
            && (!granting || t.status == crate::models::mec::TenantStatus::Active)
    }) {
        Ok(())
    } else {
        Err(ServiceError::Conflict(
            "Tenant project or lifecycle changed before membership mutation".into(),
        ))
    }
}
fn uncertain(
    state: &MecState,
    operation: &membership_store::Operation,
    reason: &str,
) -> HttpResponse {
    if membership_store::unknown(&state.db, operation, reason).is_err() {
        return database_failure_response(operation);
    }
    from_service_error(ServiceError::upstream(
        "rancher",
        format!(
            "Membership operation {} has an unknown result; inspect before retrying",
            operation.id
        ),
    ))
}
fn persistence_failure(state: &MecState, operation: &membership_store::Operation) -> HttpResponse {
    if membership_store::unknown(
        &state.db,
        operation,
        "Final checkpoint or audit could not be committed",
    )
    .is_err()
    {
        eprintln!(
            "Membership operation {} requires reconciliation; final state could not be persisted",
            operation.id
        );
    }
    database_failure_response(operation)
}
fn database_failure_response(operation: &membership_store::Operation) -> HttpResponse {
    from_service_error(ServiceError::Internal(format!(
        "Membership operation {} could not persist its final outcome; inspect before retrying",
        operation.id
    )))
}
