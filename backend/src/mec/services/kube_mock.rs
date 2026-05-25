use super::kube_mock_fixtures::{
    sample_events, sample_namespaces, sample_node_metrics, sample_nodes, sample_pod_phases,
    sample_services, sample_tenant_usage,
};
use super::kube_service::{IngressRow, KubeHealth, KubeService, NamespaceInfo, PvcRow};
use super::{ServiceError, ServiceResult};
use crate::models::mec::{
    ClusterEvent, GpuMode, LoadBalancerService, Node, NodeMetrics, PodInfo, PodPhaseSummary,
    QuotaUsage, ResourceQuota, Taint, TaintEffect, TenantUsage,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct KubeMock {
    state: Mutex<MockState>,
}

struct MockState {
    nodes: Vec<Node>,
    namespaces: Vec<NamespaceInfo>,
    services: Vec<LoadBalancerService>,
    pods: HashMap<String, Vec<PodInfo>>,
}

impl Default for KubeMock {
    fn default() -> Self {
        Self::new()
    }
}

impl KubeMock {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(MockState {
                nodes: sample_nodes(),
                namespaces: sample_namespaces(),
                services: sample_services(),
                pods: HashMap::new(),
            }),
        }
    }
}

#[async_trait]
impl KubeService for KubeMock {
    async fn list_nodes(&self) -> ServiceResult<Vec<Node>> {
        Ok(self.state.lock().unwrap().nodes.clone())
    }

