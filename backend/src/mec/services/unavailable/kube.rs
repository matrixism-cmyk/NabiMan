use crate::mec::services::ServiceResult;
use crate::models::mec::{
    ClusterEvent, GpuMode, LoadBalancerService, Node, NodeMetrics, PodInfo, PodPhaseSummary,
    QuotaUsage, ResourceQuota, Taint, TaintEffect, TenantUsage,
};
use async_trait::async_trait;

use super::UnavailableService;
use crate::mec::services::kube_service::*;

#[async_trait]
impl KubeService for UnavailableService {
    async fn list_nodes(&self) -> ServiceResult<Vec<Node>> {
        self.reject()
    }

    async fn get_node(&self, _name: &str) -> ServiceResult<Node> {
        self.reject()
    }

    async fn list_pods_on_node(&self, _node: &str) -> ServiceResult<Vec<PodInfo>> {
        self.reject()
    }

    async fn patch_labels(
        &self,
        _node: &str,
        _add: &[(String, String)],
        _remove: &[String],
    ) -> ServiceResult<()> {
        self.reject()
    }

    async fn patch_taints(
        &self,
        _node: &str,
        _add: &[Taint],
        _remove: &[(String, Option<TaintEffect>)],
    ) -> ServiceResult<()> {
        self.reject()
    }

    async fn cordon(&self, _node: &str, _schedulable: bool) -> ServiceResult<()> {
        self.reject()
    }

    async fn switch_gpu_mode(
        &self,
        _node: &str,
        _mode: GpuMode,
        _replicas: Option<u32>,
    ) -> ServiceResult<()> {
        self.reject()
    }

    async fn list_namespaces(&self) -> ServiceResult<Vec<NamespaceInfo>> {
        self.reject()
    }

    async fn create_namespace(
        &self,
        _name: &str,
        _labels: &[(String, String)],
    ) -> ServiceResult<()> {
        self.reject()
    }

    async fn delete_namespace(&self, _name: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn apply_resource_quota(
        &self,
        _namespace: &str,
        _quota: &ResourceQuota,
    ) -> ServiceResult<()> {
        self.reject()
    }

    async fn get_quota_usage(&self, _namespace: &str) -> ServiceResult<QuotaUsage> {
        self.reject()
    }

    async fn apply_network_policy(
        &self,
        _namespace: &str,
        _name: &str,
        _spec: &str,
    ) -> ServiceResult<()> {
        self.reject()
    }

    async fn apply_limit_range(&self, _namespace: &str, _spec: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn apply_rbac(&self, _namespace: &str, _user: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn apply_deployment(&self, _namespace: &str, _spec: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn delete_deployment(&self, _namespace: &str, _name: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn apply_service(&self, _namespace: &str, _spec: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn delete_service(&self, _namespace: &str, _name: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn list_lb_services(
        &self,
        _namespace: Option<&str>,
    ) -> ServiceResult<Vec<LoadBalancerService>> {
        self.reject()
    }

    async fn list_pods(&self, _namespace: &str) -> ServiceResult<Vec<PodInfo>> {
        self.reject()
    }

    async fn count_pods(&self, _namespace: &str) -> ServiceResult<u32> {
        self.reject()
    }

    async fn list_ingresses(&self, _namespace: Option<&str>) -> ServiceResult<Vec<IngressRow>> {
        self.reject()
    }

    async fn apply_ingress(&self, _namespace: &str, _spec: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn delete_ingress(&self, _namespace: &str, _name: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn list_pvcs(&self, _namespace: Option<&str>) -> ServiceResult<Vec<PvcRow>> {
        self.reject()
    }

    async fn node_metrics(&self) -> ServiceResult<Vec<NodeMetrics>> {
        self.reject()
    }

    async fn tenant_usage(&self) -> ServiceResult<Vec<TenantUsage>> {
        self.reject()
    }

    async fn pod_phase_summary(&self) -> ServiceResult<PodPhaseSummary> {
        self.reject()
    }

    async fn list_events(&self, _limit: u32) -> ServiceResult<Vec<ClusterEvent>> {
        self.reject()
    }

    async fn health_check(&self) -> ServiceResult<KubeHealth> {
        self.reject()
    }
}
