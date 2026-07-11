//! LDAP URL parsing + LDIF response parsing helpers.

pub fn parse_ldap_host_port(server_url: &str) -> Option<(String, u16)> {
    // Formats: ldap://host:port, ldaps://host:port, host:port
    let stripped = server_url
        .trim_start_matches("ldaps://")
        .trim_start_matches("ldap://");
    let is_ssl = server_url.starts_with("ldaps://");
    let default_port: u16 = if is_ssl { 636 } else { 389 };

    let (host, port) = if let Some(idx) = stripped.rfind(':') {
        let h = &stripped[..idx];
        let p = stripped[idx + 1..].trim_end_matches('/').parse::<u16>().unwrap_or(default_port);
        (h.to_string(), p)
    } else {
        (stripped.trim_end_matches('/').to_string(), default_port)
    };

    if host.is_empty() {
        None
    } else {
        Some((host, port))
    }
}

pub fn parse_dn_from_ldif(ldif: &str) -> Option<String> {
    for line in ldif.lines() {
        let trimmed = line.trim();
        if let Some(dn) = trimmed.strip_prefix("dn: ") {
            return Some(dn.to_string());
        }
        if let Some(dn) = trimmed.strip_prefix("dn:: ") {
            // Base64 encoded DN
            if let Ok(decoded) = base64_decode(dn.trim()) {
                return Some(decoded);
            }
        }
    }
    None
}

pub fn parse_member_of(ldif: &str) -> Vec<String> {
    let mut groups = Vec::new();
    for line in ldif.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("memberOf: ") {
            groups.push(val.to_string());
        }
    }
    groups
}

fn base64_decode(input: &str) -> Result<String, String> {
    // Simple base64 decode for DN
    let output = std::process::Command::new("base64")
        .arg("-d")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(input.as_bytes());
            }
            child.wait_with_output()
        })
        .map_err(|e| e.to_string())?;
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ldap_url_standard() {
        let (host, port) = parse_ldap_host_port("ldap://ldap.example.com:389").unwrap();
        assert_eq!(host, "ldap.example.com");
        assert_eq!(port, 389);
    }

    #[test]
    fn test_parse_ldaps_url_default_port() {
        let (host, port) = parse_ldap_host_port("ldaps://ad.corp.local").unwrap();
        assert_eq!(host, "ad.corp.local");
        assert_eq!(port, 636);
    }

    #[test]
    fn test_parse_ldap_url_custom_port() {
        let (host, port) = parse_ldap_host_port("ldap://10.0.0.5:3268").unwrap();
        assert_eq!(host, "10.0.0.5");
        assert_eq!(port, 3268);
    }

    #[test]
    fn test_parse_empty_url() {
        assert!(parse_ldap_host_port("").is_none());
    }

    #[test]
    fn test_parse_dn_from_ldif() {
        let ldif = "dn: uid=john,ou=users,dc=example,dc=com\nuid: john\nmemberOf: cn=admins,ou=groups,dc=example,dc=com\n";
        assert_eq!(parse_dn_from_ldif(ldif), Some("uid=john,ou=users,dc=example,dc=com".into()));
    }

    #[test]
    fn test_parse_dn_empty() {
        assert_eq!(parse_dn_from_ldif(""), None);
        assert_eq!(parse_dn_from_ldif("uid: john\ncn: John"), None);
    }

    #[test]
    fn test_parse_member_of() {
        let ldif = "dn: uid=john,ou=users,dc=ex,dc=com\nmemberOf: cn=admins,ou=groups,dc=ex,dc=com\nmemberOf: cn=devs,ou=groups,dc=ex,dc=com\n";
        let groups = parse_member_of(ldif);
        assert_eq!(groups.len(), 2);
        assert!(groups[0].contains("admins"));
        assert!(groups[1].contains("devs"));
    }
}
