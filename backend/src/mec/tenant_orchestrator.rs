use super::audit_logger::AuditLogger;
use super::db::tenant_store::TenantStore;
use super::job_runner::JobRunner;
use super::services::{ServiceBundle, ServiceResult};
use crate::models::mec::{
    validate_tenant_id, AllocationType, CreateTenantRequest, JobKind, Tenant, TenantStatus,
};
use chrono::Utc;
use std::sync::Arc;

pub struct CreatedTenant {
    pub tenant: Tenant,
    pub initial_credentials: super::credentials::InitialCredentials,
}

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

    #[cfg(test)]
    pub async fn create(&self, req: CreateTenantRequest, user: &str) -> ServiceResult<Tenant> {
        self.create_with_credentials(req, user)
            .await
            .map(|created| created.tenant)
    }

    pub async fn create_with_credentials(
        &self,
        req: CreateTenantRequest,
        user: &str,
    ) -> ServiceResult<CreatedTenant> {
        validate_tenant_id(&req.tenant_id)
            .map_err(|e| super::services::ServiceError::InvalidInput(e.into()))?;

        let bootstrap_password = super::credentials::generate_password()?;
        let job = self.jobs.create(
            JobKind::TenantCreate {
                spec: serde_json::to_value(&req).unwrap_or_default(),
            },
            user,
            creation_steps(&req),
        )?;
        self.jobs.mark_started(&job.id)?;
        let mut steps = job.steps.clone();
        let audit = self
            .audit
            .begin(user, "tenant_create", "tenant", &req.tenant_id)
            .with_input(serde_json::to_value(&req).unwrap_or_default());

        let mut tenant = Tenant {
            id: req.tenant_id.clone(),
            display_name: req.display_name.clone(),
            task_name: req.task_name.clone(),
            contact_email: req.contact_email.clone(),
            namespace: req.tenant_id.clone(),
            rancher_project_id: None,
            rancher_user_id: None,
            allocation: req.allocation.clone(),
            quota: req.quota.clone(),
            starter_kit: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: TenantStatus::Provisioning,
        };
        let mut reserved = false;
        let result: ServiceResult<()> = async {
            let existing = self.services.kube.list_namespaces().await?;
            if existing.iter().any(|n| n.name == req.tenant_id) {
                return Err(super::services::ServiceError::Conflict(format!(
                    "namespace '{}' already exists; verify ownership before importing it", req.tenant_id
                )));
            }
            if !self.tenants.reserve(&tenant, user)? {
                return Err(super::services::ServiceError::Conflict(format!(
                    "tenant '{}' already has a profile; reconcile existing resources before recreating it", req.tenant_id
                )));
            }
            reserved = true;
            self.do_create(&req, &job.id, &mut steps, &mut tenant, user, &bootstrap_password).await?;
            tenant.status = TenantStatus::Active;
            tenant.updated_at = Utc::now();
            self.jobs.complete_tenant(
                &job.id, super::db::job_store::TenantCompletion::Create(&tenant),
                Some(serde_json::json!({"tenant_id": &tenant.id})),
            )?;
            Ok(())
        }.await;
        match result {
            Ok(()) => {
                audit.success(Some(serde_json::json!({"tenant_id": &tenant.id})));
                Ok(CreatedTenant {
                    initial_credentials: super::credentials::InitialCredentials {
                        username: format!("{}-dev", tenant.id),
                        password: bootstrap_password,
                    },
                    tenant,
                })
            }
            Err(e) => {
                let e = if reserved {
                    tenant.status = TenantStatus::Failed {
                        reason: format!("Creation requires reconciliation: {e}"),
                    };
                    tenant.updated_at = Utc::now();
                    match self.tenants.upsert(&tenant, user) {
                        Ok(()) => e,
                        Err(save_error) => super::services::ServiceError::Internal(format!("Creation failed: {e}; resource checkpoint could not be saved: {save_error}")),
                    }
                } else {
                    e
                };
                let recovery = reserved.then(|| serde_json::json!({"tenant": &tenant}));
                if let Err(save_error) =
                    self.jobs.fail_with_result(&job.id, e.to_string(), recovery)
                {
                    let message = format!(
                        "Operation failed: {e}; job failure could not be saved: {save_error}"
                    );
                    audit.failure(message.clone());
                    return Err(super::services::ServiceError::Internal(message));
                }
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
        tenant: &mut Tenant,
        actor: &str,
        bootstrap_password: &str,
    ) -> ServiceResult<()> {
        let kube = &self.services.kube;
        let rancher = &self.services.rancher;

        self.jobs
            .mark_step_started(job_id, steps, "create_namespace")?;
        kube.create_namespace(
            &req.tenant_id,
            &[
                ("tenant".into(), req.tenant_id.clone()),
                ("managed-by".into(), "nabiman".into()),
            ],
        )
        .await?;
        self.jobs
            .mark_step_done(job_id, steps, "create_namespace", None)?;

        self.jobs.mark_step_started(job_id, steps, "apply_quota")?;
        kube.apply_resource_quota(&req.tenant_id, &req.quota)
            .await?;
        self.jobs
            .mark_step_done(job_id, steps, "apply_quota", None)?;

        self.jobs
            .mark_step_started(job_id, steps, "apply_limit_range")?;
        let lr = crate::mec::policies::default_limit_range(&req.tenant_id, &req.quota);
        kube.apply_limit_range(&req.tenant_id, &lr.spec.to_string())
            .await?;
        self.jobs
            .mark_step_done(job_id, steps, "apply_limit_range", None)?;

        self.jobs
            .mark_step_started(job_id, steps, "apply_network_policies")?;
        for tpl in crate::mec::policies::default_tenant_policies(&req.tenant_id) {
            kube.apply_network_policy(&req.tenant_id, &tpl.name, &tpl.spec.to_string())
                .await?;
        }
        self.jobs.mark_step_done(
            job_id,
            steps,
            "apply_network_policies",
            Some("5 policies applied".into()),
        )?;

        self.jobs.mark_step_started(job_id, steps, "apply_rbac")?;
        kube.apply_rbac(&req.tenant_id, &req.tenant_id).await?;
        self.jobs
            .mark_step_done(job_id, steps, "apply_rbac", None)?;

        if let AllocationType::Dedicated = req.allocation.alloc_type {
            if let Some(node) = &req.allocation.node {
                self.jobs.mark_step_started(job_id, steps, "label_node")?;
                kube.patch_labels(node, &[("tenant".into(), req.tenant_id.clone())], &[])
                    .await?;
                self.jobs
                    .mark_step_done(job_id, steps, "label_node", None)?;
            }
        }

        self.jobs
            .mark_step_started(job_id, steps, "create_rancher_project")?;
        let project = rancher.create_project("c-local", &req.tenant_id).await?;
        tenant.rancher_project_id = Some(project.id.clone());
        tenant.updated_at = Utc::now();
        self.tenants.upsert(tenant, actor)?;
        self.jobs
            .mark_step_done(job_id, steps, "create_rancher_project", None)?;

        self.jobs
            .mark_step_started(job_id, steps, "create_rancher_user")?;
        let user = rancher
            .create_user(&format!("{}-dev", req.tenant_id), bootstrap_password)
            .await
            .map_err(super::credentials::safe_error)?;
        tenant.rancher_user_id = Some(user.id.clone());
        tenant.updated_at = Utc::now();
        self.tenants.upsert(tenant, actor)?;
        self.jobs
            .mark_step_done(job_id, steps, "create_rancher_user", None)?;

        self.jobs.mark_step_started(job_id, steps, "create_prtb")?;
        rancher
            .create_prtb(&project.id, &user.id, "project-owner")
            .await?;
        self.jobs
            .mark_step_done(job_id, steps, "create_prtb", None)?;

        Ok(())
    }

    pub async fn delete(&self, tenant_id: &str, user: &str) -> ServiceResult<()> {
        let mut tenant = self.tenants.get(tenant_id)?.ok_or_else(|| {
            super::services::ServiceError::NotFound(format!("tenant {}", tenant_id))
        })?;

        if matches!(
            tenant.status,
            TenantStatus::Provisioning | TenantStatus::Terminating
        ) {
            return Err(super::services::ServiceError::Conflict(
                "Tenant has an in-progress operation; reconcile it before deletion".into(),
            ));
        }

        let job = self.jobs.create(
            JobKind::TenantDelete {
                tenant_id: tenant_id.into(),
            },
            user,
            vec!["delete_rancher".into(), "delete_namespace".into()],
        )?;
        self.jobs.mark_started(&job.id)?;
        let mut steps = job.steps.clone();
        let audit = self.audit.begin(user, "tenant_delete", "tenant", tenant_id);

        let result: ServiceResult<()> = async {
            tenant.status = TenantStatus::Terminating;
            self.tenants.upsert(&tenant, user)?;
            self.jobs
                .mark_step_started(&job.id, &mut steps, "delete_rancher")?;
            if let Some(pid) = &tenant.rancher_project_id {
                self.services.rancher.delete_project(pid).await?;
            }
            if let Some(uid) = &tenant.rancher_user_id {
                self.services.rancher.delete_user(uid).await?;
            }
            self.jobs
                .mark_step_done(&job.id, &mut steps, "delete_rancher", None)?;

            self.jobs
                .mark_step_started(&job.id, &mut steps, "delete_namespace")?;
            self.services
                .kube
                .delete_namespace(&tenant.namespace)
                .await?;
            self.jobs
                .mark_step_done(&job.id, &mut steps, "delete_namespace", None)?;

            self.jobs.complete_tenant(
                &job.id,
                super::db::job_store::TenantCompletion::Delete(tenant_id),
                None,
            )?;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            tenant.status = TenantStatus::Failed {
                reason: format!("Deletion requires reconciliation: {error}"),
            };
            let error = match self.tenants.upsert(&tenant, user) {
                Ok(()) => error,
                Err(save_error) => super::services::ServiceError::Internal(format!("Deletion failed: {error}; tenant failure state could not be saved: {save_error}")),
            };
            let final_error = match self.jobs.fail(&job.id, error.to_string()) {
                Ok(()) => error,
                Err(save_error) => super::services::ServiceError::Internal(format!(
                    "Operation failed: {error}; job failure could not be saved: {save_error}"
                )),
            };
            audit.failure(final_error.to_string());
            return Err(final_error);
        }
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
    if matches!(req.allocation.alloc_type, AllocationType::Dedicated)
        && req.allocation.node.is_some()
    {
        s.push("label_node".into());
    }
    s.push("create_rancher_project".into());
    s.push("create_rancher_user".into());
    s.push("create_prtb".into());
    s
}

#[cfg(test)]
#[path = "tenant_orchestrator_tests.rs"]
mod tests;
