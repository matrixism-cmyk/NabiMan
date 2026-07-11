use super::{ServiceError, ServiceResult};
use crate::models::mec::{
    ClusterEvent, GpuMode, LoadBalancerService, Node, NodeMetrics, PodInfo, PodPhaseSummary,
    QuotaUsage, ResourceQuota, Taint, TaintEffect, TenantUsage,
};
use async_trait::async_trait;
use std::sync::Arc;

pub type KubeServiceArc = Arc<dyn KubeService + Send + Sync>;

#[async_trait]
pub trait KubeService: Send + Sync {
    async fn list_nodes(&self) -> ServiceResult<Vec<Node>>;
    async fn get_node(&self, name: &str) -> ServiceResult<Node>;
    async fn list_pods_on_node(&self, node: &str) -> ServiceResult<Vec<PodInfo>>;
    async fn patch_labels(
        &self,
        node: &str,
        add: &[(String, String)],
        remove: &[String],
    ) -> ServiceResult<()>;
    async fn patch_taints(
        &self,
        node: &str,
        add: &[Taint],
        remove: &[(String, Option<TaintEffect>)],
    ) -> ServiceResult<()>;
    async fn cordon(&self, node: &str, schedulable: bool) -> ServiceResult<()>;
    async fn switch_gpu_mode(
        &self,
        node: &str,
        mode: GpuMode,
        replicas: Option<u32>,
    ) -> ServiceResult<()>;

    async fn list_namespaces(&self) -> ServiceResult<Vec<NamespaceInfo>>;
    async fn create_namespace(
        &self,
        name: &str,
        labels: &[(String, String)],
    ) -> ServiceResult<()>;
    async fn delete_namespace(&self, name: &str) -> ServiceResult<()>;
    async fn apply_resource_quota(
        &self,
        namespace: &str,
        quota: &ResourceQuota,
    ) -> ServiceResult<()>;
    async fn get_quota_usage(&self, namespace: &str) -> ServiceResult<QuotaUsage>;
    async fn apply_network_policy(&self, namespace: &str, name: &str, spec: &str)
        -> ServiceResult<()>;
    async fn apply_limit_range(&self, namespace: &str, spec: &str) -> ServiceResult<()>;
    async fn apply_rbac(&self, namespace: &str, user: &str) -> ServiceResult<()>;
    async fn apply_deployment(&self, namespace: &str, spec: &str) -> ServiceResult<()>;
    async fn delete_deployment(&self, namespace: &str, name: &str) -> ServiceResult<()>;
    async fn apply_service(&self, namespace: &str, spec: &str) -> ServiceResult<()>;
    async fn delete_service(&self, namespace: &str, name: &str) -> ServiceResult<()>;

    async fn list_lb_services(&self, namespace: Option<&str>)
        -> ServiceResult<Vec<LoadBalancerService>>;

    async fn list_pods(&self, namespace: &str) -> ServiceResult<Vec<PodInfo>>;
    async fn count_pods(&self, namespace: &str) -> ServiceResult<u32>;

    async fn list_ingresses(&self, namespace: Option<&str>) -> ServiceResult<Vec<IngressRow>>;
    async fn apply_ingress(&self, namespace: &str, spec: &str) -> ServiceResult<()>;
    async fn delete_ingress(&self, namespace: &str, name: &str) -> ServiceResult<()>;

    async fn list_pvcs(&self, namespace: Option<&str>) -> ServiceResult<Vec<PvcRow>>;

    /// Real CPU/memory utilization per node (metrics-server + node capacity).
    async fn node_metrics(&self) -> ServiceResult<Vec<NodeMetrics>>;
    /// Real CPU/memory consumption rolled up per namespace (pod metrics).
    async fn tenant_usage(&self) -> ServiceResult<Vec<TenantUsage>>;
    /// Count of pods by phase across all namespaces.
    async fn pod_phase_summary(&self) -> ServiceResult<PodPhaseSummary>;
    /// Recent cluster events (live activity), newest first.
    async fn list_events(&self, limit: u32) -> ServiceResult<Vec<ClusterEvent>>;

    async fn health_check(&self) -> ServiceResult<KubeHealth>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IngressRow {
    pub namespace: String,
    pub name: String,
    pub class: Option<String>,
    pub hosts: Vec<String>,
    pub paths: Vec<IngressPathRow>,
    pub age_seconds: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IngressPathRow {
    pub host: Option<String>,
    pub path: String,
    pub path_type: String,
    pub backend_service: String,
    pub backend_port: u16,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PvcRow {
    pub namespace: String,
    pub name: String,
    pub phase: String,
    pub storage_request: String,
    pub storage_class: Option<String>,
    pub volume_name: Option<String>,
    pub access_modes: Vec<String>,
    pub age_seconds: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NamespaceInfo {
    pub name: String,
    pub phase: String,
    pub labels: std::collections::HashMap<String, String>,
    pub age_seconds: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct KubeHealth {
    pub connected: bool,
    pub api_version: Option<String>,
    pub endpoint: String,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[allow(dead_code)]
pub fn unavailable<T>(msg: &str) -> ServiceResult<T> {
    Err(ServiceError::Unavailable(msg.to_string()))
}
