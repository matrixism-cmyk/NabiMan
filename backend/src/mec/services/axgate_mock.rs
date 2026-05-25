use super::axgate_service::{AxgateHealth, AxgateService, SyncResult};
use super::{ServiceError, ServiceResult};
use crate::models::mec::{
    AnyMarker, CreateNatRuleRequest, NatRule, NatSource, NatType, PolicyAction, Protocol,
    PublicIp, PublicIpStatus, SecurityPolicy,
};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Mutex;

pub struct AxgateMock {
    state: Mutex<State>,
}

struct State {
    public_ips: Vec<PublicIp>,
    nat_rules: Vec<NatRule>,
    policies: Vec<SecurityPolicy>,
    next_id: u32,
}

impl Default for AxgateMock {
    fn default() -> Self {
        Self::new()
    }
}

impl AxgateMock {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                public_ips: default_public_ips(),
                nat_rules: default_rules(),
                policies: vec![],
                next_id: 20,
            }),
        }
    }
}

fn default_public_ips() -> Vec<PublicIp> {
    let base = "121.147.13.";
    let mut v = vec![PublicIp {
        ip: format!("{}228", base),
        assigned_to: Some("axgate".into()),
        proxy_arp_enabled: false,
        status: PublicIpStatus::SystemReserved,
    }];
    for i in 230..=248u32 {
        v.push(PublicIp {
            ip: format!("{}{}", base, i),
            assigned_to: None,
            proxy_arp_enabled: false,
            status: PublicIpStatus::Available,
        });
    }
    v
}

fn default_rules() -> Vec<NatRule> {
    vec![NatRule {
        id: "nat-10".into(),
        from_zone: "untrust".into(),
        to_zone: "trust".into(),
        rule_type: NatType::Dnat,
        label: Some("Rancher HTTPS".into()),
        source: NatSource::Any(AnyMarker { any: true }),
        destination: vec!["121.147.13.233/32".into()],
        service_groups: vec!["TCP_443".into()],
        translated_to: "172.20.26.230".into(),
        enabled: true,
        hits: 42,
        last_hit: None,
        created_at: Utc::now(),
    }]
}

#[async_trait]
impl AxgateService for AxgateMock {
    async fn list_public_ips(&self) -> ServiceResult<Vec<PublicIp>> {
        Ok(self.state.lock().unwrap().public_ips.clone())
    }

    async fn list_nat_rules(&self) -> ServiceResult<Vec<NatRule>> {
        Ok(self.state.lock().unwrap().nat_rules.clone())
    }

    async fn add_nat_rule(&self, req: &CreateNatRuleRequest) -> ServiceResult<NatRule> {
        let mut st = self.state.lock().unwrap();
        st.next_id += 1;
        let service_groups = req
            .ports
            .iter()
            .map(|p| format!("{}_{}", protocol_prefix(&req.protocol), p))
            .collect::<Vec<_>>();
        let rule = NatRule {
            id: format!("nat-{}", st.next_id),
            from_zone: "untrust".into(),
            to_zone: "trust".into(),
            rule_type: NatType::Dnat,
            label: Some(req.label.clone()),
            source: if req.source == "any" {
                NatSource::Any(AnyMarker { any: true })
            } else {
                NatSource::Groups {
                    groups: vec![req.source.clone()],
                }
            },
            destination: vec![format!("{}/32", req.public_ip)],
            service_groups,
            translated_to: req.private_ip.clone(),
            enabled: req.enable_immediately,
            hits: 0,
            last_hit: None,
            created_at: Utc::now(),
        };
        st.nat_rules.push(rule.clone());
        if let Some(ip) = st.public_ips.iter_mut().find(|p| p.ip == req.public_ip) {
            ip.assigned_to = Some(req.label.clone());
            ip.status = PublicIpStatus::Assigned;
            if req.create_proxy_arp {
                ip.proxy_arp_enabled = true;
            }
        }
        Ok(rule)
    }

    async fn delete_nat_rule(&self, id: &str) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let before = st.nat_rules.len();
        let removed: Vec<_> = st
            .nat_rules
            .iter()
            .filter(|r| r.id == id)
            .cloned()
            .collect();
        st.nat_rules.retain(|r| r.id != id);
        if st.nat_rules.len() == before {
            return Err(ServiceError::NotFound(format!("nat rule {}", id)));
        }
        for rule in removed {
            if let Some(dest) = rule.destination.first() {
                let ip = dest.trim_end_matches("/32");
                if let Some(p) = st.public_ips.iter_mut().find(|p| p.ip == ip) {
                    p.assigned_to = None;
                    p.proxy_arp_enabled = false;
                    p.status = PublicIpStatus::Available;
                }
            }
        }
        Ok(())
    }

    async fn toggle_nat_rule(&self, id: &str, enabled: bool) -> ServiceResult<()> {
        let mut st = self.state.lock().unwrap();
        let r = st
            .nat_rules
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| ServiceError::NotFound(format!("nat rule {}", id)))?;
        r.enabled = enabled;
        Ok(())
    }

    async fn list_security_policies(&self) -> ServiceResult<Vec<SecurityPolicy>> {
        Ok(self.state.lock().unwrap().policies.clone())
    }

    async fn sync_running_config(&self) -> ServiceResult<SyncResult> {
        let st = self.state.lock().unwrap();
        Ok(SyncResult {
            nat_rules_count: st.nat_rules.len(),
            security_policies_count: st.policies.len(),
            public_ips_count: st.public_ips.len(),
            synced_at: Utc::now(),
        })
    }

    async fn health_check(&self) -> ServiceResult<AxgateHealth> {
        Ok(AxgateHealth {
            connected: true,
            endpoint: "mock://axgate".into(),
            latency_ms: Some(1),
            error: None,
        })
    }
}

fn protocol_prefix(p: &Protocol) -> &'static str {
    match p {
        Protocol::Tcp => "TCP",
        Protocol::Udp => "UDP",
        Protocol::Icmp => "ICMP",
    }
}

impl From<&PolicyAction> for &'static str {
    fn from(a: &PolicyAction) -> Self {
        match a {
            PolicyAction::Pass => "pass",
            PolicyAction::Drop => "drop",
            PolicyAction::Reject => "reject",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_req() -> CreateNatRuleRequest {
        CreateNatRuleRequest {
            label: "test".into(),
            public_ip: "121.147.13.246".into(),
            private_ip: "172.20.26.199".into(),
            protocol: Protocol::Tcp,
            ports: vec![22, 8080],
            source: "any".into(),
            create_proxy_arp: true,
            enable_immediately: true,
        }
    }

    #[tokio::test]
    async fn add_list_delete_nat() {
        let m = AxgateMock::new();
        let before = m.list_nat_rules().await.unwrap().len();
        let rule = m.add_nat_rule(&sample_req()).await.unwrap();
        assert!(rule.enabled);
        let list = m.list_nat_rules().await.unwrap();
        assert_eq!(list.len(), before + 1);
        m.delete_nat_rule(&rule.id).await.unwrap();
        assert_eq!(m.list_nat_rules().await.unwrap().len(), before);
    }

    #[tokio::test]
    async fn add_marks_public_ip_assigned() {
        let m = AxgateMock::new();
        m.add_nat_rule(&sample_req()).await.unwrap();
        let ips = m.list_public_ips().await.unwrap();
        let ip = ips.iter().find(|p| p.ip == "121.147.13.246").unwrap();
        assert_eq!(ip.status, PublicIpStatus::Assigned);
        assert!(ip.proxy_arp_enabled);
    }
}
