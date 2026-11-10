use super::util::{from_service_error, list_response};
use crate::mec::MecState;
use crate::models::mec::LbIpPool;
use actix_web::{web, HttpResponse};

/// The MetalLB address pools this install has (`NABIMAN_MEC_LB_POOLS`), with how
/// many of each pool's addresses LoadBalancer services hold. None set: an empty list.
pub async fn lb_pools(state: web::Data<MecState>) -> HttpResponse {
    // Reading MetalLB's IPAddressPool CRDs is for the kube-rs integration; until
    // then the pools are configured and their use counted from the services.
    let configured = crate::install_config::settings::mec_lb_pools();
    if configured.is_empty() {
        return list_response(Vec::<LbIpPool>::new());
    }
    let services = match state.services.kube.list_lb_services(None).await {
        Ok(s) => s,
        Err(e) => return from_service_error(e),
    };
    let used: std::collections::BTreeSet<String> = services.iter().map(|s| s.external_ip.clone()).collect();
    list_response(pools(&configured, &used))
}

fn pools(configured: &[crate::install_config::settings::LbPool], used: &std::collections::BTreeSet<String>) -> Vec<LbIpPool> {
    configured
        .iter()
        .map(|p| LbIpPool {
            name: p.name.clone(),
            address_ranges: p.ranges.iter().map(|(a, b)| if a == b { a.to_string() } else { format!("{a}-{b}") }).collect(),
            total_ips: p.size(),
            used_ips: used.iter().filter(|ip| p.contains(ip)).count() as u32,
            auto_assign: p.auto_assign,
        })
        .collect()
}

pub async fn services(state: web::Data<MecState>) -> HttpResponse {
    let ns = None;
    match state.services.kube.list_lb_services(ns).await {
        Ok(s) => list_response(s),
        Err(e) => from_service_error(e),
    }
}

pub async fn ingresses(state: web::Data<MecState>) -> HttpResponse {
    match state.services.kube.list_ingresses(None).await {
        Ok(v) => list_response(v),
        Err(e) => from_service_error(e),
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct CreateIngressRequest {
    pub namespace: String,
    pub name: String,
    pub host: String,
    #[serde(default = "default_ingress_class")]
    pub class: String,
    pub backend_service: String,
    pub backend_port: u16,
    #[serde(default = "default_path")]
    pub path: String,
    #[serde(default = "default_path_type")]
    pub path_type: String,
    #[serde(default)]
    pub tls_secret_name: Option<String>,
}

fn default_ingress_class() -> String {
    "nginx".into()
}

fn default_path() -> String {
    "/".into()
}

fn default_path_type() -> String {
    "Prefix".into()
}

pub async fn create_ingress(
    req: actix_web::HttpRequest,
    body: web::Json<CreateIngressRequest>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = match super::util::current_user(&req) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let audit = super::audit_helper::begin_audit(
        &req,
        &state,
        &user,
        "ingress_create",
        "ingress",
        &format!("{}/{}", body.namespace, body.name),
    )
    .with_input(serde_json::to_value(&*body).unwrap_or_default());
    let tls = body
        .tls_secret_name
        .as_ref()
        .map(|name| {
            serde_json::json!([{
                "hosts": [body.host],
                "secretName": name,
            }])
        })
        .unwrap_or(serde_json::json!([]));
    let spec = serde_json::json!({
        "apiVersion": "networking.k8s.io/v1",
        "kind": "Ingress",
        "metadata": {
            "name": body.name,
            "namespace": body.namespace,
            "labels": { "managed-by": "nabiman" }
        },
        "spec": {
            "ingressClassName": body.class,
            "rules": [{
                "host": body.host,
                "http": {
                    "paths": [{
                        "path": body.path,
                        "pathType": body.path_type,
                        "backend": {
                            "service": {
                                "name": body.backend_service,
                                "port": { "number": body.backend_port }
                            }
                        }
                    }]
                }
            }],
            "tls": tls
        }
    });
    match state
        .services
        .kube
        .apply_ingress(&body.namespace, &spec.to_string())
        .await
    {
        Ok(_) => {
            audit.success(None);
            super::util::created(serde_json::json!({
                "namespace": body.namespace,
                "name": body.name,
            }))
        }
        Err(e) => {
            audit.failure(e.to_string());
            from_service_error(e)
        }
    }
}

pub async fn delete_ingress(
    req: actix_web::HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<MecState>,
) -> HttpResponse {
    let user = match super::util::current_user(&req) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let (namespace, name) = path.into_inner();
    if let Err(e) = crate::mec::safety::require_confirm_header(&req, &name) {
        return from_service_error(e);
    }
    let audit = super::audit_helper::begin_audit(
        &req,
        &state,
        &user,
        "ingress_delete",
        "ingress",
        &format!("{}/{}", namespace, name),
    );
    match state.services.kube.delete_ingress(&namespace, &name).await {
        Ok(_) => {
            audit.success(None);
            super::util::no_content()
        }
        Err(e) => {
            audit.failure(e.to_string());
            from_service_error(e)
        }
    }
}

#[cfg(test)]
mod pool_tests {
    #[test]
    fn pools_count_the_addresses_services_hold() {
        let get = |n: &str| (n == "NABIMAN_MEC_LB_POOLS").then(|| "tenant=10.1.0.10-10.1.0.19 auto,one=10.1.0.30".to_string());
        let configured = crate::install_config::settings::mec_lb_pools_in(&get);
        let used = ["10.1.0.10", "10.1.0.19", "10.1.0.20", "10.1.0.30", ""].iter().map(|s| s.to_string()).collect();
        let p = super::pools(&configured, &used);
        assert_eq!((p[0].address_ranges.clone(), p[0].total_ips, p[0].used_ips, p[0].auto_assign), (vec!["10.1.0.10-10.1.0.19".to_string()], 10, 2, true));
        assert_eq!((p[1].address_ranges.clone(), p[1].total_ips, p[1].used_ips, p[1].auto_assign), (vec!["10.1.0.30".to_string()], 1, 1, false));
    }
}
