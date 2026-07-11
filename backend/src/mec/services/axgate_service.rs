use super::ServiceResult;
use crate::models::mec::{CreateNatRuleRequest, NatRule, PublicIp, SecurityPolicy, VpnSession};
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;

pub type AxgateServiceArc = Arc<dyn AxgateService + Send + Sync>;

#[async_trait]
pub trait AxgateService: Send + Sync {
    async fn list_public_ips(&self) -> ServiceResult<Vec<PublicIp>>;
    async fn list_nat_rules(&self) -> ServiceResult<Vec<NatRule>>;
    async fn add_nat_rule(&self, req: &CreateNatRuleRequest) -> ServiceResult<NatRule>;
    async fn delete_nat_rule(&self, id: &str) -> ServiceResult<()>;
    async fn toggle_nat_rule(&self, id: &str, enabled: bool) -> ServiceResult<()>;
    async fn list_security_policies(&self) -> ServiceResult<Vec<SecurityPolicy>>;
    async fn sync_running_config(&self) -> ServiceResult<SyncResult>;
    async fn health_check(&self) -> ServiceResult<AxgateHealth>;
    /// Active SSL-VPN user sessions. Returns an empty list when the source is
    /// not connected/enabled — never fabricates. The real probe is opt-in
    /// (`NABIMAN_MEC_AXGATE_VPN`) to avoid AXGATE's 600s CLI lockout.
    async fn list_vpn_sessions(&self) -> ServiceResult<Vec<VpnSession>>;
}

#[derive(Debug, Clone, Serialize)]
pub struct AxgateHealth {
    pub connected: bool,
    pub endpoint: String,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    pub nat_rules_count: usize,
    pub security_policies_count: usize,
    pub public_ips_count: usize,
    pub synced_at: chrono::DateTime<chrono::Utc>,
}
