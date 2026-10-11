use crate::mec::services::ServiceResult;
use crate::models::mec::{CreateNatRuleRequest, NatRule, PublicIp, SecurityPolicy, VpnSession};
use async_trait::async_trait;

use super::UnavailableService;
use crate::mec::services::axgate_service::*;

#[async_trait]
impl AxgateService for UnavailableService {
    async fn list_public_ips(&self) -> ServiceResult<Vec<PublicIp>> {
        self.reject()
    }

    async fn list_nat_rules(&self) -> ServiceResult<Vec<NatRule>> {
        self.reject()
    }

    async fn add_nat_rule(&self, _req: &CreateNatRuleRequest) -> ServiceResult<NatRule> {
        self.reject()
    }

    async fn delete_nat_rule(&self, _id: &str) -> ServiceResult<()> {
        self.reject()
    }

    async fn toggle_nat_rule(&self, _id: &str, _enabled: bool) -> ServiceResult<()> {
        self.reject()
    }

    async fn list_security_policies(&self) -> ServiceResult<Vec<SecurityPolicy>> {
        self.reject()
    }

    async fn sync_running_config(&self) -> ServiceResult<SyncResult> {
        self.reject()
    }

    async fn health_check(&self) -> ServiceResult<AxgateHealth> {
        self.reject()
    }

    async fn list_vpn_sessions(&self) -> ServiceResult<Vec<VpnSession>> {
        self.reject()
    }
}
