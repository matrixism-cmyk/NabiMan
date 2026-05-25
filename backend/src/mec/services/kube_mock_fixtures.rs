use super::kube_service::NamespaceInfo;
use crate::models::mec::{
    GpuInfo, GpuMode, LoadBalancerService, Node, NodeAllocated, NodeCapacity, NodeStatus,
    ServicePort, Taint, TaintEffect,
};
use chrono::Utc;
use std::collections::HashMap;

pub fn sample_nodes() -> Vec<Node> {
    vec![
        make_node("mec-cp01", "control-plane", 32, "125Gi", None, None),
        make_node("mec-cp02", "worker", 32, "125Gi", Some(("T4", 4, 12)), Some("funit-poc")),
        make_node("mec-wn01", "worker", 32, "125Gi", Some(("T4", 4, 12)), Some("maninblock-poc")),
        make_node("mec-wn02", "worker", 64, "503Gi", Some(("A40", 4, 16)), Some("witches-poc")),
        make_node("mec-wn04", "worker", 72, "555Gi", Some(("GH200", 7, 7)), Some("ygram-poc")),
    ]
}

fn make_node(
    name: &str,
    role: &str,
    cpu: u32,
    mem: &str,
    gpu: Option<(&str, u32, u32)>,
    tenant: Option<&str>,
) -> Node {
    let mut labels = HashMap::new();
    if let Some((model, _, _)) = gpu {
        labels.insert("gpu".into(), model.to_lowercase());
    }
    if let Some(t) = tenant {
        labels.insert("tenant".into(), t.into());
    }
    let taints = tenant
        .map(|t| vec![Taint {
            key: "tenant".into(),
            value: Some(t.into()),
            effect: TaintEffect::NoSchedule,
        }])
        .unwrap_or_default();
    let gpu_info = gpu.map(|(model, count, slots)| GpuInfo {
        model: model.into(),
        count,
        total_slots: slots,
        mode: GpuMode::Container,
        sharing_strategy: Some("time-slicing".into()),
        replicas: Some(slots / count.max(1)),
    });
    Node {
        name: name.into(),
        roles: vec![role.into()],
        status: NodeStatus::Ready,
        capacity: NodeCapacity {
            cpu: cpu.to_string(),
            memory: mem.into(),
            pods: 110,
            gpu: gpu.map(|(_, _, s)| s),
            storage: None,
        },
        allocatable: NodeCapacity {
            cpu: cpu.to_string(),
            memory: mem.into(),
            pods: 110,
            gpu: gpu.map(|(_, _, s)| s),
            storage: None,
        },
        allocated: NodeAllocated::default(),
        labels,
        taints,
        architecture: if name.starts_with("mec-wn0") {
            "arm64".into()
        } else {
            "amd64".into()
        },
        os_image: "Ubuntu 22.04".into(),
        kernel_version: "6.8.0".into(),
        cuda_driver: gpu.map(|_| "590.48.01".into()),
        gpu_info,
        current_tenant: tenant.map(String::from),
    }
}

pub fn sample_namespaces() -> Vec<NamespaceInfo> {
    ["default", "kube-system", "rancher", "nabiman-system"]
        .iter()
        .map(|n| NamespaceInfo {
            name: (*n).into(),
            phase: "Active".into(),
            labels: HashMap::new(),
            age_seconds: 86400,
        })
        .collect()
}

pub fn sample_services() -> Vec<LoadBalancerService> {
    vec![LoadBalancerService {
        namespace: "ygram-poc".into(),
        name: "ubuntu-ssh-svc".into(),
        external_ip: "172.20.26.231".into(),
        ports: vec![ServicePort {
            name: Some("ssh".into()),
            port: 22,
            target_port: 22,
            node_port: None,
            protocol: "TCP".into(),
        }],
        selector_summary: "app=ubuntu-ssh".into(),
        age_seconds: (Utc::now().timestamp() % 86400) as u64,
    }]
}
