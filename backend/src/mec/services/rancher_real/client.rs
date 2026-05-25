use crate::mec::services::rancher_service::{
    ProjectRoleBinding, RancherHealth, RancherProject, RancherService, RancherUser,
};
use crate::mec::services::{ServiceError, ServiceResult};
use async_trait::async_trait;
use reqwest::Client;
use std::time::{Duration, Instant};

pub struct RancherRealConfig {
    pub base_url: String,
    pub token: String,
    pub cluster_id: String,
    pub insecure_tls: bool,
}

impl RancherRealConfig {
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("NABIMAN_MEC_RANCHER_URL").ok()?;
        let token = std::env::var("NABIMAN_MEC_RANCHER_TOKEN").ok()?;
        let cluster_id = std::env::var("NABIMAN_MEC_RANCHER_CLUSTER")
            .unwrap_or_else(|_| "local".into());
        let insecure_tls = std::env::var("NABIMAN_MEC_RANCHER_INSECURE")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);
        Some(Self {
            base_url,
            token,
            cluster_id,
            insecure_tls,
        })
    }
}

pub struct RancherReal {
    pub(super) http: Client,
    pub(super) base_url: String,
    pub(super) token: String,
    pub(super) cluster_id: String,
}

impl RancherReal {
    pub fn new(cfg: RancherRealConfig) -> ServiceResult<Self> {
        let http = Client::builder()
            .danger_accept_invalid_certs(cfg.insecure_tls)
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| ServiceError::Unavailable(format!("rancher http: {}", e)))?;
        Ok(Self {
            http,
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            token: cfg.token,
            cluster_id: cfg.cluster_id,
        })
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub(super) fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.bearer_auth(&self.token)
    }
}

#[async_trait]
impl RancherService for RancherReal {
    async fn create_project(
        &self,
        cluster_id: &str,
        name: &str,
    ) -> ServiceResult<RancherProject> {
        super::projects::create(self, cluster_id, name).await
    }

    async fn delete_project(&self, id: &str) -> ServiceResult<()> {
        super::projects::delete(self, id).await
    }

    async fn get_project(&self, id: &str) -> ServiceResult<RancherProject> {
        super::projects::get(self, id).await
    }

    async fn list_projects(&self, cluster_id: &str) -> ServiceResult<Vec<RancherProject>> {
        super::projects::list(self, cluster_id).await
    }

    async fn create_user(&self, username: &str, password: &str) -> ServiceResult<RancherUser> {
        super::users::create(self, username, password).await
    }

    async fn delete_user(&self, id: &str) -> ServiceResult<()> {
        super::users::delete(self, id).await
    }

    async fn get_user(&self, id: &str) -> ServiceResult<RancherUser> {
        super::users::get(self, id).await
    }

    async fn list_users(&self) -> ServiceResult<Vec<RancherUser>> {
        super::users::list(self).await
    }

    async fn reset_password(&self, id: &str, new_password: &str) -> ServiceResult<()> {
        super::users::reset_password(self, id, new_password).await
    }

    async fn create_prtb(
        &self,
        project_id: &str,
        user_id: &str,
        role: &str,
    ) -> ServiceResult<ProjectRoleBinding> {
        super::prtb::create(self, project_id, user_id, role).await
    }

    async fn delete_prtb(&self, id: &str) -> ServiceResult<()> {
        super::prtb::delete(self, id).await
    }

    async fn list_prtb(&self, project_id: &str) -> ServiceResult<Vec<ProjectRoleBinding>> {
        super::prtb::list(self, project_id).await
    }

    async fn health_check(&self) -> ServiceResult<RancherHealth> {
        let start = Instant::now();
        let url = self.url("/v3");
        let res = self.auth(self.http.get(&url)).send().await;
        let ms = start.elapsed().as_millis() as u64;
        match res {
            Ok(r) if r.status().is_success() => Ok(RancherHealth {
                connected: true,
                endpoint: self.base_url.clone(),
                latency_ms: Some(ms),
                error: None,
            }),
            Ok(r) => Ok(RancherHealth {
                connected: false,
                endpoint: self.base_url.clone(),
                latency_ms: Some(ms),
                error: Some(format!("HTTP {}", r.status())),
            }),
            Err(e) => Ok(RancherHealth {
                connected: false,
                endpoint: self.base_url.clone(),
                latency_ms: None,
                error: Some(e.to_string()),
            }),
        }
    }
}

pub(super) fn upstream_err<E: std::fmt::Display>(e: E) -> ServiceError {
    ServiceError::Upstream {
        source: "rancher".into(),
        message: e.to_string(),
    }
}

pub(super) async fn handle_response<T: serde::de::DeserializeOwned>(
    res: reqwest::Response,
    what: &str,
) -> ServiceResult<T> {
    let status = res.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(ServiceError::NotFound(what.to_string()));
    }
    if status == reqwest::StatusCode::CONFLICT {
        let body = res.text().await.unwrap_or_default();
        return Err(ServiceError::Conflict(format!("{}: {}", what, body)));
    }
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(ServiceError::Upstream {
            source: "rancher".into(),
            message: format!("{} failed {}: {}", what, status, body),
        });
    }
    res.json::<T>().await.map_err(upstream_err)
}
