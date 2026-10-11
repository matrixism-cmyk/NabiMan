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
    assert_eq!(
        r.label.as_deref(),
        Some("Rancher HTTPS(외부 사용자)-서비스정책수정해야함")
    );
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
    assert!(
        rules[0].enabled,
        "rule without explicit `enable` should still be active"
    );
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
    let r = rules
        .iter()
        .find(|r| r.label.as_deref() == Some("Bastion SSH(외부 사용자)"))
        .unwrap();
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
