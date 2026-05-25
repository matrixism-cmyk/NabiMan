use super::util::{current_user, from_service_error, list_response, no_content, ok_response};
use crate::mec::MecState;
use crate::models::mec::{GpuModeSwitchRequest, LabelPatch, Taint, TaintPatch};
use actix_web::{web, HttpRequest, HttpResponse};

pub async fn list(state: web::Data<MecState>) -> HttpResponse {
    match state.services.kube.list_nodes().await {
        Ok(n) => list_response(n),
        Err(e) => from_service_error(e),
    }
}

pub async fn get(path: web::Path<String>, state: web::Data<MecState>) -> HttpResponse {
    match state.services.kube.get_node(&path).await {
        Ok(n) => ok_response(n),
        Err(e) => from_service_error(e),
    }
}

pub async fn pods(path: web::Path<String>, state: web::Data<MecState>) -> HttpResponse {
    match state.services.kube.list_pods_on_node(&path).await {
        Ok(p) => list_response(p),
        Err(e) => from_service_error(e),
    }
}

pub async fn patch_labels(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<LabelPatch>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let node = path.into_inner();
    let add: Vec<(String, String)> = body
        .add
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect();
    let remove = body.remove.clone().unwrap_or_default();
    let audit = super::audit_helper::begin_audit(
        &req, &state, &user, "node_patch_labels", "node", &node,
    )
    .with_input(serde_json::to_value(&*body).unwrap_or_default());
    match state
        .services
        .kube
        .patch_labels(&node, &add, &remove)
        .await
    {
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

pub async fn patch_taints(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<TaintPatch>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let node = path.into_inner();
    let add: Vec<Taint> = body.add.clone().unwrap_or_default();
    let remove: Vec<(String, Option<crate::models::mec::TaintEffect>)> = body
        .remove
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|r| (r.key, r.effect))
        .collect();
    let audit = super::audit_helper::begin_audit(
        &req, &state, &user, "node_patch_taints", "node", &node,
    )
    .with_input(serde_json::to_value(&*body).unwrap_or_default());
    match state
        .services
        .kube
        .patch_taints(&node, &add, &remove)
        .await
    {
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

pub async fn cordon(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let node = path.into_inner();
    let audit =
        super::audit_helper::begin_audit(&req, &state, &user, "node_cordon", "node", &node);
    match state.services.kube.cordon(&node, false).await {
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

pub async fn uncordon(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let node = path.into_inner();
    let audit =
        super::audit_helper::begin_audit(&req, &state, &user, "node_uncordon", "node", &node);
    match state.services.kube.cordon(&node, true).await {
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

pub async fn gpu_mode(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<GpuModeSwitchRequest>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let node = path.into_inner();
    let current = state
        .services
        .kube
        .get_node(&node)
        .await
        .ok()
        .and_then(|n| n.gpu_info.map(|g| g.mode.as_label().to_string()))
        .unwrap_or_else(|| "unknown".into());
    let target = body.mode.as_label().to_string();
    let audit = super::audit_helper::begin_audit(
        &req, &state, &user, "gpu_mode_switch", "node", &node,
    )
    .with_input(
        serde_json::json!({ "from": current.clone(), "to": target.clone(), "replicas": body.replicas }),
    );
    match state
        .services
        .kube
        .switch_gpu_mode(&node, body.mode.clone(), body.replicas)
        .await
    {
        Ok(_) => {
            audit.success(Some(serde_json::json!({"mode": target.clone()})));
            if let Some(ch) = state.channels.as_ref() {
                crate::mec::notify::MecNotify::GpuModeSwitched {
                    node: &node,
                    from: &current,
                    to: &target,
                    user: &user,
                }
                .dispatch(ch);
            }
            ok_response(serde_json::json!({"status": "switched"}))
        }
        Err(e) => {
            audit.failure(e.to_string());
            from_service_error(e)
        }
    }
}
