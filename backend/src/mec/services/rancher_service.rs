use super::ServiceResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type RancherServiceArc = Arc<dyn RancherService + Send + Sync>;

#[async_trait]
pub trait RancherService: Send + Sync {
    async fn create_project(&self, cluster_id: &str, name: &str) -> ServiceResult<RancherProject>;
    async fn delete_project(&self, id: &str) -> ServiceResult<()>;
    async fn get_project(&self, id: &str) -> ServiceResult<RancherProject>;
    async fn list_projects(&self, cluster_id: &str) -> ServiceResult<Vec<RancherProject>>;

    async fn create_user(&self, username: &str, password: &str) -> ServiceResult<RancherUser>;
    async fn delete_user(&self, id: &str) -> ServiceResult<()>;
    async fn get_user(&self, id: &str) -> ServiceResult<RancherUser>;
    async fn list_users(&self) -> ServiceResult<Vec<RancherUser>>;
    async fn reset_password(&self, id: &str, new_password: &str) -> ServiceResult<()>;

    async fn create_prtb(
        &self,
        project_id: &str,
        user_id: &str,
        role: &str,
    ) -> ServiceResult<ProjectRoleBinding>;
    async fn delete_prtb(&self, id: &str) -> ServiceResult<()>;
    async fn list_prtb(&self, project_id: &str) -> ServiceResult<Vec<ProjectRoleBinding>>;

    async fn health_check(&self) -> ServiceResult<RancherHealth>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RancherProject {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub cluster_id: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RancherUser {
    pub id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub enabled: bool,
    pub must_change_password: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRoleBinding {
    pub id: String,
    pub project_id: String,
    pub user_id: String,
    pub role_template: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RancherHealth {
    pub connected: bool,
    pub endpoint: String,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}
