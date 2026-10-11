use super::*;

#[test]
fn test_validate_ip() {
    assert!(validate_ip("192.168.1.1"));
    assert!(validate_ip("10.0.0.1"));
    assert!(validate_ip("::1"));
    assert!(validate_ip("fe80::1"));
    assert!(validate_ip("2001:db8::1"));
    assert!(!validate_ip(""));
    assert!(!validate_ip("1.2.3.4; rm -rf /"));
    assert!(!validate_ip("$(whoami)"));
    assert!(!validate_ip("1.2.3.4 && echo pwned"));
    assert!(!validate_ip("not-an-ip"));
    assert!(!validate_ip("192.168.1.999"));
}

#[test]
fn test_validate_jail() {
    assert!(validate_jail("sshd"));
    assert!(validate_jail("apache-auth"));
    assert!(!validate_jail(""));
    assert!(!validate_jail("jail; rm -rf /"));
}

#[test]
fn test_parse_jail_list() {
    let o = "Status\n|- Number of jail:\t2\n`- Jail list:\tsshd, apache-auth";
    assert_eq!(parse_jail_list(o), vec!["sshd", "apache-auth"]);
    let o2 = "Status\n|- Number of jail:\t0\n`- Jail list:\t";
    assert!(parse_jail_list(o2).is_empty());
}

#[test]
fn test_parse_banned_f2b() {
    let o = "Status for the jail: sshd\n|- Filter\n`- Actions\n   \
            |- Currently banned:\t2\n   `- Banned IP list:\t1.2.3.4 5.6.7.8";
    let b = parse_banned_f2b("sshd", o);
    assert_eq!(b.len(), 2);
    assert_eq!(b[0].ip, "1.2.3.4");
    assert_eq!(b[1].jail, "sshd");
}

#[test]
fn test_parse_iptables_banned() {
    let o = "Chain INPUT (policy ACCEPT)\ntarget     prot opt source               destination\n\
            DROP       all  --  10.0.0.5             0.0.0.0/0\nACCEPT     all  --  0.0.0.0/0            0.0.0.0/0";
    let b = parse_iptables_banned(o);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].ip, "10.0.0.5");
}

#[test]
fn test_extract_jail_from_log() {
    let l = "2024-01-15 10:23:45,123 fail2ban.actions [123]: NOTICE  [sshd] Ban 1.2.3.4";
    assert_eq!(extract_jail_from_log(l), Some("sshd".to_string()));
}
