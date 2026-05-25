use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LbIpPool {
    pub name: String,
    pub address_ranges: Vec<String>,
    pub total_ips: u32,
    pub used_ips: u32,
    pub auto_assign: bool,
}

impl LbIpPool {
    pub fn free_ips(&self) -> u32 {
        self.total_ips.saturating_sub(self.used_ips)
    }

    pub fn usage_percent(&self) -> f32 {
        if self.total_ips == 0 {
            0.0
        } else {
            (self.used_ips as f32 / self.total_ips as f32) * 100.0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerService {
    pub namespace: String,
    pub name: String,
    pub external_ip: String,
    pub ports: Vec<ServicePort>,
    pub selector_summary: String,
    pub age_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePort {
    pub name: Option<String>,
    pub port: u16,
    pub target_port: u16,
    pub node_port: Option<u16>,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressRule {
    pub namespace: String,
    pub name: String,
    pub class: Option<String>,
    pub host: Option<String>,
    pub paths: Vec<IngressPath>,
    pub tls: Vec<IngressTls>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressPath {
    pub path: String,
    pub path_type: String,
    pub backend_service: String,
    pub backend_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressTls {
    pub hosts: Vec<String>,
    pub secret_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIngressRequest {
    pub namespace: String,
    pub name: String,
    pub host: String,
    pub paths: Vec<IngressPath>,
    #[serde(default)]
    pub tls_secret_name: Option<String>,
    #[serde(default = "default_ingress_class")]
    pub class: String,
}

fn default_ingress_class() -> String {
    "nginx".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendPoolRequest {
    pub additional_ranges: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_usage() {
        let p = LbIpPool {
            name: "tenant".into(),
            address_ranges: vec![],
            total_ips: 40,
            used_ips: 10,
            auto_assign: true,
        };
        assert_eq!(p.free_ips(), 30);
        assert!((p.usage_percent() - 25.0).abs() < 0.01);
    }

    #[test]
    fn pool_zero_total() {
        let p = LbIpPool {
            name: "empty".into(),
            address_ranges: vec![],
            total_ips: 0,
            used_ips: 0,
            auto_assign: false,
        };
        assert_eq!(p.usage_percent(), 0.0);
    }
}
