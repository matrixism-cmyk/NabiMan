use super::client::upstream_err;
use crate::mec::services::{ServiceError, ServiceResult};
use crate::models::mec::{
    GpuInfo, GpuMode, Node as MecNode, NodeAllocated, NodeCapacity, NodeStatus, PodInfo, Taint,
    TaintEffect,
};
use k8s_openapi::api::core::v1::{Node as K8sNode, Pod, Taint as K8sTaint};
use kube::{
    api::{Api, ListParams, Patch, PatchParams},
    Client,
};
use serde_json::json;
use std::collections::HashMap;

pub async fn list(client: &Client) -> ServiceResult<Vec<MecNode>> {
    let api: Api<K8sNode> = Api::all(client.clone());
    let list = api
        .list(&ListParams::default())
        .await
        .map_err(upstream_err)?;
    Ok(list.items.iter().map(convert).collect())
}

pub async fn get(client: &Client, name: &str) -> ServiceResult<MecNode> {
    let api: Api<K8sNode> = Api::all(client.clone());
    let n = api
        .get(name)
        .await
        .map_err(|e| map_not_found(e, &format!("node {}", name)))?;
    Ok(convert(&n))
}

pub async fn pods_on_node(client: &Client, node: &str) -> ServiceResult<Vec<PodInfo>> {
    let api: Api<Pod> = Api::all(client.clone());
    let lp = ListParams::default().fields(&format!("spec.nodeName={}", node));
    let list = api.list(&lp).await.map_err(upstream_err)?;
    Ok(list.items.iter().map(convert_pod).collect())
}

pub async fn patch_labels(
    client: &Client,
    name: &str,
    add: &[(String, String)],
    remove: &[String],
) -> ServiceResult<()> {
    let mut patch = json!({ "metadata": { "labels": {} } });
    let labels = patch["metadata"]["labels"]
        .as_object_mut()
        .expect("object");
    for (k, v) in add {
        labels.insert(k.clone(), json!(v));
    }
    for k in remove {
        labels.insert(k.clone(), json!(null));
    }
    apply_patch(client, name, patch).await
}

pub async fn patch_taints(
    client: &Client,
    name: &str,
    add: &[Taint],
    remove: &[(String, Option<TaintEffect>)],
) -> ServiceResult<()> {
    let api: Api<K8sNode> = Api::all(client.clone());
    let node = api
        .get(name)
        .await
        .map_err(|e| map_not_found(e, &format!("node {}", name)))?;

    let current: Vec<K8sTaint> = node.spec.and_then(|s| s.taints).unwrap_or_default();
    let mut next: Vec<K8sTaint> = current
        .into_iter()
        .filter(|t| {
            !remove
                .iter()
                .any(|(k, eff)| &t.key == k && eff.as_ref().map(|e| e.as_str() == t.effect).unwrap_or(true))
        })
        .collect();
    for t in add {
        next.push(K8sTaint {
            key: t.key.clone(),
            value: t.value.clone(),
            effect: t.effect.as_str().to_string(),
            time_added: None,
        });
    }
    let patch = json!({ "spec": { "taints": next } });
    apply_patch(client, name, patch).await
}

pub async fn cordon(client: &Client, name: &str, schedulable: bool) -> ServiceResult<()> {
    let patch = json!({ "spec": { "unschedulable": !schedulable } });
    apply_patch(client, name, patch).await
}

pub async fn switch_gpu_mode(
    client: &Client,
    name: &str,
    mode: GpuMode,
    replicas: Option<u32>,
) -> ServiceResult<()> {
    let mut labels = HashMap::new();
    labels.insert("nvidia.com/gpu.deploy.gpu-feature-discovery".to_string(), "true".to_string());
    labels.insert("gpu-mode".to_string(), mode.as_label().to_string());
    if let Some(r) = replicas {
        labels.insert("nvidia.com/gpu.replicas".to_string(), r.to_string());
    }
    let add: Vec<(String, String)> = labels.into_iter().collect();
    patch_labels(client, name, &add, &[]).await
}

