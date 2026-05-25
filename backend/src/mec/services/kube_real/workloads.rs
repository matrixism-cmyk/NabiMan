use super::client::upstream_err;
use super::nodes::map_not_found;
use crate::mec::services::ServiceResult;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Service;
use kube::{
    api::{Api, DeleteParams, Patch, PatchParams, PostParams},
    Client,
};

pub async fn apply_deployment(
    client: &Client,
    namespace: &str,
    spec_json: &str,
) -> ServiceResult<()> {
    let d: Deployment = serde_json::from_str(spec_json).map_err(|e| {
        crate::mec::services::ServiceError::InvalidInput(format!("deployment json: {}", e))
    })?;
    let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let name = d
        .metadata
        .name
        .clone()
        .ok_or_else(|| crate::mec::services::ServiceError::InvalidInput(
            "deployment missing metadata.name".into(),
        ))?;
    match api.create(&PostParams::default(), &d).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            let patch = serde_json::to_value(&d).unwrap_or_default();
            api.patch(&name, &PatchParams::default(), &Patch::Merge(&patch))
                .await
                .map_err(upstream_err)?;
            Ok(())
        }
        Err(e) => Err(map_not_found(e, &format!("deployment {}", name))),
    }
}

pub async fn delete_deployment(
    client: &Client,
    namespace: &str,
    name: &str,
) -> ServiceResult<()> {
    let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 404 => Ok(()),
        Err(e) => Err(upstream_err(e)),
    }
}

pub async fn apply_service(
    client: &Client,
    namespace: &str,
    spec_json: &str,
) -> ServiceResult<()> {
    let s: Service = serde_json::from_str(spec_json).map_err(|e| {
        crate::mec::services::ServiceError::InvalidInput(format!("service json: {}", e))
    })?;
    let api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let name = s
        .metadata
        .name
        .clone()
        .ok_or_else(|| crate::mec::services::ServiceError::InvalidInput(
            "service missing metadata.name".into(),
        ))?;
    match api.create(&PostParams::default(), &s).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            let patch = serde_json::to_value(&s).unwrap_or_default();
            api.patch(&name, &PatchParams::default(), &Patch::Merge(&patch))
                .await
                .map_err(upstream_err)?;
            Ok(())
        }
        Err(e) => Err(map_not_found(e, &format!("service {}", name))),
    }
}

pub async fn delete_service(
    client: &Client,
    namespace: &str,
    name: &str,
) -> ServiceResult<()> {
    let api: Api<Service> = Api::namespaced(client.clone(), namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 404 => Ok(()),
        Err(e) => Err(upstream_err(e)),
    }
}
