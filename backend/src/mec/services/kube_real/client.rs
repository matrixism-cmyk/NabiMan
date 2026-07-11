use crate::mec::services::kube_service::{KubeHealth, KubeService, NamespaceInfo};
use crate::mec::services::{ServiceError, ServiceResult};
use crate::models::mec::{
    GpuMode, LoadBalancerService, Node, PodInfo, QuotaUsage, ResourceQuota, Taint, TaintEffect,
};
use async_trait::async_trait;
use kube::Client;
use std::time::Instant;

pub struct KubeRealConfig {
    pub kubeconfig_path: Option<String>,
    #[allow(dead_code)]
    pub default_namespace: String,
}

impl KubeRealConfig {
    pub fn from_env() -> Self {
        Self {
            kubeconfig_path: std::env::var("NABIMAN_MEC_KUBECONFIG").ok(),
            default_namespace: std::env::var("NABIMAN_MEC_NAMESPACE")
                .unwrap_or_else(|_| "default".to_string()),
        }
    }
}

pub struct KubeReal {
    pub(super) client: Client,
    pub(super) endpoint: String,
}

impl KubeReal {
    pub async fn connect(config: KubeRealConfig) -> ServiceResult<Self> {
        let (client, endpoint) = build_client(config).await?;
        Ok(Self { client, endpoint })
    }

    #[allow(dead_code)]
    pub fn client(&self) -> &Client {
        &self.client
    }
}

async fn build_client(config: KubeRealConfig) -> ServiceResult<(Client, String)> {
    let kube_config = if let Some(path) = config.kubeconfig_path.as_ref() {
        let kc = kube::config::Kubeconfig::read_from(path).map_err(|e| {
            ServiceError::Unavailable(format!("read kubeconfig {}: {}", path, e))
        })?;
        kube::Config::from_custom_kubeconfig(kc, &Default::default())
            .await
            .map_err(|e| ServiceError::Unavailable(format!("kubeconfig: {}", e)))?
    } else {
        kube::Config::infer()
            .await
            .map_err(|e| ServiceError::Unavailable(format!("infer kubeconfig: {}", e)))?
    };
    let endpoint = kube_config.cluster_url.to_string();
    let client = Client::try_from(kube_config)
        .map_err(|e| ServiceError::Unavailable(format!("create kube client: {}", e)))?;
    Ok((client, endpoint))
}

#[async_trait]
impl KubeService for KubeReal {
    async fn list_nodes(&self) -> ServiceResult<Vec<Node>> {
        super::nodes::list(&self.client).await
    }

    async fn get_node(&self, name: &str) -> ServiceResult<Node> {
        super::nodes::get(&self.client, name).await
    }

    async fn list_pods_on_node(&self, node: &str) -> ServiceResult<Vec<PodInfo>> {
        super::nodes::pods_on_node(&self.client, node).await
    }

    async fn patch_labels(
        &self,
        node: &str,
        add: &[(String, String)],
        remove: &[String],
    ) -> ServiceResult<()> {
        super::nodes::patch_labels(&self.client, node, add, remove).await
    }

    async fn patch_taints(
        &self,
        node: &str,
        add: &[Taint],
        remove: &[(String, Option<TaintEffect>)],
    ) -> ServiceResult<()> {
        super::nodes::patch_taints(&self.client, node, add, remove).await
    }

    async fn cordon(&self, node: &str, schedulable: bool) -> ServiceResult<()> {
        super::nodes::cordon(&self.client, node, schedulable).await
    }

    async fn switch_gpu_mode(
        &self,
        node: &str,
        mode: GpuMode,
        replicas: Option<u32>,
    ) -> ServiceResult<()> {
        super::nodes::switch_gpu_mode(&self.client, node, mode, replicas).await
    }

    async fn list_namespaces(&self) -> ServiceResult<Vec<NamespaceInfo>> {
        super::namespaces::list(&self.client).await
    }

    async fn create_namespace(
        &self,
        name: &str,
        labels: &[(String, String)],
    ) -> ServiceResult<()> {
        super::namespaces::create(&self.client, name, labels).await
    }

    async fn delete_namespace(&self, name: &str) -> ServiceResult<()> {
        super::namespaces::delete(&self.client, name).await
    }

    async fn apply_resource_quota(
        &self,
        namespace: &str,
        quota: &ResourceQuota,
    ) -> ServiceResult<()> {
        super::quota::apply(&self.client, namespace, quota).await
    }

