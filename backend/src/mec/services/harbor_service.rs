use super::ServiceResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type HarborServiceArc = Arc<dyn HarborService + Send + Sync>;

#[async_trait]
pub trait HarborService: Send + Sync {
    async fn list_projects(&self) -> ServiceResult<Vec<HarborProject>>;
    async fn create_project(&self, name: &str, public: bool) -> ServiceResult<HarborProject>;
    #[allow(dead_code)]
    async fn delete_project(&self, name: &str) -> ServiceResult<()>;
    async fn list_repositories(&self, project: &str) -> ServiceResult<Vec<HarborRepository>>;
    async fn health_check(&self) -> ServiceResult<HarborHealth>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarborProject {
    pub name: String,
    pub id: u32,
    pub public: bool,
    pub repo_count: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarborRepository {
    pub name: String,
    pub project: String,
    pub pull_count: u64,
    pub artifact_count: u32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HarborHealth {
    pub connected: bool,
    pub endpoint: String,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}
