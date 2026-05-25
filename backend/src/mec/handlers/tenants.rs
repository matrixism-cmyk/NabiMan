use super::util::{accepted, created, current_user, from_service_error, list_response, no_content, ok_response};
use crate::mec::tenant_orchestrator::TenantOrchestrator;
use crate::mec::MecState;
use crate::models::mec::CreateTenantRequest;
use actix_web::{web, HttpRequest, HttpResponse};

pub async fn list(state: web::Data<MecState>) -> HttpResponse {
    match state.tenants.list() {
        Ok(ts) => list_response(ts),
        Err(e) => from_service_error(e.into()),
    }
}

pub async fn get(path: web::Path<String>, state: web::Data<MecState>) -> HttpResponse {
    match state.tenants.get(&path) {
        Ok(Some(t)) => ok_response(t),
        Ok(None) => from_service_error(
            crate::mec::services::ServiceError::NotFound(format!("tenant {}", &path)),
        ),
        Err(e) => from_service_error(e.into()),
    }
}

pub async fn create(
    req: HttpRequest,
    body: web::Json<CreateTenantRequest>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let orch = TenantOrchestrator::new(
        state.services.clone(),
        state.audit.clone(),
        state.jobs.clone(),
        state.tenants.clone(),
    );
    let spec = body.into_inner();
    let tenant_id_copy = spec.tenant_id.clone();
    match orch.create(spec, &user).await {
        Ok(t) => {
            if let Some(ch) = state.channels.as_ref() {
                crate::mec::notify::MecNotify::TenantCreated {
                    tenant_id: &t.id,
                    display_name: &t.display_name,
                    user: &user,
                }
                .dispatch(ch);
            }
            created(t)
        }
        Err(e) => {
            if let Some(ch) = state.channels.as_ref() {
                let msg = e.to_string();
                crate::mec::notify::MecNotify::TenantCreateFailed {
                    tenant_id: &tenant_id_copy,
                    reason: &msg,
                }
                .dispatch(ch);
            }
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
    let orch = TenantOrchestrator::new(
        state.services.clone(),
        state.audit.clone(),
        state.jobs.clone(),
        state.tenants.clone(),
    );
    match orch.delete(&id, &user).await {
        Ok(_) => {
            if let Some(ch) = state.channels.as_ref() {
                crate::mec::notify::MecNotify::TenantDeleted {
                    tenant_id: &id,
                    user: &user,
                }
                .dispatch(ch);
            }
            no_content()
        }
        Err(e) => from_service_error(e),
    }
}

pub async fn quota_usage(
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    match state.services.kube.get_quota_usage(&path).await {
        Ok(q) => ok_response(q),
        Err(e) => from_service_error(e),
    }
}

pub async fn resources(path: web::Path<String>, state: web::Data<MecState>) -> HttpResponse {
    let ns = path.into_inner();
    let pods = match state.services.kube.list_pods(&ns).await {
        Ok(p) => p,
        Err(e) => return from_service_error(e),
    };
    let services = match state.services.kube.list_lb_services(Some(&ns)).await {
        Ok(s) => s,
        Err(e) => return from_service_error(e),
    };
    ok_response(serde_json::json!({
        "pods": pods,
        "services": services,
    }))
}

pub async fn guide_download(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let tenant = match state.tenants.get(&path) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return from_service_error(
                crate::mec::services::ServiceError::NotFound(format!("tenant {}", &path)),
            );
        }
        Err(e) => return from_service_error(e.into()),
    };
    let gen = crate::mec::docgen::DocGenerator::new(
        state.docs_store.clone(),
        crate::mec::docgen::DocGenerator::default_output_dir(),
    );
    let text = gen.generate_tenant_guide_text(&tenant);
    match gen.generate_tenant_guide_text_file(&tenant, &user) {
        Ok(doc) => HttpResponse::Ok()
            .insert_header((
                "Content-Disposition",
                format!(r#"attachment; filename="{}""#, doc.filename),
            ))
            .content_type("text/plain; charset=utf-8")
            .body(text),
        Err(e) => from_service_error(
            crate::mec::services::ServiceError::Internal(e.to_string()),
        ),
    }
}

pub async fn guide_download_docx(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let tenant = match state.tenants.get(&path) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return from_service_error(
                crate::mec::services::ServiceError::NotFound(format!("tenant {}", &path)),
            );
        }
        Err(e) => return from_service_error(e.into()),
    };
    let gen = crate::mec::docgen::DocxGenerator::new(
        state.docs_store.clone(),
        crate::mec::docgen::DocGenerator::default_output_dir(),
    );
    let doc = match gen.generate_tenant_guide(&tenant, &user) {
        Ok(d) => d,
        Err(e) => {
            return from_service_error(
                crate::mec::services::ServiceError::Internal(e.to_string()),
            );
        }
    };
    match std::fs::read(&doc.file_path) {
        Ok(bytes) => HttpResponse::Ok()
            .insert_header((
                "Content-Disposition",
                format!(r#"attachment; filename="{}""#, doc.filename),
            ))
            .content_type("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
            .body(bytes),
        Err(e) => from_service_error(
            crate::mec::services::ServiceError::Internal(e.to_string()),
        ),
    }
}

pub async fn create_job_placeholder() -> HttpResponse {
    accepted(serde_json::json!({
        "message": "deprecated; use POST /api/mec/v1/tenants",
    }))
}

pub async fn deploy_starter_kit(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let orch = crate::mec::starter_kit::StarterKitOrchestrator::new(
        state.services.clone(),
        state.tenants.clone(),
    );
    let id = path.into_inner();
    let audit = super::audit_helper::begin_audit(
        &req, &state, &user, "starter_kit_deploy", "tenant", &id,
    );
    match orch.deploy(&id, &user).await {
        Ok(r) => {
            audit.success(Some(serde_json::json!({"ssh_user": &r.ssh_user})));
            if let Some(ch) = state.channels.as_ref() {
                crate::mec::notify::MecNotify::StarterKitDeployed {
                    tenant_id: &id,
                    user: &user,
                }
                .dispatch(ch);
            }
            created(serde_json::json!({
                "tenant_id": r.tenant.id,
                "ssh_user": r.ssh_user,
                "ssh_password": r.ssh_password_plain,
                "vscode_password": r.vscode_password_plain,
                "message": "Starter Kit 배포 완료. 비밀번호는 다시 조회할 수 없으니 안전하게 보관하세요.",
            }))
        }
        Err(e) => {
            audit.failure(e.to_string());
            from_service_error(e)
        }
    }
}

pub async fn delete_starter_kit(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let id = path.into_inner();
    if let Err(e) = crate::mec::safety::require_confirm_header(&req, &id) {
        return from_service_error(e);
    }
    let audit = super::audit_helper::begin_audit(
        &req, &state, &user, "starter_kit_delete", "tenant", &id,
    );
    let orch = crate::mec::starter_kit::StarterKitOrchestrator::new(
        state.services.clone(),
        state.tenants.clone(),
    );
    match orch.remove(&id, &user).await {
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
