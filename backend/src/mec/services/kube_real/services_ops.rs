use super::client::upstream_err;
use crate::mec::services::ServiceResult;
use crate::models::mec::{LoadBalancerService, PodInfo, ServicePort};
use chrono::Utc;
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::{
    api::{Api, ListParams},
    Client,
};

pub async fn list_lb(
    client: &Client,
    namespace: Option<&str>,
) -> ServiceResult<Vec<LoadBalancerService>> {
    let api: Api<Service> = match namespace {
        Some(ns) => Api::namespaced(client.clone(), ns),
        None => Api::all(client.clone()),
    };
    let result = api
        .list(&ListParams::default())
        .await
        .map_err(upstream_err)?;
    let mut out = Vec::new();
    for svc in result.items {
        let spec = match svc.spec.clone() {
            Some(s) => s,
            None => continue,
        };
        if spec.type_.as_deref() != Some("LoadBalancer") {
            continue;
        }
        let status = svc.status.clone();
        let external_ip = status
            .as_ref()
            .and_then(|s| s.load_balancer.as_ref())
            .and_then(|lb| lb.ingress.as_ref())
            .and_then(|ing| ing.first())
            .and_then(|i| i.ip.clone())
            .unwrap_or_default();
        let ports = spec
            .ports
            .unwrap_or_default()
            .into_iter()
            .map(|p| ServicePort {
                name: p.name,
                port: p.port as u16,
                target_port: to_port(&p.target_port),
                node_port: p.node_port.map(|n| n as u16),
                protocol: p.protocol.unwrap_or_else(|| "TCP".into()),
            })
            .collect();
        let selector = spec
            .selector
            .unwrap_or_default()
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",");
        let age_seconds = svc
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|t| (Utc::now() - t.0).num_seconds().max(0) as u64)
            .unwrap_or(0);
        out.push(LoadBalancerService {
            namespace: svc.metadata.namespace.clone().unwrap_or_default(),
            name: svc.metadata.name.clone().unwrap_or_default(),
            external_ip,
            ports,
            selector_summary: selector,
            age_seconds,
        });
    }
    Ok(out)
}

pub async fn list_pods(client: &Client, namespace: &str) -> ServiceResult<Vec<PodInfo>> {
    let api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let list = api
        .list(&ListParams::default())
        .await
        .map_err(upstream_err)?;
    Ok(list.items.iter().map(convert_pod).collect())
}

fn convert_pod(p: &Pod) -> PodInfo {
    let meta = &p.metadata;
    let spec = p.spec.as_ref();
    let status = p.status.as_ref();
    let mut cpu_req = String::new();
    let mut mem_req = String::new();
    let mut gpu_req = 0u32;
    if let Some(s) = spec {
        for c in &s.containers {
            if let Some(r) = c.resources.as_ref().and_then(|r| r.requests.clone()) {
                if let Some(v) = r.get("cpu") {
                    cpu_req = v.0.clone();
                }
                if let Some(v) = r.get("memory") {
                    mem_req = v.0.clone();
                }
                if let Some(v) = r.get("nvidia.com/gpu") {
                    gpu_req += v.0.parse::<u32>().unwrap_or(0);
                }
            }
        }
    }
    let ready = status
        .and_then(|s| s.container_statuses.as_ref())
        .map(|cs| cs.iter().filter(|c| c.ready).count() as u32)
        .unwrap_or(0);
    PodInfo {
        namespace: meta.namespace.clone().unwrap_or_default(),
        name: meta.name.clone().unwrap_or_default(),
        phase: status
            .and_then(|s| s.phase.clone())
            .unwrap_or_else(|| "Unknown".into()),
        node_name: spec.and_then(|s| s.node_name.clone()),
        cpu_requests: cpu_req,
        memory_requests: mem_req,
        gpu_requests: gpu_req,
        containers: spec.map(|s| s.containers.len() as u32).unwrap_or(0),
        ready_containers: ready,
    }
}

fn to_port(
    target: &Option<k8s_openapi::apimachinery::pkg::util::intstr::IntOrString>,
) -> u16 {
    use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
    match target {
        Some(IntOrString::Int(i)) => *i as u16,
        Some(IntOrString::String(_)) | None => 0,
    }
}
