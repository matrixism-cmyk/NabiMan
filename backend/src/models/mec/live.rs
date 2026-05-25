use super::node::NodeMetrics;
use serde::{Deserialize, Serialize};

/// Cluster-wide real CPU/memory utilization (from metrics-server), aggregated
/// across nodes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClusterUsage {
    pub cpu_used_millicores: u64,
    pub cpu_capacity_millicores: u64,
    pub memory_used_bytes: u64,
    pub memory_capacity_bytes: u64,
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub node_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PodPhaseSummary {
    pub running: u32,
    pub pending: u32,
    pub failed: u32,
    pub succeeded: u32,
    pub unknown: u32,
    pub total: u32,
}

/// Real resource consumption rolled up per tenant (namespace), from pod metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantUsage {
    pub namespace: String,
    pub cpu_millicores: u64,
    pub memory_bytes: u64,
    pub pods: u32,
}

/// A Kubernetes cluster event — the live activity stream (scheduling, pulls,
/// probe failures, restarts, mounts, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterEvent {
    pub last_time: Option<String>,
    pub event_type: String, // Normal | Warning
    pub reason: String,
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub message: String,
    pub count: u32,
}

/// One-shot snapshot powering the live monitoring view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSnapshot {
    pub cluster: ClusterUsage,
    pub nodes: Vec<NodeMetrics>,
    pub pods: PodPhaseSummary,
    pub tenants: Vec<TenantUsage>,
    pub events: Vec<ClusterEvent>,
}
