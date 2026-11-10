//! Signing in against LDAP with `ldapsearch`: find the user's DN with the
//! service account, then bind as that DN with the user's password.
//!
//! Neither password is ever on the command line, where `ps` shows it to
//! everyone: each is written to a 0600 file handed over with `-y` and
//! removed when the run is over. The username reaches the search filter
//! escaped as RFC 4515 asks, so it is always a value and never filter
//! syntax. (It never reaches a DN: the DN bound as is the one the directory
//! returned.) Each run is bounded.

use std::process::Command;

use super::config::LdapConfig;
use crate::bounded_command::{deadline, OutputWithin};
use crate::ssh_auth::{write_secret_file, PwFile};

/// RFC 4515 §3: `*`, `(`, `)`, `\` and NUL in an assertion value are written
/// as `\` and two hex digits.
pub(super) fn escape_filter_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '*' => out.push_str("\\2a"),
            '(' => out.push_str("\\28"),
            ')' => out.push_str("\\29"),
            '\\' => out.push_str("\\5c"),
            '\0' => out.push_str("\\00"),
            c => out.push(c),
        }
    }
    out
}

/// The configured filter with `{username}` replaced by the escaped name.
pub(super) fn user_filter(template: &str, username: &str) -> String {
    template.replace("{username}", &escape_filter_value(username))
}

/// A DN as compared for group membership: case and the spaces around `,`
/// and `=` do not matter. Empty matches nothing.
fn normal_dn(dn: &str) -> String {
    dn.split(',')
        .map(|rdn| rdn.split('=').map(|p| p.trim().to_lowercase()).collect::<Vec<_>>().join("="))
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn member_of(groups: &[String], group: &str) -> bool {
    let want = normal_dn(group);
    !want.is_empty() && groups.iter().any(|g| normal_dn(g) == want)
}

fn password_file(password: &str) -> Result<PwFile, String> {
    write_secret_file(password).map_err(|_| "Could not prepare the LDAP password".to_string())
}

/// The service account's search for `filter`. `bind_pw` is the 0600 file
/// holding the bind password, when a bind DN is configured.
pub(super) fn search_args(cfg: &LdapConfig, uri: &str, filter: &str, bind_pw: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = ["-x", "-o", "ldif-wrap=no", "-H", uri, "-b", &cfg.search_base, "-LLL"]
        .iter()
        .map(|a| a.to_string())
        .collect();
    if let (false, Some(file)) = (cfg.bind_dn.is_empty(), bind_pw) {
        args.extend(["-D".into(), cfg.bind_dn.clone(), "-y".into(), file.to_string()]);
    }
    args.extend([filter.to_string(), "dn".into(), "memberOf".into()]);
    args
}

/// Binding as `user_dn`, reading only its own entry.
pub(super) fn bind_args(uri: &str, user_dn: &str, user_pw: &str) -> Vec<String> {
    ["-x", "-o", "ldif-wrap=no", "-H", uri, "-D", user_dn, "-y", user_pw, "-b", user_dn, "-s", "base", "-LLL", "(objectClass=*)", "dn"]
        .iter()
        .map(|a| a.to_string())
        .collect()
}

/// The user's DN and LDIF (with memberOf), via the service account.
pub(super) fn find_user(cfg: &LdapConfig, uri: &str, username: &str) -> Result<String, String> {
    let bind_pw = if cfg.bind_dn.is_empty() { None } else { Some(password_file(&cfg.bind_password)?) };
    let args = search_args(cfg, uri, &user_filter(&cfg.user_filter, username), bind_pw.as_ref().map(|f| f.path()));
    let out = Command::new("ldapsearch")
        .args(&args)
        .output_within(deadline::PROBE)
        .map_err(|e| format!("ldapsearch not available: {}", e))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Whether `password` binds as `user_dn`.
pub(super) fn bind_as(uri: &str, user_dn: &str, password: &str) -> Result<(), String> {
    let user_pw = password_file(password)?;
    let out = Command::new("ldapsearch")
        .args(bind_args(uri, user_dn, user_pw.path()))
        .output_within(deadline::PROBE)
        .map_err(|e| format!("ldapsearch bind failed: {}", e))?;
    drop(user_pw);
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("Invalid credentials") || stderr.contains("49") {
        return Err("Invalid LDAP credentials".into());
    }
    Err(format!("LDAP bind failed: {}", stderr.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_values_are_escaped_per_rfc_4515() {
        assert_eq!(escape_filter_value("john"), "john");
        assert_eq!(escape_filter_value("a*b(c)d\\e\0"), "a\\2ab\\28c\\29d\\5ce\\00");
        assert_eq!(escape_filter_value("김철수"), "김철수");
        // A name that tries to close the filter stays one value.
        assert_eq!(user_filter("(&(uid={username})(objectClass=person))", "x)(uid=*"), "(&(uid=x\\29\\28uid=\\2a)(objectClass=person))");
        assert_eq!(user_filter("(|(uid={username})(mail={username}))", "*"), "(|(uid=\\2a)(mail=\\2a))");
    }

    #[test]
    fn passwords_go_by_file_never_by_argv() {
        let cfg = LdapConfig { bind_dn: "cn=svc,dc=example,dc=com".into(), bind_password: "bind-secret".into(), ..LdapConfig::default() };
        let search = search_args(&cfg, "ldap://h:389", "(uid=x)", Some("/run/pw_1"));
        let bind = bind_args("ldap://h:389", "uid=x,dc=example,dc=com", "/run/pw_2");
        for args in [&search, &bind] {
            assert!(!args.iter().any(|a| a == "-w" || a.contains("secret")), "{args:?}");
            assert!(args.windows(2).any(|w| w[0] == "-y"));
        }
        let anonymous = LdapConfig { bind_dn: String::new(), ..cfg };
        assert!(!search_args(&anonymous, "ldap://h", "(uid=x)", None).iter().any(|a| a == "-D" || a == "-y"));
    }

    #[test]
    fn the_password_file_holds_exactly_the_password() {
        let dir = std::env::temp_dir().join(format!("nabiman-ldap-pw-{}", uuid::Uuid::new_v4()));
        let file = crate::ssh_auth::write_secret_in(&dir.to_string_lossy(), "p@ss word\n2").unwrap();
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(file.path()).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(file.path()).unwrap(), "p@ss word\n2");
        let path = file.path().to_string();
        drop(file);
        assert!(!std::path::Path::new(&path).exists(), "removed after use");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn group_membership_is_the_same_dn_not_a_substring() {
        let groups = vec!["CN=Admins, OU=Groups,DC=example,DC=com".to_string(), "cn=a".to_string()];
        assert!(member_of(&groups, "cn=admins,ou=groups,dc=example,dc=com"));
        assert!(!member_of(&["cn=a".into()], "cn=admins,ou=groups,dc=example,dc=com"));
        assert!(!member_of(&["cn=x,cn=admins,ou=groups,dc=example,dc=com".into()], "cn=admins,ou=groups,dc=example,dc=com"));
        assert!(!member_of(&groups, ""), "an empty group setting matches nobody");
    }
}