    async fn get_quota_usage(&self, namespace: &str) -> ServiceResult<QuotaUsage> {
        super::quota::usage(&self.client, namespace).await
    }

    async fn apply_network_policy(
        &self,
        namespace: &str,
        name: &str,
        spec: &str,
    ) -> ServiceResult<()> {
        super::rbac::apply_network_policy(&self.client, namespace, name, spec).await
    }

    async fn apply_limit_range(&self, namespace: &str, spec: &str) -> ServiceResult<()> {
        super::rbac::apply_limit_range(&self.client, namespace, spec).await
    }

    async fn apply_rbac(&self, namespace: &str, user: &str) -> ServiceResult<()> {
        super::rbac::apply_rbac(&self.client, namespace, user).await
    }

    async fn apply_deployment(&self, namespace: &str, spec: &str) -> ServiceResult<()> {
        super::workloads::apply_deployment(&self.client, namespace, spec).await
    }

    async fn delete_deployment(&self, namespace: &str, name: &str) -> ServiceResult<()> {
        super::workloads::delete_deployment(&self.client, namespace, name).await
    }

    async fn apply_service(&self, namespace: &str, spec: &str) -> ServiceResult<()> {
        super::workloads::apply_service(&self.client, namespace, spec).await
    }

    async fn delete_service(&self, namespace: &str, name: &str) -> ServiceResult<()> {
        super::workloads::delete_service(&self.client, namespace, name).await
    }

    async fn list_lb_services(
        &self,
        namespace: Option<&str>,
    ) -> ServiceResult<Vec<LoadBalancerService>> {
        super::services_ops::list_lb(&self.client, namespace).await
    }

    async fn list_pods(&self, namespace: &str) -> ServiceResult<Vec<PodInfo>> {
        super::services_ops::list_pods(&self.client, namespace).await
    }

    async fn count_pods(&self, namespace: &str) -> ServiceResult<u32> {
        let pods = super::services_ops::list_pods(&self.client, namespace).await?;
        Ok(pods.len() as u32)
    }

    async fn list_ingresses(
        &self,
        namespace: Option<&str>,
    ) -> ServiceResult<Vec<crate::mec::services::kube_service::IngressRow>> {
        super::ingress_ops::list(&self.client, namespace).await
    }

    async fn apply_ingress(&self, namespace: &str, spec: &str) -> ServiceResult<()> {
        super::ingress_ops::apply(&self.client, namespace, spec).await
    }

    async fn delete_ingress(&self, namespace: &str, name: &str) -> ServiceResult<()> {
        super::ingress_ops::delete(&self.client, namespace, name).await
    }

    async fn list_pvcs(
        &self,
        namespace: Option<&str>,
    ) -> ServiceResult<Vec<crate::mec::services::kube_service::PvcRow>> {
        super::pvcs::list(&self.client, namespace).await
    }

    async fn node_metrics(&self) -> ServiceResult<Vec<crate::models::mec::NodeMetrics>> {
        super::live::node_metrics(&self.client).await
    }

    async fn tenant_usage(&self) -> ServiceResult<Vec<crate::models::mec::TenantUsage>> {
        super::live::tenant_usage(&self.client).await
    }

    async fn pod_phase_summary(&self) -> ServiceResult<crate::models::mec::PodPhaseSummary> {
        super::live::pod_phase_summary(&self.client).await
    }

    async fn list_events(&self, limit: u32) -> ServiceResult<Vec<crate::models::mec::ClusterEvent>> {
        super::live::list_events(&self.client, limit).await
    }

    async fn health_check(&self) -> ServiceResult<KubeHealth> {
        let start = Instant::now();
        let api: kube::Api<k8s_openapi::api::core::v1::Namespace> =
            kube::Api::all(self.client.clone());
        let _ = api.list(&Default::default()).await.map_err(|e| {
            ServiceError::Upstream {
                source: "kubernetes".into(),
                message: e.to_string(),
            }
        })?;
        Ok(KubeHealth {
            connected: true,
            api_version: Some("v1".into()),
            endpoint: self.endpoint.clone(),
            latency_ms: Some(start.elapsed().as_millis() as u64),
            error: None,
        })
    }
}

pub(super) fn upstream_err<E: std::fmt::Display>(e: E) -> ServiceError {
    ServiceError::Upstream {
        source: "kubernetes".into(),
        message: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_env() {
        let c = KubeRealConfig::from_env();
        assert!(!c.default_namespace.is_empty());
    }
}
