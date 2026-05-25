use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub display_name: String,
    pub task_name: Option<String>,
    pub contact_email: Option<String>,

    pub namespace: String,
    pub rancher_project_id: Option<String>,
    pub rancher_user_id: Option<String>,

    pub allocation: NodeAllocation,
    pub quota: ResourceQuota,
    pub starter_kit: Option<StarterKit>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: TenantStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum TenantStatus {
    Provisioning,
    Active,
    Terminating,
    Failed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAllocation {
    #[serde(rename = "type")]
    pub alloc_type: AllocationType,
    pub node: Option<String>,
    pub gpu_label: Option<String>,
    pub tier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AllocationType {
    Dedicated,
    Shared,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    pub cpu_requests: String,
    pub cpu_limits: String,
    pub memory_requests: String,
    pub memory_limits: String,
    pub gpu: u32,
    pub storage: String,
    pub pods: u32,
    pub pvcs: u32,
    pub lb_services: u32,
}

impl ResourceQuota {
    pub fn standard() -> Self {
        Self {
            cpu_requests: "32".to_string(),
            cpu_limits: "64".to_string(),
            memory_requests: "64Gi".to_string(),
            memory_limits: "128Gi".to_string(),
            gpu: 2,
            storage: "500Gi".to_string(),
            pods: 100,
            pvcs: 20,
            lb_services: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarterKit {
    pub ubuntu_ssh: StarterService,
    pub vscode: StarterService,
    pub ssh_user: String,
    pub ssh_password_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarterService {
    pub deployed: bool,
    pub external_ip: Option<String>,
    pub port: u16,
    pub deployed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTenantRequest {
    pub tenant_id: String,
    pub display_name: String,
    pub task_name: Option<String>,
    pub contact_email: Option<String>,
    pub allocation: NodeAllocation,
    pub quota: ResourceQuota,
    pub options: TenantOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TenantOptions {
    #[serde(default)]
    pub deploy_starter_kit: bool,
    #[serde(default)]
    pub create_harbor_project: bool,
    #[serde(default)]
    pub generate_guide: bool,
    #[serde(default)]
    pub include_egress_policy: bool,
    pub ingress_domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateTenantRequest {
    pub display_name: Option<String>,
    pub task_name: Option<String>,
    pub contact_email: Option<String>,
    pub quota: Option<ResourceQuota>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSummary {
    pub id: String,
    pub display_name: String,
    pub namespace: String,
    pub node: Option<String>,
    pub gpu_label: Option<String>,
    pub cpu_used: String,
    pub cpu_limit: String,
    pub mem_used: String,
    pub mem_limit: String,
    pub pods_used: u32,
    pub pods_limit: u32,
    pub status: TenantStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaUsage {
    pub tenant_id: String,
    pub cpu_requests_used: String,
    pub cpu_requests_hard: String,
    pub cpu_limits_used: String,
    pub cpu_limits_hard: String,
    pub memory_requests_used: String,
    pub memory_requests_hard: String,
    pub memory_limits_used: String,
    pub memory_limits_hard: String,
    pub gpu_used: u32,
    pub gpu_hard: u32,
    pub pods_used: u32,
    pub pods_hard: u32,
    pub lb_services_used: u32,
    pub lb_services_hard: u32,
    pub storage_used: String,
    pub storage_hard: String,
}

pub fn validate_tenant_id(id: &str) -> Result<(), &'static str> {
    if id.is_empty() {
        return Err("tenant_id cannot be empty");
    }
    if id.len() > 63 {
        return Err("tenant_id must be 63 characters or fewer");
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("tenant_id must contain only lowercase letters, digits, or hyphens");
    }
    let first = id.chars().next().unwrap();
    let last = id.chars().last().unwrap();
    if !first.is_ascii_alphabetic() || !last.is_ascii_alphanumeric() {
        return Err("tenant_id must start with a letter and end with alphanumeric");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_ok() {
        assert!(validate_tenant_id("ygram-poc").is_ok());
    }

    #[test]
    fn validate_empty() {
        assert!(validate_tenant_id("").is_err());
    }

    #[test]
    fn validate_uppercase() {
        assert!(validate_tenant_id("YGram").is_err());
    }

    #[test]
    fn validate_leading_hyphen() {
        assert!(validate_tenant_id("-foo").is_err());
    }

    #[test]
    fn validate_too_long() {
        let s = "a".repeat(64);
        assert!(validate_tenant_id(&s).is_err());
    }

    #[test]
    fn standard_quota() {
        let q = ResourceQuota::standard();
        assert_eq!(q.cpu_requests, "32");
        assert_eq!(q.gpu, 2);
    }
}
