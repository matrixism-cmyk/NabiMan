use super::rancher_service::{
    ProjectRoleBinding, RancherHealth, RancherProject, RancherService, RancherUser,
};
use super::{ServiceError, ServiceResult};
use async_trait::async_trait;
use std::sync::Mutex;

pub struct RancherMock {
    state: Mutex<State>,
}

struct State {
    projects: Vec<RancherProject>,
    users: Vec<RancherUser>,
    prtbs: Vec<ProjectRoleBinding>,
    next_id: u32,
}

impl Default for RancherMock {
    fn default() -> Self {
        Self::new()
    }
}

impl RancherMock {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                projects: vec![],
                users: vec![],
                prtbs: vec![],
                next_id: 1,
            }),
        }
    }

    fn gen_id(&self, prefix: &str) -> String {
        let mut st = self.state.lock().unwrap();
        let id = format!("{}-mock{:04}", prefix, st.next_id);
        st.next_id += 1;
        id
    }
}

#[async_trait]
impl RancherService for RancherMock {
    async fn create_project(
        &self,
        cluster_id: &str,
        name: &str,
    ) -> ServiceResult<RancherProject> {
        let id = self.gen_id("p");
        let p = RancherProject {
            id: id.clone(),
            name: name.into(),
            display_name: name.into(),
            cluster_id: cluster_id.into(),
            description: None,
        };
        self.state.lock().unwrap().projects.push(p.clone());
        Ok(p)
    }

    async fn delete_project(&self, id: &str) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let before = st.projects.len();
        st.projects.retain(|p| p.id != id);
        if st.projects.len() == before {
            return Err(ServiceError::NotFound(format!("project {}", id)));
        }
        Ok(())
    }

    async fn get_project(&self, id: &str) -> ServiceResult<RancherProject> {
        self.state
            .lock()
            .unwrap()
            .projects
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound(format!("project {}", id)))
    }

    async fn list_projects(&self, cluster_id: &str) -> ServiceResult<Vec<RancherProject>> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .projects
            .iter()
            .filter(|p| p.cluster_id == cluster_id)
            .cloned()
            .collect())
    }

    async fn create_user(&self, username: &str, _password: &str) -> ServiceResult<RancherUser> {
        let id = self.gen_id("u");
        let u = RancherUser {
            id: id.clone(),
            username: username.into(),
            display_name: Some(username.into()),
            enabled: true,
            must_change_password: false,
        };
        self.state.lock().unwrap().users.push(u.clone());
        Ok(u)
    }

    async fn delete_user(&self, id: &str) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let before = st.users.len();
        st.users.retain(|u| u.id != id);
        if st.users.len() == before {
            return Err(ServiceError::NotFound(format!("user {}", id)));
        }
        Ok(())
    }

    async fn get_user(&self, id: &str) -> ServiceResult<RancherUser> {
        self.state
            .lock()
            .unwrap()
            .users
            .iter()
            .find(|u| u.id == id)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound(format!("user {}", id)))
    }

    async fn list_users(&self) -> ServiceResult<Vec<RancherUser>> {
        Ok(self.state.lock().unwrap().users.clone())
    }

    async fn reset_password(&self, id: &str, _new_password: &str) -> ServiceResult<()> {
        let st = self.state.lock().unwrap();
        if !st.users.iter().any(|u| u.id == id) {
            return Err(ServiceError::NotFound(format!("user {}", id)));
        }
        Ok(())
    }

    async fn create_prtb(
        &self,
        project_id: &str,
        user_id: &str,
        role: &str,
    ) -> ServiceResult<ProjectRoleBinding> {
        let id = self.gen_id("prtb");
        let b = ProjectRoleBinding {
            id: id.clone(),
            project_id: project_id.into(),
            user_id: user_id.into(),
            role_template: role.into(),
        };
        self.state.lock().unwrap().prtbs.push(b.clone());
        Ok(b)
    }

    async fn delete_prtb(&self, id: &str) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let before = st.prtbs.len();
        st.prtbs.retain(|b| b.id != id);
        if st.prtbs.len() == before {
            return Err(ServiceError::NotFound(format!("prtb {}", id)));
        }
        Ok(())
    }

    async fn list_prtb(&self, project_id: &str) -> ServiceResult<Vec<ProjectRoleBinding>> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .prtbs
            .iter()
            .filter(|b| b.project_id == project_id)
            .cloned()
            .collect())
    }

    async fn health_check(&self) -> ServiceResult<RancherHealth> {
        Ok(RancherHealth {
            connected: true,
            endpoint: "mock://rancher".into(),
            latency_ms: Some(1),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_and_list_project() {
        let m = RancherMock::new();
        let p = m.create_project("c-local", "ygram").await.unwrap();
        let all = m.list_projects("c-local").await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, p.id);
    }

    #[tokio::test]
    async fn create_prtb_flow() {
        let m = RancherMock::new();
        let p = m.create_project("c", "p1").await.unwrap();
        let u = m.create_user("alice", "pw").await.unwrap();
        let b = m.create_prtb(&p.id, &u.id, "project-owner").await.unwrap();
        let list = m.list_prtb(&p.id).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, b.id);
    }
}
