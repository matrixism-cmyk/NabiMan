use super::client::upstream_err;
use super::nodes::map_not_found;
use crate::mec::services::ServiceResult;
use crate::models::mec::{QuotaUsage, ResourceQuota};
use k8s_openapi::api::core::v1::ResourceQuota as K8sQuota;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::{
    api::{Api, Patch, PatchParams, PostParams},
    Client,
};
use std::collections::BTreeMap;

const QUOTA_NAME: &str = "tenant-quota";

pub async fn apply(
    client: &Client,
    namespace: &str,
    quota: &ResourceQuota,
) -> ServiceResult<()> {
    let api: Api<K8sQuota> = Api::namespaced(client.clone(), namespace);

    let mut hard: BTreeMap<String, Quantity> = BTreeMap::new();
    hard.insert("requests.cpu".into(), Quantity(quota.cpu_requests.clone()));
    hard.insert("limits.cpu".into(), Quantity(quota.cpu_limits.clone()));
    hard.insert("requests.memory".into(), Quantity(quota.memory_requests.clone()));
    hard.insert("limits.memory".into(), Quantity(quota.memory_limits.clone()));
    hard.insert(
        "requests.nvidia.com/gpu".into(),
        Quantity(quota.gpu.to_string()),
    );
    hard.insert(
        "limits.nvidia.com/gpu".into(),
        Quantity(quota.gpu.to_string()),
    );
    hard.insert(
        "requests.storage".into(),
        Quantity(quota.storage.clone()),
    );
    hard.insert("pods".into(), Quantity(quota.pods.to_string()));
    hard.insert("persistentvolumeclaims".into(), Quantity(quota.pvcs.to_string()));
    hard.insert(
        "services.loadbalancers".into(),
        Quantity(quota.lb_services.to_string()),
    );

    let body = K8sQuota {
        metadata: kube::core::ObjectMeta {
            name: Some(QUOTA_NAME.into()),
            namespace: Some(namespace.into()),
            ..Default::default()
        },
        spec: Some(k8s_openapi::api::core::v1::ResourceQuotaSpec {
            hard: Some(hard),
            ..Default::default()
        }),
        status: None,
    };

    match api.create(&PostParams::default(), &body).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            let patch = serde_json::json!({ "spec": body.spec });
            api.patch(QUOTA_NAME, &PatchParams::default(), &Patch::Merge(&patch))
                .await
                .map_err(upstream_err)?;
            Ok(())
        }
        Err(e) => Err(map_not_found(e, &format!("quota in {}", namespace))),
    }
}

pub async fn usage(client: &Client, namespace: &str) -> ServiceResult<QuotaUsage> {
    let api: Api<K8sQuota> = Api::namespaced(client.clone(), namespace);
    let q = api
        .get(QUOTA_NAME)
        .await
        .map_err(|e| map_not_found(e, &format!("quota in {}", namespace)))?;
    let used = q
        .status
        .as_ref()
        .and_then(|s| s.used.clone())
        .unwrap_or_default();
    let hard = q
        .status
        .as_ref()
        .and_then(|s| s.hard.clone())
        .unwrap_or_default();
    Ok(QuotaUsage {
        tenant_id: namespace.into(),
        cpu_requests_used: q_val(&used, "requests.cpu"),
        cpu_requests_hard: q_val(&hard, "requests.cpu"),
        cpu_limits_used: q_val(&used, "limits.cpu"),
        cpu_limits_hard: q_val(&hard, "limits.cpu"),
        memory_requests_used: q_val(&used, "requests.memory"),
        memory_requests_hard: q_val(&hard, "requests.memory"),
        memory_limits_used: q_val(&used, "limits.memory"),
        memory_limits_hard: q_val(&hard, "limits.memory"),
        gpu_used: q_u32(&used, "requests.nvidia.com/gpu"),
        gpu_hard: q_u32(&hard, "requests.nvidia.com/gpu"),
        pods_used: q_u32(&used, "pods"),
        pods_hard: q_u32(&hard, "pods"),
        lb_services_used: q_u32(&used, "services.loadbalancers"),
        lb_services_hard: q_u32(&hard, "services.loadbalancers"),
        storage_used: q_val(&used, "requests.storage"),
        storage_hard: q_val(&hard, "requests.storage"),
    })
}

fn q_val(map: &BTreeMap<String, Quantity>, k: &str) -> String {
    map.get(k).map(|v| v.0.clone()).unwrap_or_else(|| "0".into())
}

fn q_u32(map: &BTreeMap<String, Quantity>, k: &str) -> u32 {
    map.get(k)
        .and_then(|v| v.0.parse().ok())
        .unwrap_or(0)
}
