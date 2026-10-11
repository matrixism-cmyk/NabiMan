use crate::mec::services::ServiceResult;
use async_trait::async_trait;

use super::UnavailableService;
use crate::mec::services::rancher_service::*;

#[async_trait]
impl RancherService for UnavailableService {
    async fn create_project(
        &self,
        _cluster_id: &str,
        _name: &str,
    ) -> ServiceResult<RancherProject> {
        self.reject()
    }

    async fn delete_project(&self, _id: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn get_project(&self, _id: &str) -> ServiceResult<RancherProject> {
        self.reject()
    }

    async fn list_projects(&self, _cluster_id: &str) -> ServiceResult<Vec<RancherProject>> {
        self.reject()
    }

    async fn create_user(&self, _username: &str, _password: &str) -> ServiceResult<RancherUser> {
        self.reject()
    }

    async fn delete_user(&self, _id: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn get_user(&self, _id: &str) -> ServiceResult<RancherUser> {
        self.reject()
    }

    async fn list_users(&self) -> ServiceResult<Vec<RancherUser>> {
        self.reject()
    }

    async fn reset_password(&self, _id: &str, _new_password: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn create_prtb(
        &self,
        _project_id: &str,
        _user_id: &str,
        _role: &str,
    ) -> ServiceResult<ProjectRoleBinding> {
        self.reject()
    }

    async fn delete_prtb(&self, _id: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn list_prtb(&self, _project_id: &str) -> ServiceResult<Vec<ProjectRoleBinding>> {
        self.reject()
    }

    async fn health_check(&self) -> ServiceResult<RancherHealth> {
        self.reject()
    }
}
