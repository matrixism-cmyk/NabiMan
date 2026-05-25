use super::audit_logger::AuditLogger;
use super::job_runner::JobRunner;
use super::services::{ServiceBundle, ServiceResult};
use super::db::tenant_store::TenantStore;
use crate::models::mec::{
    AllocationType, CreateTenantRequest, JobKind, Tenant, TenantStatus, validate_tenant_id,
};
use chrono::Utc;
use std::sync::Arc;

pub struct TenantOrchestrator {
    pub services: Arc<ServiceBundle>,
    pub audit: Arc<AuditLogger>,
    pub jobs: Arc<JobRunner>,
    pub tenants: Arc<TenantStore>,
}

impl TenantOrchestrator {
    pub fn new(
        services: Arc<ServiceBundle>,
        audit: Arc<AuditLogger>,
        jobs: Arc<JobRunner>,
        tenants: Arc<TenantStore>,
    ) -> Self {
        Self {
            services,
            audit,
            jobs,
            tenants,
        }
    }

    pub async fn create(&self, req: CreateTenantRequest, user: &str) -> ServiceResult<Tenant> {
        validate_tenant_id(&req.tenant_id)
            .map_err(|e| super::services::ServiceError::InvalidInput(e.into()))?;

        let job = self.jobs.create(
            JobKind::TenantCreate {
                spec: serde_json::to_value(&req).unwrap_or_default(),
            },
            user,
            creation_steps(&req),
        );
        self.jobs.mark_started(&job.id);
        let mut steps = job.steps.clone();
        let audit = self
            .audit
            .begin(user, "tenant_create", "tenant", &req.tenant_id)
            .with_input(serde_json::to_value(&req).unwrap_or_default());

        match self.do_create(&req, &job.id, &mut steps).await {
            Ok(tenant) => {
                self.tenants.upsert(&tenant, user).ok();
                self.jobs.complete(
                    &job.id,
                    Some(serde_json::json!({"tenant_id": &tenant.id})),
                );
                audit.success(Some(serde_json::json!({"tenant_id": &tenant.id})));
                Ok(tenant)
            }
            Err(e) => {
                self.jobs.fail(&job.id, e.to_string());
                audit.failure(e.to_string());
                Err(e)
            }
        }
    }

    async fn do_create(
        &self,
        req: &CreateTenantRequest,
        job_id: &str,
        steps: &mut [crate::models::mec::JobStep],
    ) -> ServiceResult<Tenant> {
        let kube = &self.services.kube;
        let rancher = &self.services.rancher;

        // Safety: reject creation when a namespace already exists in the cluster.
        // Operators should use POST /tenants/{id}/import instead of overwriting
        // a live tenant's state.
        let existing = kube.list_namespaces().await?;
        if existing.iter().any(|n| n.name == req.tenant_id) {
            return Err(super::services::ServiceError::Conflict(format!(
                "namespace '{}' already exists in the cluster. Use POST /api/mec/v1/tenants/{}/import to adopt it.",
                req.tenant_id, req.tenant_id
            )));
        }

        self.jobs.mark_step_started(job_id, steps, "create_namespace");
        kube.create_namespace(
            &req.tenant_id,
            &[
                ("tenant".into(), req.tenant_id.clone()),
                ("managed-by".into(), "nabiman".into()),
            ],
        )
        .await?;
        self.jobs.mark_step_done(job_id, steps, "create_namespace", None);

        self.jobs.mark_step_started(job_id, steps, "apply_quota");
        kube.apply_resource_quota(&req.tenant_id, &req.quota).await?;
        self.jobs.mark_step_done(job_id, steps, "apply_quota", None);

        self.jobs.mark_step_started(job_id, steps, "apply_limit_range");
        let lr = crate::mec::policies::default_limit_range(&req.tenant_id, &req.quota);
        kube.apply_limit_range(&req.tenant_id, &lr.spec.to_string())
            .await?;
        self.jobs
            .mark_step_done(job_id, steps, "apply_limit_range", None);

        self.jobs.mark_step_started(job_id, steps, "apply_network_policies");
        for tpl in crate::mec::policies::default_tenant_policies(&req.tenant_id) {
            kube.apply_network_policy(&req.tenant_id, &tpl.name, &tpl.spec.to_string())
                .await?;
        }
        self.jobs.mark_step_done(
            job_id,
            steps,
            "apply_network_policies",
            Some("5 policies applied".into()),
        );

        self.jobs.mark_step_started(job_id, steps, "apply_rbac");
        kube.apply_rbac(&req.tenant_id, &req.tenant_id).await?;
        self.jobs.mark_step_done(job_id, steps, "apply_rbac", None);

        if let AllocationType::Dedicated = req.allocation.alloc_type {
            if let Some(node) = &req.allocation.node {
                self.jobs.mark_step_started(job_id, steps, "label_node");
                kube.patch_labels(
                    node,
                    &[("tenant".into(), req.tenant_id.clone())],
                    &[],
                )
                .await?;
                self.jobs.mark_step_done(job_id, steps, "label_node", None);
            }
        }

        self.jobs.mark_step_started(job_id, steps, "create_rancher_project");
        let project = rancher.create_project("c-local", &req.tenant_id).await?;
        self.jobs
            .mark_step_done(job_id, steps, "create_rancher_project", None);

        self.jobs.mark_step_started(job_id, steps, "create_rancher_user");
        let user = rancher
            .create_user(&format!("{}-dev", req.tenant_id), "changeme!")
            .await?;
        self.jobs
            .mark_step_done(job_id, steps, "create_rancher_user", None);

        self.jobs.mark_step_started(job_id, steps, "create_prtb");
        rancher
            .create_prtb(&project.id, &user.id, "project-owner")
            .await?;
        self.jobs.mark_step_done(job_id, steps, "create_prtb", None);

        Ok(Tenant {
            id: req.tenant_id.clone(),
            display_name: req.display_name.clone(),
            task_name: req.task_name.clone(),
            contact_email: req.contact_email.clone(),
            namespace: req.tenant_id.clone(),
            rancher_project_id: Some(project.id),
            rancher_user_id: Some(user.id),
            allocation: req.allocation.clone(),
            quota: req.quota.clone(),
            starter_kit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: TenantStatus::Active,
        })
    }

