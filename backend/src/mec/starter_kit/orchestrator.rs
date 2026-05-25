use super::templates::{ubuntu_ssh_manifests, vscode_manifests};
use crate::mec::db::tenant_store::TenantStore;
use crate::mec::services::{ServiceBundle, ServiceResult};
use crate::models::mec::{StarterKit, StarterService, Tenant};
use chrono::Utc;
use rand::Rng;
use std::sync::Arc;

pub struct StarterKitOrchestrator {
    pub services: Arc<ServiceBundle>,
    pub tenants: Arc<TenantStore>,
}

pub struct DeployResult {
    pub tenant: Tenant,
    pub ssh_user: String,
    pub ssh_password_plain: String,
    pub vscode_password_plain: String,
}

impl StarterKitOrchestrator {
    pub fn new(services: Arc<ServiceBundle>, tenants: Arc<TenantStore>) -> Self {
        Self { services, tenants }
    }

    pub async fn deploy(&self, tenant_id: &str, user: &str) -> ServiceResult<DeployResult> {
        let mut tenant = self
            .tenants
            .get(tenant_id)?
            .ok_or_else(|| {
                crate::mec::services::ServiceError::NotFound(format!("tenant {}", tenant_id))
            })?;

        let ssh_user = format!("{}-dev", tenant_id);
        let ssh_password = random_password(16);
        let vscode_password = random_password(16);
        let ssh_hash = bcrypt::hash(&ssh_password, 10).map_err(|e| {
            crate::mec::services::ServiceError::Internal(format!("bcrypt: {}", e))
        })?;

        let kube = &self.services.kube;
        let (deploy_ssh, svc_ssh) =
            ubuntu_ssh_manifests(&tenant.namespace, tenant_id, &ssh_user, &ssh_hash);
        let (deploy_vs, svc_vs) =
            vscode_manifests(&tenant.namespace, tenant_id, &vscode_password);

        kube.apply_deployment(&tenant.namespace, &deploy_ssh.to_string())
            .await?;
        kube.apply_service(&tenant.namespace, &svc_ssh.to_string())
            .await?;
        kube.apply_deployment(&tenant.namespace, &deploy_vs.to_string())
            .await?;
        kube.apply_service(&tenant.namespace, &svc_vs.to_string())
            .await?;

        tenant.starter_kit = Some(StarterKit {
            ubuntu_ssh: StarterService {
                deployed: true,
                external_ip: None,
                port: 22,
                deployed_at: Some(Utc::now()),
            },
            vscode: StarterService {
                deployed: true,
                external_ip: None,
                port: 8080,
                deployed_at: Some(Utc::now()),
            },
            ssh_user: ssh_user.clone(),
            ssh_password_hash: Some(ssh_hash),
        });
        tenant.updated_at = Utc::now();
        self.tenants.upsert(&tenant, user).ok();

        Ok(DeployResult {
            tenant,
            ssh_user,
            ssh_password_plain: ssh_password,
            vscode_password_plain: vscode_password,
        })
    }

    pub async fn remove(&self, tenant_id: &str, user: &str) -> ServiceResult<()> {
        let mut tenant = self
            .tenants
            .get(tenant_id)?
            .ok_or_else(|| {
                crate::mec::services::ServiceError::NotFound(format!("tenant {}", tenant_id))
            })?;
        let kube = &self.services.kube;
        let _ = kube.delete_service(&tenant.namespace, "ubuntu-ssh-svc").await;
        let _ = kube.delete_service(&tenant.namespace, "vscode-svc").await;
        let _ = kube.delete_deployment(&tenant.namespace, "ubuntu-ssh").await;
        let _ = kube.delete_deployment(&tenant.namespace, "vscode").await;
        tenant.starter_kit = None;
        tenant.updated_at = Utc::now();
        self.tenants.upsert(&tenant, user).ok();
        Ok(())
    }
}

fn random_password(len: usize) -> String {
    const CHARSET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mec::state::MecState;
    use crate::models::mec::{AllocationType, NodeAllocation, ResourceQuota, TenantStatus};

    fn sample_tenant(id: &str) -> Tenant {
        Tenant {
            id: id.into(),
            display_name: id.into(),
            task_name: None,
            contact_email: None,
            namespace: id.into(),
            rancher_project_id: None,
            rancher_user_id: None,
            allocation: NodeAllocation {
                alloc_type: AllocationType::Shared,
                node: None,
                gpu_label: None,
                tier: None,
            },
            quota: ResourceQuota::standard(),
            starter_kit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: TenantStatus::Active,
        }
    }

    #[tokio::test]
    async fn deploy_and_remove() {
        let state = MecState::mocks();
        state
            .tenants
            .upsert(&sample_tenant("x"), "admin")
            .unwrap();
        let orch = StarterKitOrchestrator::new(state.services.clone(), state.tenants.clone());
        let res = orch.deploy("x", "admin").await.unwrap();
        assert_eq!(res.ssh_user, "x-dev");
        assert!(!res.ssh_password_plain.is_empty());
        let after = state.tenants.get("x").unwrap().unwrap();
        assert!(after.starter_kit.is_some());
        orch.remove("x", "admin").await.unwrap();
        let cleaned = state.tenants.get("x").unwrap().unwrap();
        assert!(cleaned.starter_kit.is_none());
    }

    #[test]
    fn password_length() {
        assert_eq!(random_password(16).len(), 16);
    }
}
