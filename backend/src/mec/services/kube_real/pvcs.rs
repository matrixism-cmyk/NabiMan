use super::client::upstream_err;
use crate::mec::services::kube_service::PvcRow;
use crate::mec::services::ServiceResult;
use chrono::Utc;
use k8s_openapi::api::core::v1::PersistentVolumeClaim;
use kube::{
    api::{Api, ListParams},
    Client,
};

pub async fn list(
    client: &Client,
    namespace: Option<&str>,
) -> ServiceResult<Vec<PvcRow>> {
    let api: Api<PersistentVolumeClaim> = match namespace {
        Some(ns) => Api::namespaced(client.clone(), ns),
        None => Api::all(client.clone()),
    };
    let result = api
        .list(&ListParams::default())
        .await
        .map_err(upstream_err)?;
    Ok(result.items.iter().map(convert).collect())
}

fn convert(pvc: &PersistentVolumeClaim) -> PvcRow {
    let meta = pvc.metadata.clone();
    let spec = pvc.spec.clone().unwrap_or_default();
    let status = pvc.status.clone().unwrap_or_default();

    let storage_request = spec
        .resources
        .as_ref()
        .and_then(|r| r.requests.as_ref())
        .and_then(|req| req.get("storage"))
        .map(|q| q.0.clone())
        .unwrap_or_else(|| "-".into());

    PvcRow {
        namespace: meta.namespace.clone().unwrap_or_default(),
        name: meta.name.clone().unwrap_or_default(),
        phase: status.phase.unwrap_or_else(|| "Unknown".into()),
        storage_request,
        storage_class: spec.storage_class_name,
        volume_name: spec.volume_name,
        access_modes: spec.access_modes.unwrap_or_default(),
        age_seconds: meta
            .creation_timestamp
            .as_ref()
            .map(|t| (Utc::now() - t.0).num_seconds().max(0) as u64)
            .unwrap_or(0),
    }
}
