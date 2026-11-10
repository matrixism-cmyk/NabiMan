use super::settings::*;
use std::path::PathBuf;

fn none(name: &str) -> Option<String> {
    (name == "PATH").then(|| "/nonexistent-nabiman-path".to_string())
}

#[test]
fn a_fresh_install_has_no_site_values() {
    assert_eq!(dictation_url_in(&none), None, "dictation is off until a service is set");
    assert!(letsencrypt_email_in(&none).unwrap_err().contains("NABIMAN_LETSENCRYPT_EMAIL"));
    assert_eq!(dns_check_in(&none), None, "no DNS check without a name");
    assert!(mec_lb_pools_in(&none).is_empty());
    assert_eq!(claude_in(&none), None);
    assert_eq!(rhwp_in(&none).filter(|p| !p.starts_with("/usr/local") && !p.starts_with("/opt")), None);
    for p in [soffice_in(&none), rhwp_in(&none)].into_iter().flatten() {
        assert!(!p.starts_with("/workspace"), "{p:?}");
    }
}

#[test]
fn set_values_are_used() {
    let get = |name: &str| match name {
        "NABIMAN_DICTATION_URL" => Some("https://speech.example.com".into()),
        "NABIMAN_LETSENCRYPT_EMAIL" => Some("ops@example.com".into()),
        "NABIMAN_DNS_CHECK_DOMAIN" => Some("example.com".into()),
        "NABIMAN_DNS_CHECK_EXPECT" => Some("192.0.2.1".into()),
        "NABIMAN_SOFFICE" => Some("/opt/lo/soffice.bin".into()),
        "NABIMAN_CLAUDE" => Some("/opt/claude/bin/claude".into()),
        "NABIMAN_MEC_LB_POOLS" => Some("tenant=10.0.0.151-10.0.0.200 auto, reserved=10.0.0.231-10.0.0.238 10.0.0.250, bad=1.2.3, =10.0.0.1".into()),
        _ => None,
    };
    assert_eq!(dictation_url_in(&get).as_deref(), Some("https://speech.example.com"));
    assert_eq!(letsencrypt_email_in(&get).unwrap(), "ops@example.com");
    assert_eq!(dns_check_in(&get), Some(("example.com".into(), Some("192.0.2.1".into()))));
    assert_eq!(soffice_in(&get), Some(PathBuf::from("/opt/lo/soffice.bin")));
    assert_eq!(claude_in(&get), Some(PathBuf::from("/opt/claude/bin/claude")));
    let pools = mec_lb_pools_in(&get);
    assert_eq!(pools.len(), 2);
    assert_eq!((pools[0].name.as_str(), pools[0].size(), pools[0].auto_assign), ("tenant", 50, true));
    assert_eq!((pools[1].name.as_str(), pools[1].size(), pools[1].auto_assign), ("reserved", 9, false));
    assert!(pools[0].contains("10.0.0.200") && !pools[0].contains("10.0.0.201") && pools[1].contains("10.0.0.250"));
    assert!(!pools[0].contains("not-an-ip"));
}

#[test]
fn unsafe_values_are_refused() {
    let get = |name: &str| match name {
        "NABIMAN_LETSENCRYPT_EMAIL" => Some("--agree-tos".into()),
        "NABIMAN_DNS_CHECK_DOMAIN" => Some("-x".into()),
        _ => None,
    };
    assert!(letsencrypt_email_in(&get).is_err());
    assert_eq!(dns_check_in(&get), None);
}
