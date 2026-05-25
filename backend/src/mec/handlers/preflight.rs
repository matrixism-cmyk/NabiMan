use super::util::{from_service_error, ok_response};
use crate::mec::MecState;
use actix_web::{web, HttpResponse};
use serde::Serialize;

#[derive(Serialize)]
pub struct TenantDeletePreflight {
    pub tenant_id: String,
    pub namespace_exists: bool,
    pub running_pods: u32,
    pub lb_services: u32,
    pub has_starter_kit: bool,
    pub rancher_project_exists: bool,
    pub dedicated_node: Option<String>,
    pub warnings: Vec<String>,
    pub confirm_token: String,
}

pub async fn tenant_delete(
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

    let namespaces = state
        .services
        .kube
        .list_namespaces()
        .await
        .unwrap_or_default();
    let namespace_exists = namespaces.iter().any(|n| n.name == tenant.namespace);

    let pods = state
        .services
        .kube
        .count_pods(&tenant.namespace)
        .await
        .unwrap_or(0);

    let services = state
        .services
        .kube
        .list_lb_services(Some(&tenant.namespace))
        .await
        .unwrap_or_default();

    let has_starter_kit = tenant.starter_kit.is_some();

    let rancher_project_exists = if let Some(pid) = &tenant.rancher_project_id {
        state.services.rancher.get_project(pid).await.is_ok()
    } else {
        false
    };

    let mut warnings = Vec::new();
    if pods > 0 {
        warnings.push(format!(
            "실행 중인 Pod가 {}개 있습니다. 삭제 시 강제 종료됩니다.",
            pods
        ));
    }
    if !services.is_empty() {
        warnings.push(format!(
            "LoadBalancer Service {}개가 외부 IP를 사용 중입니다. 삭제 시 외부 접근이 끊깁니다.",
            services.len()
        ));
    }
    if has_starter_kit {
        warnings.push("Starter Kit (Ubuntu SSH + VS Code) 이 배포되어 있습니다.".into());
    }
    if tenant.allocation.node.is_some() {
        warnings.push(format!(
            "노드 '{}'에 단독 할당되어 있습니다. Taint는 삭제 시 제거되지 않으므로 수동 정리가 필요할 수 있습니다.",
            tenant.allocation.node.as_deref().unwrap_or("-")
        ));
    }
    if !namespace_exists {
        warnings.push(
            "K8s에 네임스페이스가 존재하지 않습니다. DB 레코드만 제거됩니다."
                .into(),
        );
    }

    ok_response(TenantDeletePreflight {
        tenant_id: tenant.id.clone(),
        namespace_exists,
        running_pods: pods,
        lb_services: services.len() as u32,
        has_starter_kit,
        rancher_project_exists,
        dedicated_node: tenant.allocation.node.clone(),
        warnings,
        confirm_token: tenant.id,
    })
}
