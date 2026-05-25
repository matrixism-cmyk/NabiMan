use super::client::{handle_response, upstream_err, RancherReal};
use crate::mec::services::rancher_service::ProjectRoleBinding;
use crate::mec::services::ServiceResult;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct PrtbList {
    #[serde(default)]
    data: Vec<PrtbWire>,
}

#[derive(Debug, Deserialize)]
struct PrtbWire {
    id: String,
    #[serde(default, rename = "projectId")]
    project_id: Option<String>,
    #[serde(default, rename = "userId")]
    user_id: Option<String>,
    #[serde(default, rename = "roleTemplateId")]
    role_template_id: Option<String>,
}

impl From<PrtbWire> for ProjectRoleBinding {
    fn from(w: PrtbWire) -> Self {
        Self {
            id: w.id,
            project_id: w.project_id.unwrap_or_default(),
            user_id: w.user_id.unwrap_or_default(),
            role_template: w.role_template_id.unwrap_or_default(),
        }
    }
}

pub async fn create(
    r: &RancherReal,
    project_id: &str,
    user_id: &str,
    role: &str,
) -> ServiceResult<ProjectRoleBinding> {
    let url = r.url("/v3/projectroletemplatebindings");
    let body = json!({
        "type": "projectRoleTemplateBinding",
        "projectId": project_id,
        "userId": user_id,
        "roleTemplateId": role,
    });
    let res = r
        .auth(r.http.post(&url))
        .json(&body)
        .send()
        .await
        .map_err(upstream_err)?;
    let wire: PrtbWire = handle_response(res, "create prtb").await?;
    Ok(wire.into())
}

pub async fn delete(r: &RancherReal, id: &str) -> ServiceResult<()> {
    let url = r.url(&format!("/v3/projectroletemplatebindings/{}", id));
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
        Err(upstream_err(format!("delete prtb {}: {} {}", id, code, body)))
    }
}

pub async fn list(
    r: &RancherReal,
    project_id: &str,
) -> ServiceResult<Vec<ProjectRoleBinding>> {
    let url = r.url(&format!(
        "/v3/projectroletemplatebindings?projectId={}",
        project_id
    ));
    let res = r
        .auth(r.http.get(&url))
        .send()
        .await
        .map_err(upstream_err)?;
    let list: PrtbList = handle_response(res, "list prtb").await?;
    Ok(list.data.into_iter().map(Into::into).collect())
}
