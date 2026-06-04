use super::kube_service::NamespaceInfo;
use crate::models::mec::{
    ClusterEvent, GpuInfo, GpuMode, LoadBalancerService, Node, NodeAllocated, NodeCapacity,
    NodeMetrics, NodeStatus, PodPhaseSummary, ServicePort, Taint, TaintEffect, TenantUsage,
};
use chrono::Utc;
use std::collections::HashMap;

const GIB: u64 = 1_073_741_824;

pub fn sample_node_metrics() -> Vec<NodeMetrics> {
    let mk = |name: &str, cpu_u: u64, cpu_c: u64, mem_u: u64, mem_c: u64| NodeMetrics {
        name: name.into(),
        status: "Ready".into(),
        cpu_usage_millicores: cpu_u,
        cpu_capacity_millicores: cpu_c,
        memory_usage_bytes: mem_u * GIB,
        memory_capacity_bytes: mem_c * GIB,
        cpu_usage_percent: (cpu_u as f32 / cpu_c as f32) * 100.0,
        memory_usage_percent: (mem_u as f32 / mem_c as f32) * 100.0,
        gpu_usage_percent: None,
    };
    vec![
        mk("mec-cp01", 3200, 32000, 18, 64),
        mk("mec-wn01", 24500, 32000, 96, 128),
        mk("mec-wn02", 12100, 64000, 40, 256),
    ]
}

pub fn sample_tenant_usage() -> Vec<TenantUsage> {
    let mk = |ns: &str, cpu: u64, mem: u64, pods: u32| TenantUsage {
        namespace: ns.into(),
        cpu_millicores: cpu,
        memory_bytes: mem * GIB,
        pods,
    };
    vec![
        mk("maninblock-poc", 8400, 36, 12),
        mk("witches-poc", 5200, 22, 8),
        mk("funit-poc", 3100, 14, 5),
        mk("monitoring", 1800, 9, 14),
    ]
}

pub fn sample_pod_phases() -> PodPhaseSummary {
    PodPhaseSummary { running: 279, pending: 0, failed: 7, succeeded: 27, unknown: 0, total: 313 }
}

pub fn sample_events(limit: u32) -> Vec<ClusterEvent> {
    let now = Utc::now();
    let mk = |secs: i64, t: &str, reason: &str, kind: &str, name: &str, ns: &str, msg: &str| {
        ClusterEvent {
            last_time: Some((now - chrono::Duration::seconds(secs)).to_rfc3339()),
            event_type: t.into(),
            reason: reason.into(),
            kind: kind.into(),
            name: name.into(),
            namespace: Some(ns.into()),
            message: msg.into(),
            count: 1,
        }
    };
    let all = vec![
        mk(8, "Warning", "BackOff", "Pod", "loki-0", "monitoring", "Back-off restarting failed container"),
        mk(35, "Normal", "Pulled", "Pod", "triton-a40-xyz", "maninblock-poc", "Container image already present"),
        mk(60, "Warning", "Unhealthy", "Pod", "istio-cni-node-7npdd", "istio-system", "Readiness probe failed"),
        mk(95, "Normal", "Scheduled", "Pod", "gpu-job-12", "witches-poc", "Successfully assigned to mec-wn01"),
        mk(140, "Normal", "Started", "Pod", "api-gw-5f9", "funit-poc", "Started container api"),
        mk(210, "Warning", "FailedMount", "Pod", "triton-a40-dwwfh", "maninblock-poc", "MountVolume.SetUp failed for pvc"),
    ];
    all.into_iter().take(limit as usize).collect()
}

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
