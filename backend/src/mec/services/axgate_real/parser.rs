use crate::models::mec::{
    AnyMarker, NatRule, NatSource, NatType, PolicyAction, PublicIp, PublicIpStatus, SecurityPolicy,
    VpnSession,
};
use chrono::Utc;

/// Parses `show running-config` output from AXGATE.
///
/// Real format verified against JCIA AXGATE (aos v2.1, 2026-04-21):
/// ```
/// ip nat policy from untrust to trust 10 id 14
///  label Rancher HTTPS
///  source any
///  destination 121.147.13.233/32
///  service-group ssh
///  nat-type dnat static 172.20.26.230/32
///  enable
/// !
/// ```
pub fn parse_nat_rules(config: &str) -> Vec<NatRule> {
    parse_blocks(config, "ip nat policy from ", finalize_nat)
}

pub fn parse_security_policies(config: &str) -> Vec<SecurityPolicy> {
    parse_blocks(config, "ip security policy from ", finalize_policy)
}

/// Parses proxy-arp aliases as public IP entries.
pub fn parse_public_ips(config: &str) -> Vec<PublicIp> {
    let mut out = Vec::new();
    for line in config.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("ip proxy-arp alias ") {
            if let Some(ip) = rest.split_whitespace().next() {
                out.push(PublicIp {
                    ip: ip.trim_end_matches('/').to_string(),
                    assigned_to: None,
                    proxy_arp_enabled: true,
                    status: PublicIpStatus::Assigned,
                });
            }
        }
    }
    out
}

