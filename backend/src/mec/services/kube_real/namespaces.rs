use super::client::upstream_err;
use super::nodes::map_not_found;
use crate::mec::services::kube_service::NamespaceInfo;
use crate::mec::services::ServiceResult;
use chrono::Utc;
use k8s_openapi::api::core::v1::Namespace;
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams},
    Client,
};
use std::collections::{BTreeMap, HashMap};

pub async fn list(client: &Client) -> ServiceResult<Vec<NamespaceInfo>> {
    let api: Api<Namespace> = Api::all(client.clone());
    let result = api
        .list(&ListParams::default())
        .await
        .map_err(upstream_err)?;
    Ok(result
        .items
        .iter()
        .map(|ns| NamespaceInfo {
            name: ns.metadata.name.clone().unwrap_or_default(),
            phase: ns
                .status
                .as_ref()
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".into()),
            labels: ns
                .metadata
                .labels
                .clone()
                .unwrap_or_default()
                .into_iter()
                .collect::<HashMap<_, _>>(),
            age_seconds: ns
                .metadata
                .creation_timestamp
                .as_ref()
                .map(|t| (Utc::now() - t.0).num_seconds().max(0) as u64)
                .unwrap_or(0),
        })
        .collect())
}

pub async fn create(
    client: &Client,
    name: &str,
    labels: &[(String, String)],
) -> ServiceResult<()> {
    let api: Api<Namespace> = Api::all(client.clone());
    let label_map: BTreeMap<String, String> =
        labels.iter().cloned().collect::<BTreeMap<_, _>>();
    let ns = Namespace {
        metadata: kube::core::ObjectMeta {
            name: Some(name.to_string()),
            labels: Some(label_map),
            ..Default::default()
        },
        ..Default::default()
    };
    api.create(&PostParams::default(), &ns)
        .await
        .map_err(|e| map_not_found(e, &format!("namespace {}", name)))?;
    Ok(())
}

pub async fn delete(client: &Client, name: &str) -> ServiceResult<()> {
    let api: Api<Namespace> = Api::all(client.clone());
    api.delete(name, &DeleteParams::default())
        .await
        .map_err(|e| map_not_found(e, &format!("namespace {}", name)))?;
    Ok(())
}
