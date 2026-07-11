use super::util::{from_service_error, list_response};
use crate::mec::MecState;
use actix_web::{web, HttpResponse};
use crate::models::mec::LbIpPool;

pub async fn lb_pools(state: web::Data<MecState>) -> HttpResponse {
    // MetalLB IPAddressPool CRD 조회는 Phase 2 kube-rs 통합에서 구현.
    // 현재는 서비스 현황으로부터 추정치를 제공한다.
    let services = match state.services.kube.list_lb_services(None).await {
        Ok(s) => s,
        Err(e) => return from_service_error(e),
    };
    let used: std::collections::BTreeSet<String> =
        services.iter().map(|s| s.external_ip.clone()).collect();
    let pools = vec![
        LbIpPool {
            name: "tenant".into(),
            address_ranges: vec!["172.20.26.151-172.20.26.200".into()],
            total_ips: 50,
            used_ips: used
                .iter()
                .filter(|ip| prefix_matches(ip, "172.20.26.1") || prefix_matches(ip, "172.20.26.2"))
                .count() as u32,
            auto_assign: true,
        },
        LbIpPool {
            name: "reserved".into(),
            address_ranges: vec!["172.20.26.231-172.20.26.238".into()],
            total_ips: 8,
            used_ips: used
                .iter()
                .filter(|ip| prefix_matches(ip, "172.20.26.23"))
                .count() as u32,
            auto_assign: false,
        },
    ];
    list_response(pools)
}

fn prefix_matches(ip: &str, prefix: &str) -> bool {
    ip.starts_with(prefix)
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
    let user = super::util::current_user(&req);
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
    let user = super::util::current_user(&req);
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
