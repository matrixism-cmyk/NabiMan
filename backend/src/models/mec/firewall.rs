use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicIp {
    pub ip: String,
    pub assigned_to: Option<String>,
    pub proxy_arp_enabled: bool,
    pub status: PublicIpStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PublicIpStatus {
    Available,
    Assigned,
    SystemReserved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatRule {
    pub id: String,
    pub from_zone: String,
    pub to_zone: String,
    pub rule_type: NatType,
    pub label: Option<String>,
    pub source: NatSource,
    pub destination: Vec<String>,
    pub service_groups: Vec<String>,
    pub translated_to: String,
    pub enabled: bool,
    pub hits: u64,
    pub last_hit: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NatType {
    Dnat,
    Snat,
    Pat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NatSource {
    Any(AnyMarker),
    Groups { groups: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnyMarker {
    pub any: bool,
}

impl Default for NatSource {
    fn default() -> Self {
        Self::Any(AnyMarker { any: true })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub id: String,
    pub from_zone: String,
    pub to_zone: String,
    pub action: PolicyAction,
    pub destination_group: Option<String>,
    pub service_groups: Vec<String>,
    pub label: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyAction {
    Pass,
    Drop,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNatRuleRequest {
    pub label: String,
    pub public_ip: String,
    pub private_ip: String,
    pub protocol: Protocol,
    pub ports: Vec<u16>,
    #[serde(default = "default_source_any")]
    pub source: String,
    #[serde(default)]
    pub create_proxy_arp: bool,
    #[serde(default = "default_true")]
    pub enable_immediately: bool,
}

fn default_source_any() -> String {
    "any".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
            Self::Icmp => "icmp",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNatRuleRequest {
    pub label: Option<String>,
    pub enabled: Option<bool>,
    pub ports: Option<Vec<u16>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_str() {
        assert_eq!(Protocol::Tcp.as_str(), "tcp");
    }

    #[test]
    fn nat_source_default() {
        if let NatSource::Any(m) = NatSource::default() {
            assert!(m.any);
        } else {
            panic!("expected Any");
        }
    }
}
