use super::audit_helper::begin_audit;
use super::util::{created, current_user, from_service_error, list_response, no_content, ok_response};
use crate::mec::MecState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

pub async fn list(state: web::Data<MecState>) -> HttpResponse {
    match state.services.rancher.list_users().await {
        Ok(v) => list_response(v),
        Err(e) => from_service_error(e),
    }
}

pub async fn get(path: web::Path<String>, state: web::Data<MecState>) -> HttpResponse {
    match state.services.rancher.get_user(&path).await {
        Ok(u) => ok_response(u),
        Err(e) => from_service_error(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

pub async fn create(
    req: HttpRequest,
    body: web::Json<CreateUserRequest>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let audit = begin_audit(&req, &state, &user, "user_create", "user", &body.username)
        .with_input(serde_json::json!({ "username": &body.username }));
    match state
        .services
        .rancher
        .create_user(&body.username, &body.password)
        .await
    {
        Ok(u) => {
            audit.success(Some(serde_json::json!({"user_id": &u.id})));
            created(u)
        }
        Err(e) => {
            audit.failure(e.to_string());
            from_service_error(e)
        }
    }
}

pub async fn delete(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let id = path.into_inner();
    if let Err(e) = crate::mec::safety::require_confirm_header(&req, &id) {
        return from_service_error(e);
    }
    let audit = begin_audit(&req, &state, &user, "user_delete", "user", &id);
    match state.services.rancher.delete_user(&id).await {
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

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub new_password: String,
}

pub async fn reset_password(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<ResetPasswordRequest>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let id = path.into_inner();
    if let Err(e) = crate::mec::safety::require_confirm_header(&req, &id) {
        return from_service_error(e);
    }
    let audit = begin_audit(&req, &state, &user, "user_reset_password", "user", &id);
    match state
        .services
        .rancher
        .reset_password(&id, &body.new_password)
        .await
    {
        Ok(_) => {
            audit.success(None);
            ok_response(serde_json::json!({"status": "reset"}))
        }
        Err(e) => {
            audit.failure(e.to_string());
            from_service_error(e)
        }
    }
}
