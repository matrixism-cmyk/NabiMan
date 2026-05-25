use super::util::{created, current_user, from_service_error, list_response, no_content, ok_response};
use crate::mec::MecState;
use crate::models::mec::{CreateNatRuleRequest, UpdateNatRuleRequest};
use actix_web::{web, HttpRequest, HttpResponse};

pub async fn public_ips(state: web::Data<MecState>) -> HttpResponse {
    match state.services.axgate.list_public_ips().await {
        Ok(v) => list_response(v),
        Err(e) => from_service_error(e),
    }
}

pub async fn list_nat(state: web::Data<MecState>) -> HttpResponse {
    match state.services.axgate.list_nat_rules().await {
        Ok(v) => list_response(v),
        Err(e) => from_service_error(e),
    }
}

pub async fn add_nat(
    req: HttpRequest,
    body: web::Json<CreateNatRuleRequest>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let audit = super::audit_helper::begin_audit(
        &req,
        &state,
        &user,
        "firewall_nat_add",
        "nat_rule",
        &body.public_ip,
    )
    .with_input(serde_json::to_value(&*body).unwrap_or_default());
    match state.services.axgate.add_nat_rule(&body).await {
        Ok(r) => {
            audit.success(Some(serde_json::json!({"rule_id": &r.id})));
            if let Some(ch) = state.channels.as_ref() {
                crate::mec::notify::MecNotify::NatRuleAdded {
                    rule_id: &r.id,
                    label: r.label.as_deref().unwrap_or("-"),
                    public_ip: &body.public_ip,
                    user: &user,
                }
                .dispatch(ch);
            }
            created(r)
        }
        Err(e) => {
            audit.failure(e.to_string());
            from_service_error(e)
        }
    }
}

pub async fn delete_nat(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let id = path.into_inner();
    if let Err(e) = crate::mec::safety::require_confirm_header(&req, &id) {
        return from_service_error(e);
    }
    let audit =
        super::audit_helper::begin_audit(&req, &state, &user, "firewall_nat_delete", "nat_rule", &id);
    match state.services.axgate.delete_nat_rule(&id).await {
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

pub async fn patch_nat(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<UpdateNatRuleRequest>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let id = path.into_inner();
    let audit = super::audit_helper::begin_audit(
        &req,
        &state,
        &user,
        "firewall_nat_patch",
        "nat_rule",
        &id,
    )
    .with_input(serde_json::to_value(&*body).unwrap_or_default());
    if let Some(enabled) = body.enabled {
        if let Err(e) = state.services.axgate.toggle_nat_rule(&id, enabled).await {
            audit.failure(e.to_string());
            return from_service_error(e);
        }
    }
    audit.success(None);
    ok_response(serde_json::json!({"status": "updated"}))
}

pub async fn security_policies(state: web::Data<MecState>) -> HttpResponse {
    match state.services.axgate.list_security_policies().await {
        Ok(v) => list_response(v),
        Err(e) => from_service_error(e),
    }
}

pub async fn sync(state: web::Data<MecState>) -> HttpResponse {
    match state.services.axgate.sync_running_config().await {
        Ok(r) => ok_response(r),
        Err(e) => from_service_error(e),
    }
}
