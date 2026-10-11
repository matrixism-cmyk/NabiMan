use super::client::{handle_response, upstream_err, RancherReal};
use crate::mec::services::rancher_service::ProjectRoleBinding;
use crate::mec::services::{ServiceError, ServiceResult};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct PrtbList {
    data: Vec<PrtbWire>,
    #[serde(default)]
    pagination: Option<Pagination>,
}

#[derive(Debug, Deserialize)]
struct Pagination {
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    partial: bool,
    #[serde(default)]
    total: Option<usize>,
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
    validate_identifier(id)?;
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
        Err(upstream_err(format!(
            "delete prtb {}: {} {}",
            id, code, body
        )))
    }
}

pub async fn list(r: &RancherReal, project_id: &str) -> ServiceResult<Vec<ProjectRoleBinding>> {
    validate_identifier(project_id)?;
    let base = reqwest::Url::parse(&r.url("/v3/projectroletemplatebindings"))
        .map_err(|_| invalid_page())?;
    let mut url = base.clone();
    url.query_pairs_mut()
        .append_pair("projectId", project_id)
        .append_pair("limit", "1000");
    let mut seen_pages = std::collections::HashSet::new();
    let mut seen_ids = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut reported_total = None;
    // Bound time as well as pages/resources. Never return a partial success.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    for _ in 0..100 {
        if !seen_pages.insert(url.to_string()) {
            return Err(invalid_page());
        }
        let page: PrtbList = tokio::time::timeout_at(deadline, async {
            let response = r
                .auth(r.http.get(url.clone()))
                .send()
                .await
                .map_err(upstream_err)?;
            handle_response(response, "list project members").await
        })
        .await
        .map_err(|_| {
            ServiceError::Unavailable("Rancher project member inventory timed out".into())
        })??;
        if let Some(total) = page.pagination.as_ref().and_then(|page| page.total) {
            if total > 10_000 || reported_total.is_some_and(|previous| previous != total) {
                return Err(invalid_page());
            }
            reported_total = Some(total);
        }
        for wire in page.data {
            let binding: ProjectRoleBinding = wire.into();
            if binding.project_id != project_id
                || validate_identifier(&binding.id).is_err()
                || validate_identifier(&binding.user_id).is_err()
                || validate_identifier(&binding.role_template).is_err()
                || !seen_ids.insert(binding.id.clone())
            {
                return Err(invalid_page());
            }
            out.push(binding);
            if out.len() > 10_000 {
                return Err(invalid_page());
            }
        }
        match page.pagination {
            Some(page) if page.next.as_ref().is_some_and(|next| !next.is_empty()) => {
                url = next_page(&base, &url, page.next.as_deref().unwrap(), project_id)?;
            }
            // Norman marks the final page partial when its start index is >0.
            // A stable total matching all collected rows proves exhaustion.
            Some(page) if page.partial && reported_total != Some(out.len()) => {
                return Err(invalid_page())
            }
            _ if reported_total.is_some_and(|total| total != out.len()) => {
                return Err(invalid_page())
            }
            _ => return Ok(out),
        }
    }
    Err(invalid_page())
}

fn invalid_page() -> ServiceError {
    ServiceError::upstream(
        "rancher",
        "Project member inventory is incomplete or has an invalid scope",
    )
}

fn next_page(
    base: &reqwest::Url,
    current: &reqwest::Url,
    next: &str,
    project: &str,
) -> ServiceResult<reqwest::Url> {
    if next.len() > 8192 {
        return Err(invalid_page());
    }
    let mut url = current.join(next).map_err(|_| invalid_page())?;
    if url.origin() != base.origin()
        || url.path() != base.path()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_page());
    }
    let projects: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| key == "projectId")
        .map(|(_, value)| value.into_owned())
        .collect();
    if projects.len() > 1 || projects.first().is_some_and(|value| value != project) {
        return Err(invalid_page());
    }
    if projects.is_empty() {
        url.query_pairs_mut().append_pair("projectId", project);
    }
    Ok(url)
}

fn validate_identifier(id: &str) -> ServiceResult<()> {
    if id.is_empty()
        || id.len() > 256
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b))
    {
        return Err(ServiceError::InvalidInput(
            "Invalid Rancher resource identifier".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "prtb_tests.rs"]
mod tests;
