use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    pub roles: Vec<String>,
    pub status: NodeStatus,

    pub capacity: NodeCapacity,
    pub allocatable: NodeCapacity,
    pub allocated: NodeAllocated,

    pub labels: HashMap<String, String>,
    pub taints: Vec<Taint>,

    pub architecture: String,
    pub os_image: String,
    pub kernel_version: String,
    pub cuda_driver: Option<String>,

    pub gpu_info: Option<GpuInfo>,
    pub current_tenant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum NodeStatus {
    Ready,
    NotReady,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeCapacity {
    pub cpu: String,
    pub memory: String,
    pub pods: u32,
    pub gpu: Option<u32>,
    pub storage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeAllocated {
    pub cpu_requests: String,
    pub cpu_limits: String,
    pub memory_requests: String,
    pub memory_limits: String,
    pub gpu_requests: u32,
    pub pod_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Taint {
    pub key: String,
    pub value: Option<String>,
    pub effect: TaintEffect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaintEffect {
    NoSchedule,
    PreferNoSchedule,
    NoExecute,
}

impl TaintEffect {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoSchedule => "NoSchedule",
            Self::PreferNoSchedule => "PreferNoSchedule",
            Self::NoExecute => "NoExecute",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "NoSchedule" => Some(Self::NoSchedule),
            "PreferNoSchedule" => Some(Self::PreferNoSchedule),
            "NoExecute" => Some(Self::NoExecute),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub model: String,
    pub count: u32,
    pub total_slots: u32,
    pub mode: GpuMode,
    pub sharing_strategy: Option<String>,
    pub replicas: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum GpuMode {
    Container,
    VmPassthrough,
    Mig,
    TimeSlicing,
}

impl GpuMode {
    pub fn as_label(&self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::VmPassthrough => "vm-passthrough",
            Self::Mig => "mig",
            Self::TimeSlicing => "time-slicing",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodInfo {
    pub namespace: String,
    pub name: String,
    pub phase: String,
    pub node_name: Option<String>,
    pub cpu_requests: String,
    pub memory_requests: String,
    pub gpu_requests: u32,
    pub containers: u32,
    pub ready_containers: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub name: String,
    pub cpu_usage_millicores: u64,
    pub cpu_capacity_millicores: u64,
    pub memory_usage_bytes: u64,
    pub memory_capacity_bytes: u64,
    pub cpu_usage_percent: f32,
    pub memory_usage_percent: f32,
    pub gpu_usage_percent: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelPatch {
    pub add: Option<HashMap<String, String>>,
    pub remove: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaintPatch {
    pub add: Option<Vec<Taint>>,
    pub remove: Option<Vec<TaintRef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaintRef {
    pub key: String,
    pub effect: Option<TaintEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuModeSwitchRequest {
    pub mode: GpuMode,
    #[serde(default)]
    pub replicas: Option<u32>,
    #[serde(default)]
    pub force: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taint_effect_roundtrip() {
        let e = TaintEffect::NoSchedule;
        assert_eq!(TaintEffect::from_str(e.as_str()), Some(e));
    }

    #[test]
    fn gpu_mode_label() {
        assert_eq!(GpuMode::VmPassthrough.as_label(), "vm-passthrough");
    }
}
