use super::client::upstream_err;
use super::nodes::map_not_found;
use crate::mec::services::kube_service::{IngressPathRow, IngressRow};
use crate::mec::services::ServiceResult;
use chrono::Utc;
use k8s_openapi::api::networking::v1::Ingress;
use kube::{
    api::{Api, DeleteParams, ListParams, Patch, PatchParams, PostParams},
    Client,
};

pub async fn list(
    client: &Client,
    namespace: Option<&str>,
) -> ServiceResult<Vec<IngressRow>> {
    let api: Api<Ingress> = match namespace {
        Some(ns) => Api::namespaced(client.clone(), ns),
        None => Api::all(client.clone()),
    };
    let result = api
        .list(&ListParams::default())
        .await
        .map_err(upstream_err)?;
    Ok(result.items.iter().map(convert).collect())
}

pub async fn apply(client: &Client, namespace: &str, spec_json: &str) -> ServiceResult<()> {
    let ing: Ingress = serde_json::from_str(spec_json).map_err(|e| {
        crate::mec::services::ServiceError::InvalidInput(format!("ingress json: {}", e))
    })?;
    let api: Api<Ingress> = Api::namespaced(client.clone(), namespace);
    let name = ing
        .metadata
        .name
        .clone()
        .ok_or_else(|| {
            crate::mec::services::ServiceError::InvalidInput(
                "ingress missing metadata.name".into(),
            )
        })?;
    match api.create(&PostParams::default(), &ing).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            let patch = serde_json::to_value(&ing).unwrap_or_default();
            api.patch(&name, &PatchParams::default(), &Patch::Merge(&patch))
                .await
                .map_err(upstream_err)?;
            Ok(())
        }
        Err(e) => Err(map_not_found(e, &format!("ingress {}", name))),
    }
}

pub async fn delete(client: &Client, namespace: &str, name: &str) -> ServiceResult<()> {
    let api: Api<Ingress> = Api::namespaced(client.clone(), namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 404 => Ok(()),
        Err(e) => Err(upstream_err(e)),
    }
}

fn convert(ing: &Ingress) -> IngressRow {
    let meta = ing.metadata.clone();
    let spec = ing.spec.clone().unwrap_or_default();
    let class = spec.ingress_class_name.clone();

    let mut hosts = Vec::new();
    let mut paths = Vec::new();

    for rule in spec.rules.unwrap_or_default() {
        let host = rule.host.clone();
        if let Some(h) = host.clone() {
            hosts.push(h);
        }
        if let Some(http) = rule.http {
            for p in http.paths {
                paths.push(IngressPathRow {
                    host: host.clone(),
                    path: p.path.unwrap_or_else(|| "/".into()),
                    path_type: p.path_type,
                    backend_service: p
                        .backend
                        .service
                        .as_ref()
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                    backend_port: p
                        .backend
                        .service
                        .as_ref()
                        .and_then(|s| s.port.as_ref())
                        .and_then(|prt| prt.number)
                        .map(|n| n as u16)
                        .unwrap_or(0),
                });
            }
        }
    }

    IngressRow {
        namespace: meta.namespace.clone().unwrap_or_default(),
        name: meta.name.clone().unwrap_or_default(),
        class,
        hosts,
        paths,
        age_seconds: meta
            .creation_timestamp
            .as_ref()
            .map(|t| (Utc::now() - t.0).num_seconds().max(0) as u64)
            .unwrap_or(0),
    }
}