async fn apply_patch(client: &Client, name: &str, patch: serde_json::Value) -> ServiceResult<()> {
    let api: Api<K8sNode> = Api::all(client.clone());
    let pp = PatchParams::default();
    api.patch(name, &pp, &Patch::Merge(&patch))
        .await
        .map_err(|e| map_not_found(e, &format!("node {}", name)))?;
    Ok(())
}

pub(super) fn map_not_found(e: kube::Error, what: &str) -> ServiceError {
    match &e {
        kube::Error::Api(ae) if ae.code == 404 => ServiceError::NotFound(what.into()),
        _ => upstream_err(e),
    }
}

fn convert(n: &K8sNode) -> MecNode {
    let meta = n.metadata.clone();
    let name = meta.name.clone().unwrap_or_default();
    let labels: HashMap<String, String> =
        meta.labels.clone().unwrap_or_default().into_iter().collect();
    let spec = n.spec.clone().unwrap_or_default();
    let status = n.status.clone().unwrap_or_default();

    let status_val = status
        .conditions
        .as_ref()
        .and_then(|conds| {
            conds.iter().find(|c| c.type_ == "Ready").map(|c| {
                if c.status == "True" {
                    NodeStatus::Ready
                } else {
                    NodeStatus::NotReady
                }
            })
        })
        .unwrap_or(NodeStatus::Unknown);

    let capacity = to_capacity(&status.capacity.clone().unwrap_or_default());
    let allocatable = to_capacity(&status.allocatable.clone().unwrap_or_default());
    let node_info = status.node_info;
    let gpu_count = capacity.gpu.unwrap_or(0);
    let gpu_info = if gpu_count > 0 {
        Some(GpuInfo {
            model: labels
                .get("nvidia.com/gpu.product")
                .cloned()
                .unwrap_or_else(|| labels.get("gpu").cloned().unwrap_or_else(|| "GPU".into())),
            count: gpu_count,
            total_slots: gpu_count,
            mode: GpuMode::Container,
            sharing_strategy: labels.get("gpu-mode").cloned(),
            replicas: labels
                .get("nvidia.com/gpu.replicas")
                .and_then(|v| v.parse::<u32>().ok()),
        })
    } else {
        None
    };

    let taints = spec
        .taints
        .unwrap_or_default()
        .into_iter()
        .map(|t| Taint {
            key: t.key,
            value: t.value,
            effect: TaintEffect::from_str(&t.effect).unwrap_or(TaintEffect::NoSchedule),
        })
        .collect();
    let roles = meta
        .labels
        .clone()
        .unwrap_or_default()
        .keys()
        .filter_map(|k| k.strip_prefix("node-role.kubernetes.io/").map(String::from))
        .collect();
    let current_tenant = labels.get("tenant").cloned();

    MecNode {
        name,
        roles,
        status: status_val,
        capacity,
        allocatable,
        allocated: NodeAllocated::default(),
        labels,
        taints,
        architecture: node_info
            .as_ref()
            .map(|i| i.architecture.clone())
            .unwrap_or_default(),
        os_image: node_info
            .as_ref()
            .map(|i| i.os_image.clone())
            .unwrap_or_default(),
        kernel_version: node_info
            .as_ref()
            .map(|i| i.kernel_version.clone())
            .unwrap_or_default(),
        cuda_driver: None,
        gpu_info,
        current_tenant,
    }
}

fn to_capacity(
    map: &std::collections::BTreeMap<String, k8s_openapi::apimachinery::pkg::api::resource::Quantity>,
) -> NodeCapacity {
    NodeCapacity {
        cpu: map.get("cpu").map(|q| q.0.clone()).unwrap_or_default(),
        memory: map.get("memory").map(|q| q.0.clone()).unwrap_or_default(),
        pods: map
            .get("pods")
            .and_then(|q| q.0.parse().ok())
            .unwrap_or(0),
        gpu: map.get("nvidia.com/gpu").and_then(|q| q.0.parse().ok()),
        storage: map.get("ephemeral-storage").map(|q| q.0.clone()),
    }
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
