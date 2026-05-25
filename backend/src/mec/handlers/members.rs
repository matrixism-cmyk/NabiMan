use super::util::{created, current_user, from_service_error, list_response, no_content};
use crate::mec::MecState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

pub async fn list(
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let tenant_id = path.into_inner();
    let tenant = match state.tenants.get(&tenant_id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return from_service_error(
                crate::mec::services::ServiceError::NotFound(format!("tenant {}", tenant_id)),
            );
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

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub username: String,
    #[serde(default = "default_password")]
    pub password: String,
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default)]
    pub create_if_missing: bool,
}

fn default_password() -> String {
    "changeme!".into()
}

fn default_role() -> String {
    "project-owner".into()
}

pub async fn add(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<AddMemberRequest>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let tenant_id = path.into_inner();
    let audit = super::audit_helper::begin_audit(
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
            return from_service_error(
                crate::mec::services::ServiceError::NotFound(format!("tenant {}", tenant_id)),
            );
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

    // Find existing user or (optionally) create.
    let existing = state.services.rancher.list_users().await.ok();
    let user_id = if let Some(found) = existing
        .as_ref()
        .and_then(|list| list.iter().find(|u| u.username == body.username))
    {
        found.id.clone()
    } else if body.create_if_missing {
        match state
            .services
            .rancher
            .create_user(&body.username, &body.password)
            .await
        {
            Ok(u) => u.id,
            Err(e) => {
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

    match state
        .services
        .rancher
        .create_prtb(&project_id, &user_id, &body.role)
        .await
    {
        Ok(binding) => {
            audit.success(Some(serde_json::json!({
                "binding_id": &binding.id,
                "user_id": &binding.user_id
            })));
            created(serde_json::json!({
                "id": binding.id,
                "user_id": binding.user_id,
                "username": body.username,
                "role_template": binding.role_template,
            }))
        }
        Err(e) => {
            audit.failure(e.to_string());
            from_service_error(e)
        }
    }
}

pub async fn remove(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
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
    match state.services.rancher.delete_prtb(&binding_id).await {
        Ok(_) => {
            audit.success(None);
            no_content()
        }
        Err(e) => {
            audit.failure(e.to_string());
            from_service_error(e)
        }
    }
}
