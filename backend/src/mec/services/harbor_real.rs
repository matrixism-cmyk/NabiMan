use super::harbor_service::{HarborHealth, HarborProject, HarborRepository, HarborService};
use super::{ServiceError, ServiceResult};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};

pub struct HarborRealConfig {
    pub base_url: String,
    pub username: String,
    pub password: String,
    pub insecure_tls: bool,
}

impl HarborRealConfig {
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("NABIMAN_MEC_HARBOR_URL").ok()?;
        let username = std::env::var("NABIMAN_MEC_HARBOR_USER").ok()?;
        let password = std::env::var("NABIMAN_MEC_HARBOR_PASSWORD").ok()?;
        let insecure_tls = std::env::var("NABIMAN_MEC_HARBOR_INSECURE")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);
        Some(Self {
            base_url,
            username,
            password,
            insecure_tls,
        })
    }
}

pub struct HarborReal {
    http: Client,
    base_url: String,
    username: String,
    password: String,
}

impl HarborReal {
    pub fn new(cfg: HarborRealConfig) -> ServiceResult<Self> {
        let http = Client::builder()
            .danger_accept_invalid_certs(cfg.insecure_tls)
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| ServiceError::Unavailable(format!("harbor http: {}", e)))?;
        Ok(Self {
            http,
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            username: cfg.username,
            password: cfg.password,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/api/v2.0{}", self.base_url, path)
    }

    fn auth(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        rb.basic_auth(&self.username, Some(&self.password))
    }
}

fn upstream<E: std::fmt::Display>(e: E) -> ServiceError {
    ServiceError::Upstream {
        source: "harbor".into(),
        message: e.to_string(),
    }
}

#[derive(Debug, Deserialize)]
struct ProjectWire {
    project_id: u32,
    name: String,
    #[serde(default)]
    metadata: serde_json::Value,
    #[serde(default)]
    repo_count: Option<u32>,
    #[serde(default)]
    creation_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepoWire {
    name: String,
    #[allow(dead_code)]
    project_id: u32,
    #[serde(default)]
    pull_count: Option<u64>,
    #[serde(default)]
    artifact_count: Option<u32>,
    #[serde(default)]
    update_time: Option<String>,
}

fn parse_time(s: Option<String>) -> DateTime<Utc> {
    s.as_deref()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

#[async_trait]
impl HarborService for HarborReal {
    async fn list_projects(&self) -> ServiceResult<Vec<HarborProject>> {
        let url = self.url("/projects");
        let res = self.auth(self.http.get(&url)).send().await.map_err(upstream)?;
        if !res.status().is_success() {
            return Err(upstream(format!("list projects: HTTP {}", res.status())));
        }
        let wires: Vec<ProjectWire> = res.json().await.map_err(upstream)?;
        Ok(wires
            .into_iter()
            .map(|w| HarborProject {
                id: w.project_id,
                name: w.name.clone(),
                public: w
                    .metadata
                    .get("public")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "true")
                    .unwrap_or(false),
                repo_count: w.repo_count.unwrap_or(0),
                created_at: parse_time(w.creation_time),
            })
            .collect())
    }

    async fn create_project(
        &self,
        name: &str,
        public: bool,
    ) -> ServiceResult<HarborProject> {
        let url = self.url("/projects");
        let body = json!({
            "project_name": name,
            "metadata": { "public": if public { "true" } else { "false" } },
        });
        let res = self
            .auth(self.http.post(&url))
            .json(&body)
            .send()
            .await
            .map_err(upstream)?;
        if res.status() == reqwest::StatusCode::CONFLICT {
            return Err(ServiceError::Conflict(format!("project {} exists", name)));
        }
        if !res.status().is_success() {
            return Err(upstream(format!(
                "create project: HTTP {}",
                res.status()
            )));
        }
        // Harbor returns Location header; fetch project by name.
        let list = self.list_projects().await?;
        list.into_iter()
            .find(|p| p.name == name)
            .ok_or_else(|| upstream("created project not found"))
    }

    async fn delete_project(&self, name: &str) -> ServiceResult<()> {
        let list = self.list_projects().await?;
        let id = list
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.id)
            .ok_or_else(|| ServiceError::NotFound(format!("harbor project {}", name)))?;
        let url = self.url(&format!("/projects/{}", id));
        let res = self.auth(self.http.delete(&url)).send().await.map_err(upstream)?;
        if res.status().is_success() || res.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(upstream(format!("delete project: HTTP {}", res.status())))
        }
    }

    async fn list_repositories(&self, project: &str) -> ServiceResult<Vec<HarborRepository>> {
        let url = self.url(&format!("/projects/{}/repositories", project));
        let res = self.auth(self.http.get(&url)).send().await.map_err(upstream)?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ServiceError::NotFound(format!(
                "harbor project {}",
                project
            )));
        }
        if !res.status().is_success() {
            return Err(upstream(format!(
                "list repositories: HTTP {}",
                res.status()
            )));
        }
        let wires: Vec<RepoWire> = res.json().await.map_err(upstream)?;
        Ok(wires
            .into_iter()
            .map(|w| HarborRepository {
                name: w.name,
                project: project.to_string(),
                pull_count: w.pull_count.unwrap_or(0),
                artifact_count: w.artifact_count.unwrap_or(0),
                updated_at: parse_time(w.update_time),
            })
            .collect())
    }

    async fn health_check(&self) -> ServiceResult<HarborHealth> {
        let start = Instant::now();
        let url = self.url("/systeminfo");
        let res = self.auth(self.http.get(&url)).send().await;
        let ms = start.elapsed().as_millis() as u64;
        match res {
            Ok(r) if r.status().is_success() => Ok(HarborHealth {
                connected: true,
                endpoint: self.base_url.clone(),
                latency_ms: Some(ms),
                error: None,
            }),
            Ok(r) => Ok(HarborHealth {
                connected: false,
                endpoint: self.base_url.clone(),
                latency_ms: Some(ms),
                error: Some(format!("HTTP {}", r.status())),
            }),
            Err(e) => Ok(HarborHealth {
                connected: false,
                endpoint: self.base_url.clone(),
                latency_ms: None,
                error: Some(e.to_string()),
            }),
        }
    }
}