    pub async fn delete(&self, tenant_id: &str, user: &str) -> ServiceResult<()> {
        let tenant = self
            .tenants
            .get(tenant_id)?
            .ok_or_else(|| super::services::ServiceError::NotFound(format!("tenant {}", tenant_id)))?;

        let job = self.jobs.create(
            JobKind::TenantDelete {
                tenant_id: tenant_id.into(),
            },
            user,
            vec!["delete_rancher".into(), "delete_namespace".into()],
        );
        self.jobs.mark_started(&job.id);
        let mut steps = job.steps.clone();
        let audit = self
            .audit
            .begin(user, "tenant_delete", "tenant", tenant_id);

        self.jobs.mark_step_started(&job.id, &mut steps, "delete_rancher");
        if let Some(pid) = &tenant.rancher_project_id {
            let _ = self.services.rancher.delete_project(pid).await;
        }
        if let Some(uid) = &tenant.rancher_user_id {
            let _ = self.services.rancher.delete_user(uid).await;
        }
        self.jobs
            .mark_step_done(&job.id, &mut steps, "delete_rancher", None);

        self.jobs
            .mark_step_started(&job.id, &mut steps, "delete_namespace");
        let _ = self.services.kube.delete_namespace(&tenant.namespace).await;
        self.jobs
            .mark_step_done(&job.id, &mut steps, "delete_namespace", None);

        self.tenants.delete(tenant_id)?;
        self.jobs.complete(&job.id, None);
        audit.success(None);
        Ok(())
    }
}

fn creation_steps(req: &CreateTenantRequest) -> Vec<String> {
    let mut s = vec![
        "create_namespace".into(),
        "apply_quota".into(),
        "apply_limit_range".into(),
        "apply_network_policies".into(),
        "apply_rbac".into(),
    ];
    if matches!(req.allocation.alloc_type, AllocationType::Dedicated) && req.allocation.node.is_some() {
        s.push("label_node".into());
    }
    s.push("create_rancher_project".into());
    s.push("create_rancher_user".into());
    s.push("create_prtb".into());
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mec::state::MecState;
    use crate::models::mec::{
        AllocationType, NodeAllocation, ResourceQuota, TenantOptions,
    };

    fn sample_req() -> CreateTenantRequest {
        CreateTenantRequest {
            tenant_id: "testpoc".into(),
            display_name: "Test".into(),
            task_name: None,
            contact_email: None,
            allocation: NodeAllocation {
                alloc_type: AllocationType::Shared,
                node: None,
                gpu_label: None,
                tier: None,
            },
            quota: ResourceQuota::standard(),
            options: TenantOptions::default(),
        }
    }

    #[tokio::test]
    async fn create_roundtrip() {
        let state = MecState::mocks();
        let orch = TenantOrchestrator::new(
            state.services.clone(),
            state.audit.clone(),
            state.jobs.clone(),
            state.tenants.clone(),
        );
        let tenant = orch.create(sample_req(), "admin").await.unwrap();
        assert_eq!(tenant.id, "testpoc");
        assert_eq!(tenant.status, TenantStatus::Active);
        assert!(state.tenants.get("testpoc").unwrap().is_some());
    }

    #[tokio::test]
    async fn delete_removes_tenant() {
        let state = MecState::mocks();
        let orch = TenantOrchestrator::new(
            state.services.clone(),
            state.audit.clone(),
            state.jobs.clone(),
            state.tenants.clone(),
        );
        orch.create(sample_req(), "admin").await.unwrap();
        orch.delete("testpoc", "admin").await.unwrap();
        assert!(state.tenants.get("testpoc").unwrap().is_none());
    }
}
