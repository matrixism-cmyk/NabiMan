use crate::mec::services::ServiceResult;
use async_trait::async_trait;

use super::UnavailableService;
use crate::mec::services::harbor_service::*;

#[async_trait]
impl HarborService for UnavailableService {
    async fn list_projects(&self) -> ServiceResult<Vec<HarborProject>> {
        self.reject()
    }

    async fn create_project(&self, _name: &str, _public: bool) -> ServiceResult<HarborProject> {
        self.reject()
    }

    async fn delete_project(&self, _name: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn list_repositories(&self, _project: &str) -> ServiceResult<Vec<HarborRepository>> {
        self.reject()
    }

    async fn health_check(&self) -> ServiceResult<HarborHealth> {
        self.reject()
    }
}
