use super::util::{created, current_user, from_service_error, list_response};
use crate::mec::MecState;
use crate::models::mec::{
    AllocationType, NodeAllocation, ResourceQuota, Tenant, TenantStatus,
};
use actix_web::{web, HttpRequest, HttpResponse};
use chrono::Utc;
use serde::Serialize;

/// Kubernetes namespaces that look like tenant namespaces but are not yet
/// tracked by NabiMan's tenant_profiles DB. Heuristics:
///  - namespace name ends with `-poc` or has `tenant` label, OR
///  - has `managed-by=nabiman` label but missing from DB
#[derive(Serialize)]
pub struct DiscoveredTenant {
    pub id: String,
    pub namespace: String,
    pub labels: std::collections::HashMap<String, String>,
    pub already_managed: bool,
    pub suspected_tenant: bool,
}

pub async fn discover(state: web::Data<MecState>) -> HttpResponse {
    let namespaces = match state.services.kube.list_namespaces().await {
        Ok(n) => n,
        Err(e) => return from_service_error(e),
    };
    let known: std::collections::HashSet<String> = state
        .tenants
        .list()
        .unwrap_or_default()
        .into_iter()
        .map(|t| t.id)
        .collect();

    let mut out = Vec::new();
    for ns in namespaces {
        if SYSTEM_NAMESPACES.contains(&ns.name.as_str()) {
            continue;
        }
        let already = known.contains(&ns.name);
        let suspected = is_suspected_tenant(&ns.name, &ns.labels);
        if !already && !suspected {
            continue;
        }
        out.push(DiscoveredTenant {
            id: ns.name.clone(),
            namespace: ns.name,
            labels: ns.labels,
            already_managed: already,
            suspected_tenant: suspected,
        });
    }
    list_response(out)
}

const SYSTEM_NAMESPACES: &[&str] = &[
    "default",
    "kube-system",
    "kube-public",
    "kube-node-lease",
    "ingress-nginx",
    "cattle-system",
    "cattle-fleet-system",
    "cattle-impersonation-system",
    "cattle-fleet-local-system",
    "cattle-provisioning-capi-system",
    "cert-manager",
    "local-path-storage",
    "longhorn-system",
    "metallb-system",
    "nabiman-system",
    "harbor",
    "gpu-operator",
    "nvidia-gpu-operator",
];

fn is_suspected_tenant(name: &str, labels: &std::collections::HashMap<String, String>) -> bool {
    labels.get("managed-by").map(|v| v == "nabiman").unwrap_or(false)
        || labels.contains_key("tenant")
        || name.ends_with("-poc")
        || name.ends_with("-prod")
        || name.ends_with("-dev")
}

pub async fn import_tenant(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = current_user(&req);
    let id = path.into_inner();

    if state.tenants.get(&id).ok().flatten().is_some() {
        return from_service_error(
            crate::mec::services::ServiceError::Conflict(format!(
                "tenant {} already imported",
                id
            )),
        );
    }

    // Verify namespace exists.
    let namespaces = match state.services.kube.list_namespaces().await {
        Ok(n) => n,
        Err(e) => return from_service_error(e),
    };
    let ns = match namespaces.iter().find(|n| n.name == id) {
        Some(n) => n,
        None => {
            return from_service_error(
                crate::mec::services::ServiceError::NotFound(format!("namespace {}", id)),
            );
        }
    };

    // Try to find matching Rancher project (best-effort).
    let rancher_project = state
        .services
        .rancher
        .list_projects("local")
        .await
        .ok()
        .and_then(|ps| ps.into_iter().find(|p| p.name == id));

    let now = Utc::now();
    let tenant = Tenant {
        id: id.clone(),
        display_name: ns
            .labels
            .get("display-name")
            .cloned()
            .unwrap_or_else(|| id.clone()),
        task_name: ns.labels.get("task-name").cloned(),
        contact_email: ns.labels.get("contact-email").cloned(),
        namespace: id.clone(),
        rancher_project_id: rancher_project.as_ref().map(|p| p.id.clone()),
        rancher_user_id: None,
        allocation: NodeAllocation {
            alloc_type: AllocationType::Shared,
            node: ns.labels.get("node").cloned(),
            gpu_label: ns.labels.get("gpu").cloned(),
            tier: ns.labels.get("tier").cloned(),
        },
        quota: ResourceQuota::standard(),
        starter_kit: None,
        created_at: now,
        updated_at: now,
        status: TenantStatus::Active,
    };
    state
        .tenants
        .upsert(&tenant, &user)
        .ok();
    let audit = state.audit.begin(&user, "tenant_import", "tenant", &id);
    audit.success(Some(serde_json::json!({ "imported_namespace": id })));
    created(tenant)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn detects_poc_suffix() {
        let labels = HashMap::new();
        assert!(is_suspected_tenant("ygram-poc", &labels));
    }

    #[test]
    fn detects_managed_by_label() {
        let mut labels = HashMap::new();
        labels.insert("managed-by".into(), "nabiman".into());
        assert!(is_suspected_tenant("foo", &labels));
    }

    #[test]
    fn ignores_unrelated() {
        let labels = HashMap::new();
        assert!(!is_suspected_tenant("myapp", &labels));
    }
}
