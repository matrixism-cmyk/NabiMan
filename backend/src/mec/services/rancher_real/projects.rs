use super::client::{handle_response, upstream_err, RancherReal};
use crate::mec::services::rancher_service::RancherProject;
use crate::mec::services::ServiceResult;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct ProjectList {
    #[serde(default)]
    data: Vec<ProjectWire>,
}

#[derive(Debug, Deserialize)]
struct ProjectWire {
    id: String,
    name: String,
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
    #[serde(default, rename = "clusterId")]
    cluster_id: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

impl From<ProjectWire> for RancherProject {
    fn from(w: ProjectWire) -> Self {
        Self {
            id: w.id,
            name: w.name.clone(),
            display_name: w.display_name.unwrap_or(w.name),
            cluster_id: w.cluster_id.unwrap_or_default(),
            description: w.description,
        }
    }
}

pub async fn create(
    r: &RancherReal,
    cluster_id: &str,
    name: &str,
) -> ServiceResult<RancherProject> {
    let url = r.url("/v3/projects");
    let body = json!({
        "type": "project",
        "name": name,
        "clusterId": cluster_id,
    });
    let res = r
        .auth(r.http.post(&url))
        .json(&body)
        .send()
        .await
        .map_err(upstream_err)?;
    let wire: ProjectWire = handle_response(res, &format!("create project {}", name)).await?;
    Ok(wire.into())
}

pub async fn delete(r: &RancherReal, id: &str) -> ServiceResult<()> {
    let url = r.url(&format!("/v3/projects/{}", id));
    let res = r
        .auth(r.http.delete(&url))
        .send()
        .await
        .map_err(upstream_err)?;
    if res.status().is_success() || res.status() == reqwest::StatusCode::NOT_FOUND {
        Ok(())
    } else {
        let code = res.status();
        let body = res.text().await.unwrap_or_default();
        Err(upstream_err(format!("delete project {}: {} {}", id, code, body)))
    }
}

pub async fn get(r: &RancherReal, id: &str) -> ServiceResult<RancherProject> {
    let url = r.url(&format!("/v3/projects/{}", id));
    let res = r
        .auth(r.http.get(&url))
        .send()
        .await
        .map_err(upstream_err)?;
    let wire: ProjectWire = handle_response(res, &format!("project {}", id)).await?;
    Ok(wire.into())
}

pub async fn list(r: &RancherReal, cluster_id: &str) -> ServiceResult<Vec<RancherProject>> {
    let url = r.url(&format!("/v3/projects?clusterId={}", cluster_id));
    let res = r
        .auth(r.http.get(&url))
        .send()
        .await
        .map_err(upstream_err)?;
    let list: ProjectList = handle_response(res, "list projects").await?;
    Ok(list.data.into_iter().map(Into::into).collect())
}