fn parse_blocks<T>(
    config: &str,
    header_prefix: &str,
    finalize: fn(Block) -> Option<T>,
) -> Vec<T> {
    let mut rules = Vec::new();
    let mut current: Option<Block> = None;
    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed == "!" {
            if let Some(b) = current.take() {
                if let Some(r) = finalize(b) {
                    rules.push(r);
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix(header_prefix) {
            if let Some(b) = current.take() {
                if let Some(r) = finalize(b) {
                    rules.push(r);
                }
            }
            current = Block::from_header(header_prefix, rest);
        } else if let Some(ref mut b) = current {
            b.attributes.push(trimmed.to_string());
        }
    }
    if let Some(b) = current.take() {
        if let Some(r) = finalize(b) {
            rules.push(r);
        }
    }
    rules
}

struct Block {
    from_zone: String,
    to_zone: String,
    index: String,
    id: Option<String>,
    attributes: Vec<String>,
}

impl Block {
    fn from_header(prefix: &str, rest: &str) -> Option<Self> {
        // Header format: "from <ZONE> to <ZONE> <INDEX> [id <ID>]"
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() < 3 {
            return None;
        }
        let from_zone = parts[0].to_string();
        let to_zone = parts[2].to_string();
        let index = parts.get(3).copied().unwrap_or("?").to_string();
        let id = parts
            .iter()
            .position(|&p| p == "id")
            .and_then(|i| parts.get(i + 1))
            .map(|s| s.to_string());
        let _ = prefix;
        Some(Block {
            from_zone,
            to_zone,
            index,
            id,
            attributes: Vec::new(),
        })
    }

    fn attr_rest(&self, key: &str) -> Option<&str> {
        let prefix = format!("{} ", key);
        self.attributes
            .iter()
            .find_map(|a| a.strip_prefix(&prefix))
    }

    fn attr_exists(&self, key: &str) -> bool {
        self.attributes.iter().any(|a| a.trim() == key)
    }
}

fn finalize_nat(b: Block) -> Option<NatRule> {
    let id = format!("nat-{}", b.index);
    let label = b.attr_rest("label").map(str::to_string);
    let source = parse_source(&b);
    let destination = b
        .attr_rest("destination")
        .map(|s| vec![s.to_string()])
        .unwrap_or_default();
    let service_groups = b
        .attributes
        .iter()
        .filter_map(|a| a.strip_prefix("service-group "))
        .map(str::to_string)
        .collect();
    let (rule_type, translated_to) = parse_nat_type(&b);
    // AXGATE default is enabled; only explicit `no enable` flips it off.
    let enabled = !b.attr_exists("no enable");
    Some(NatRule {
        id,
        from_zone: b.from_zone,
        to_zone: b.to_zone,
        rule_type,
        label,
        source,
        destination,
        service_groups,
        translated_to,
        enabled,
        hits: 0,
        last_hit: None,
        created_at: Utc::now(),
    })
}

fn parse_source(b: &Block) -> NatSource {
    if let Some(rest) = b.attr_rest("source") {
        let rest = rest.trim();
        if rest == "any" {
            return NatSource::Any(AnyMarker { any: true });
        }
        // "source group XYZ" — keep "group XYZ" as identifier for readability
        return NatSource::Groups {
            groups: vec![rest.to_string()],
        };
    }
    NatSource::Any(AnyMarker { any: true })
}

fn parse_nat_type(b: &Block) -> (NatType, String) {
    // nat-type examples:
    //   "dnat static 172.20.26.230/32"
    //   "snat static 121.147.13.229/32"
    //   "snat dynamic auto pat rr ip-dist hash"
    if let Some(rest) = b.attr_rest("nat-type") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        match parts.as_slice() {
            ["dnat", "static", ip, ..] => {
                return (NatType::Dnat, ip.to_string());
            }
            ["snat", "static", ip, ..] => {
                return (NatType::Snat, ip.to_string());
            }
            ["snat", "dynamic", ..] | ["pat", ..] => {
                return (NatType::Pat, String::new());
            }
            _ => {}
        }
    }
    (NatType::Dnat, String::new())
}

fn finalize_policy(b: Block) -> Option<SecurityPolicy> {
    let _id = b.id.clone();
    let id = format!("sec-{}", b.index);
    let label = b.attr_rest("label").map(str::to_string);
    let action = if b.attr_exists("action drop") {
        PolicyAction::Drop
    } else if b.attr_exists("action reject") {
        PolicyAction::Reject
    } else {
        PolicyAction::Pass
    };
    let destination_group = b
        .attr_rest("destination")
        .map(|s| s.trim().to_string())
        .filter(|s| s != "any");
    let service_groups = b
        .attributes
        .iter()
        .filter_map(|a| a.strip_prefix("service-group "))
        .map(str::to_string)
        .collect();
    // AXGATE default is enabled; only explicit `no enable` flips it off.
    let enabled = !b.attr_exists("no enable");
    Some(SecurityPolicy {
        id,
        from_zone: b.from_zone,
        to_zone: b.to_zone,
        action,
        destination_group,
        service_groups,
        label,
        enabled,
    })
}

fn looks_like_ipv4(s: &str) -> bool {
    let mut parts = 0;
    for p in s.split('.') {
        if p.is_empty() || p.len() > 3 || !p.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        if p.parse::<u8>().is_err() {
            return false;
        }
        parts += 1;
    }
    parts == 4
}

/// Best-effort parse of `show sslvpn tunnel` output into VPN sessions.
///
/// PROVISIONAL: the exact AXGATE column layout is unconfirmed (see the
/// axgate-vpn-cli note). We conservatively treat any line that has a
/// non-IP first token plus at least one IPv4 as a session, and return an
/// empty list otherwise — we never invent rows. Refine once a live capture
/// pins down the format.
pub fn parse_vpn_sessions(output: &str) -> Vec<VpnSession> {
    let mut out = Vec::new();
    for line in output.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 2 || looks_like_ipv4(fields[0]) {
            continue;
        }
        let ips: Vec<&str> = fields.iter().copied().filter(|f| looks_like_ipv4(f)).collect();
        if ips.is_empty() {
            continue;
        }
        out.push(VpnSession {
            user: fields[0].to_string(),
            ip: ips[0].to_string(),
            source_ip: ips.get(1).unwrap_or(&"").to_string(),
            connected_since: None,
            state: "connected".into(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vpn_sessions_conservatively() {
        let sample = "User        VirtualIP     ClientIP\npoc-user01  10.8.0.12     203.0.113.24\nops-admin   10.8.0.5      198.51.100.7\n";
        let s = parse_vpn_sessions(sample);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].user, "poc-user01");
        assert_eq!(s[0].ip, "10.8.0.12");
        assert_eq!(s[0].source_ip, "203.0.113.24");
    }

    #[test]
    fn vpn_no_sessions_when_no_data() {
        assert!(parse_vpn_sessions("No active tunnels\n").is_empty());
        assert!(parse_vpn_sessions("").is_empty());
    }

    const REAL_SAMPLE: &str = r#"
ip nat policy from trust to untrust 10 id 1
 source group BMC_172.30.0.2
 destination any
 nat-type snat static 121.147.13.229/32
 enable
!
ip nat policy from untrust to trust 10 id 14
 label Rancher HTTPS(외부 사용자)-서비스정책수정해야함
 source any
 destination 121.147.13.233/32
 nat-type dnat static 172.20.26.230/32
 enable
!
ip nat policy from untrust to trust 11 id 13
 label Bastion SSH(외부 사용자)
 source any
 destination 121.147.13.233/32
 service-group ssh
 nat-type dnat static 172.20.26.102/32
 enable
!
interface eth0
 ip proxy-arp alias 121.147.13.233
"#;

    #[test]
    fn parses_real_format() {
        let rules = parse_nat_rules(REAL_SAMPLE);
        assert_eq!(rules.len(), 3);
        let r = &rules[1];
        assert_eq!(r.id, "nat-10");
        assert_eq!(r.from_zone, "untrust");
        assert_eq!(r.to_zone, "trust");
        assert_eq!(r.rule_type, NatType::Dnat);
        assert_eq!(r.translated_to, "172.20.26.230/32");
        assert_eq!(r.destination, vec!["121.147.13.233/32".to_string()]);
        assert_eq!(r.label.as_deref(), Some("Rancher HTTPS(외부 사용자)-서비스정책수정해야함"));
        assert!(r.enabled);
    }

    #[test]
    fn enabled_by_default_when_omitted() {
        // AXGATE running-config doesn't always emit `enable` for active rules.
        let sample = r#"
ip nat policy from untrust to trust 18 id 22
 label nabiman-rancher-235
 source any
 destination 121.147.13.235/32
 nat-type dnat static 172.20.26.230/32
!
"#;
        let rules = parse_nat_rules(sample);
        assert_eq!(rules.len(), 1);
        assert!(rules[0].enabled, "rule without explicit `enable` should still be active");
    }

    #[test]
    fn explicit_no_enable_is_disabled() {
        let sample = r#"
ip nat policy from untrust to trust 30 id 99
 label shutdown-rule
 source any
 destination 1.2.3.4/32
 nat-type dnat static 10.0.0.1/32
 no enable
!
"#;
        let rules = parse_nat_rules(sample);
        assert_eq!(rules.len(), 1);
        assert!(!rules[0].enabled);
    }

    #[test]
    fn snat_static_parses() {
        let rules = parse_nat_rules(REAL_SAMPLE);
        let r = &rules[0];
        assert_eq!(r.rule_type, NatType::Snat);
        assert_eq!(r.translated_to, "121.147.13.229/32");
        match &r.source {
            NatSource::Groups { groups } => assert_eq!(groups[0], "group BMC_172.30.0.2"),
            _ => panic!("expected group source"),
        }
    }

    #[test]
    fn service_group_captured() {
        let rules = parse_nat_rules(REAL_SAMPLE);
        let r = rules.iter().find(|r| r.label.as_deref() == Some("Bastion SSH(외부 사용자)")).unwrap();
        assert_eq!(r.service_groups, vec!["ssh".to_string()]);
        assert_eq!(r.translated_to, "172.20.26.102/32");
    }

    #[test]
    fn proxy_arp_ip_captured() {
        let ips = parse_public_ips(REAL_SAMPLE);
        assert_eq!(ips.len(), 1);
        assert_eq!(ips[0].ip, "121.147.13.233");
        assert!(ips[0].proxy_arp_enabled);
    }
}
