use super::harbor_service::{HarborHealth, HarborProject, HarborRepository, HarborService};
use super::{ServiceError, ServiceResult};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Mutex;

pub struct HarborMock {
    state: Mutex<State>,
}

struct State {
    projects: Vec<HarborProject>,
    repositories: Vec<HarborRepository>,
    next_id: u32,
}

impl Default for HarborMock {
    fn default() -> Self {
        Self::new()
    }
}

impl HarborMock {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                projects: vec![HarborProject {
                    name: "system".into(),
                    id: 1,
                    public: false,
                    repo_count: 3,
                    created_at: Utc::now(),
                }],
                repositories: vec![],
                next_id: 2,
            }),
        }
    }
}

#[async_trait]
impl HarborService for HarborMock {
    async fn list_projects(&self) -> ServiceResult<Vec<HarborProject>> {
        Ok(self.state.lock().unwrap().projects.clone())
    }

    async fn create_project(&self, name: &str, public: bool) -> ServiceResult<HarborProject> {
        let mut st = self.state.lock().unwrap();
        if st.projects.iter().any(|p| p.name == name) {
            return Err(ServiceError::Conflict(format!(
                "harbor project {} exists",
                name
            )));
        }
        let p = HarborProject {
            name: name.into(),
            id: st.next_id,
            public,
            repo_count: 0,
            created_at: Utc::now(),
        };
        st.next_id += 1;
        st.projects.push(p.clone());
        Ok(p)
    }

    async fn delete_project(&self, name: &str) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let before = st.projects.len();
        st.projects.retain(|p| p.name != name);
        if st.projects.len() == before {
            return Err(ServiceError::NotFound(format!("harbor project {}", name)));
        }
        st.repositories.retain(|r| r.project != name);
        Ok(())
    }

    async fn list_repositories(&self, project: &str) -> ServiceResult<Vec<HarborRepository>> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .repositories
            .iter()
            .filter(|r| r.project == project)
            .cloned()
            .collect())
    }

    async fn health_check(&self) -> ServiceResult<HarborHealth> {
        Ok(HarborHealth {
            connected: true,
            endpoint: "mock://harbor".into(),
            latency_ms: Some(1),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_and_delete_project() {
        let m = HarborMock::new();
        m.create_project("ygram-poc", false).await.unwrap();
        assert!(m
            .list_projects()
            .await
            .unwrap()
            .iter()
            .any(|p| p.name == "ygram-poc"));
        m.delete_project("ygram-poc").await.unwrap();
        assert!(!m
            .list_projects()
            .await
            .unwrap()
            .iter()
            .any(|p| p.name == "ygram-poc"));
    }

    #[tokio::test]
    async fn duplicate_project_rejected() {
        let m = HarborMock::new();
        m.create_project("x", false).await.unwrap();
        assert!(m.create_project("x", false).await.is_err());
    }
}