    async fn get_node(&self, name: &str) -> ServiceResult<Node> {
        self.state
            .lock()
            .unwrap()
            .nodes
            .iter()
            .find(|n| n.name == name)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound(format!("node {}", name)))
    }

    async fn list_pods_on_node(&self, node: &str) -> ServiceResult<Vec<PodInfo>> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .pods
            .values()
            .flatten()
            .filter(|p| p.node_name.as_deref() == Some(node))
            .cloned()
            .collect())
    }

    async fn patch_labels(
        &self,
        node: &str,
        add: &[(String, String)],
        remove: &[String],
    ) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let n = st
            .nodes
            .iter_mut()
            .find(|n| n.name == node)
            .ok_or_else(|| ServiceError::NotFound(format!("node {}", node)))?;
        for (k, v) in add {
            n.labels.insert(k.clone(), v.clone());
        }
        for k in remove {
            n.labels.remove(k);
        }
        Ok(())
    }

    async fn patch_taints(
        &self,
        node: &str,
        add: &[Taint],
        remove: &[(String, Option<TaintEffect>)],
    ) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let n = st
            .nodes
            .iter_mut()
            .find(|n| n.name == node)
            .ok_or_else(|| ServiceError::NotFound(format!("node {}", node)))?;
        n.taints.extend(add.iter().cloned());
        n.taints.retain(|t| {
            !remove.iter().any(|(k, eff)| {
                &t.key == k && eff.as_ref().map(|e| e == &t.effect).unwrap_or(true)
            })
        });
        Ok(())
    }

    async fn cordon(&self, node: &str, schedulable: bool) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let n = st
            .nodes
            .iter_mut()
            .find(|n| n.name == node)
            .ok_or_else(|| ServiceError::NotFound(format!("node {}", node)))?;
        if schedulable {
            n.labels.remove("unschedulable");
        } else {
            n.labels.insert("unschedulable".into(), "true".into());
        }
        Ok(())
    }

    async fn switch_gpu_mode(
        &self,
        node: &str,
        mode: GpuMode,
        replicas: Option<u32>,
    ) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let n = st
            .nodes
            .iter_mut()
            .find(|n| n.name == node)
            .ok_or_else(|| ServiceError::NotFound(format!("node {}", node)))?;
        if let Some(info) = n.gpu_info.as_mut() {
            info.mode = mode;
            info.replicas = replicas;
        }
        Ok(())
    }

    async fn list_namespaces(&self) -> ServiceResult<Vec<NamespaceInfo>> {
        Ok(self.state.lock().unwrap().namespaces.clone())
    }

    async fn create_namespace(
        &self,
        name: &str,
        labels: &[(String, String)],
    ) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        if st.namespaces.iter().any(|n| n.name == name) {
            return Err(ServiceError::Conflict(format!("namespace {} exists", name)));
        }
        st.namespaces.push(NamespaceInfo {
            name: name.into(),
            phase: "Active".into(),
            labels: labels.iter().cloned().collect(),
            age_seconds: 0,
        });
        Ok(())
    }

    async fn delete_namespace(&self, name: &str) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let before = st.namespaces.len();
        st.namespaces.retain(|n| n.name != name);
        if st.namespaces.len() == before {
            return Err(ServiceError::NotFound(format!("namespace {}", name)));
        }
        Ok(())
    }

    async fn apply_resource_quota(
        &self,
        _namespace: &str,
        _quota: &ResourceQuota,
    ) -> ServiceResult<()> {
        Ok(())
    }

    async fn get_quota_usage(&self, namespace: &str) -> ServiceResult<QuotaUsage> {
        Ok(QuotaUsage {
            tenant_id: namespace.into(),
            cpu_requests_used: "0".into(),
            cpu_requests_hard: "32".into(),
            cpu_limits_used: "0".into(),
            cpu_limits_hard: "64".into(),
            memory_requests_used: "0".into(),
            memory_requests_hard: "64Gi".into(),
            memory_limits_used: "0".into(),
            memory_limits_hard: "128Gi".into(),
            gpu_used: 0,
            gpu_hard: 2,
            pods_used: 0,
            pods_hard: 100,
            lb_services_used: 0,
            lb_services_hard: 5,
            storage_used: "0".into(),
            storage_hard: "500Gi".into(),
        })
    }

    async fn apply_network_policy(
        &self,
        _namespace: &str,
        _name: &str,
        _spec: &str,
    ) -> ServiceResult<()> {
        Ok(())
    }

    async fn apply_limit_range(
        &self,
        _namespace: &str,
        _spec: &str,
    ) -> ServiceResult<()> {
        Ok(())
    }

    async fn apply_rbac(&self, _namespace: &str, _user: &str) -> ServiceResult<()> {
        Ok(())
    }

    async fn apply_deployment(&self, _namespace: &str, _spec: &str) -> ServiceResult<()> {
        Ok(())
    }

    async fn delete_deployment(&self, _namespace: &str, _name: &str) -> ServiceResult<()> {
        Ok(())
    }

    async fn apply_service(&self, namespace: &str, spec: &str) -> ServiceResult<()> {
        // Parse minimal spec so the mock surfaces created services in list_lb_services.
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(spec) {
            let is_lb = v["spec"]["type"] == "LoadBalancer";
            if is_lb {
                let name = v["metadata"]["name"].as_str().unwrap_or("unknown").to_string();
                let port = v["spec"]["ports"][0]["port"].as_u64().unwrap_or(0) as u16;
                let target = v["spec"]["ports"][0]["targetPort"].as_u64().unwrap_or(port as u64) as u16;
                let proto = v["spec"]["ports"][0]["protocol"].as_str().unwrap_or("TCP").to_string();
                let mut st = self.state.lock().unwrap();
                st.services.retain(|s| !(s.namespace == namespace && s.name == name));
                let next_ip = format!("172.20.26.{}", 150 + (st.services.len() as u32 % 50));
                st.services.push(crate::models::mec::LoadBalancerService {
                    namespace: namespace.into(),
                    name,
                    external_ip: next_ip,
                    ports: vec![crate::models::mec::ServicePort {
                        name: None,
                        port,
                        target_port: target,
                        node_port: None,
                        protocol: proto,
                    }],
                    selector_summary: String::new(),
                    age_seconds: 0,
                });
            }
        }
        Ok(())
    }

    async fn delete_service(&self, namespace: &str, name: &str) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        st.services
            .retain(|s| !(s.namespace == namespace && s.name == name));
        Ok(())
    }

    async fn list_lb_services(
        &self,
        namespace: Option<&str>,
    ) -> ServiceResult<Vec<LoadBalancerService>> {
        let st = self.state.lock().unwrap();
        Ok(st
            .services
            .iter()
            .filter(|s| namespace.map_or(true, |ns| s.namespace == ns))
            .cloned()
            .collect())
    }

    async fn list_pods(&self, namespace: &str) -> ServiceResult<Vec<PodInfo>> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .pods
            .get(namespace)
            .cloned()
            .unwrap_or_default())
    }

    async fn count_pods(&self, namespace: &str) -> ServiceResult<u32> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .pods
            .get(namespace)
            .map(|p| p.len() as u32)
            .unwrap_or(0))
    }

    async fn list_ingresses(&self, _namespace: Option<&str>) -> ServiceResult<Vec<IngressRow>> {
        Ok(vec![])
    }

    async fn apply_ingress(&self, _namespace: &str, _spec: &str) -> ServiceResult<()> {
        Ok(())
    }

    async fn delete_ingress(&self, _namespace: &str, _name: &str) -> ServiceResult<()> {
        Ok(())
    }

    async fn list_pvcs(&self, _namespace: Option<&str>) -> ServiceResult<Vec<PvcRow>> {
        Ok(vec![])
    }

    async fn node_metrics(&self) -> ServiceResult<Vec<NodeMetrics>> {
        Ok(sample_node_metrics())
    }

    async fn tenant_usage(&self) -> ServiceResult<Vec<TenantUsage>> {
        Ok(sample_tenant_usage())
    }

    async fn pod_phase_summary(&self) -> ServiceResult<PodPhaseSummary> {
        Ok(sample_pod_phases())
    }

    async fn list_events(&self, limit: u32) -> ServiceResult<Vec<ClusterEvent>> {
        Ok(sample_events(limit))
    }

    async fn health_check(&self) -> ServiceResult<KubeHealth> {
        Ok(KubeHealth {
            connected: true,
            api_version: Some("v1.29.0".into()),
            endpoint: "mock://kube".into(),
            latency_ms: Some(1),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_list_nodes() {
        let m = KubeMock::new();
        let nodes = m.list_nodes().await.unwrap();
        assert!(nodes.len() >= 4);
    }

    #[tokio::test]
    async fn mock_create_delete_namespace() {
        let m = KubeMock::new();
        m.create_namespace("foo", &[]).await.unwrap();
        assert!(m.list_namespaces().await.unwrap().iter().any(|n| n.name == "foo"));
        m.delete_namespace("foo").await.unwrap();
        assert!(!m.list_namespaces().await.unwrap().iter().any(|n| n.name == "foo"));
    }

    #[tokio::test]
    async fn mock_taint_add_remove() {
        let m = KubeMock::new();
        m.patch_taints(
            "mec-cp01",
            &[Taint {
                key: "dedicated".into(),
                value: Some("test".into()),
                effect: TaintEffect::NoSchedule,
            }],
            &[],
        )
        .await
        .unwrap();
        let n = m.get_node("mec-cp01").await.unwrap();
        assert!(n.taints.iter().any(|t| t.key == "dedicated"));
    }
}
