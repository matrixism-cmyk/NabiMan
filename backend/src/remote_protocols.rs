//! How a saved server is reached: a terminal over SSH or Telnet (or none),
//! and files over SFTP, FTPS or FTP (or none). SSH-only, SFTP-only and
//! FTP-only servers are all common, so the two are chosen separately.

use crate::models::RemoteServer;

pub(crate) const TERMINALS: [&str; 3] = ["ssh", "telnet", "none"];
pub(crate) const FILES: [&str; 4] = ["sftp", "ftps", "ftp", "none"];

/// Changes to how a server is reached, as an add or edit request carries them.
pub(crate) struct Change<'a> {
    pub terminal: Option<&'a str>,
    pub files: Option<&'a str>,
    pub ftp_port: Option<u16>,
    pub tls_fingerprint: Option<&'a str>,
}

/// Applies the change; true when anything about the connection changed, so
/// the last health check no longer says anything about it.
pub(crate) fn apply(server: &mut RemoteServer, change: &Change) -> bool {
    let before = (server.terminal.clone(), server.files.clone(), server.ftp_port, server.tls_fingerprint.clone());
    if let Some(t) = change.terminal {
        server.terminal = t.to_string();
    }
    if let Some(f) = change.files {
        server.files = f.to_string();
    }
    if let Some(p) = change.ftp_port {
        server.ftp_port = p;
    }
    if let Some(fp) = change.tls_fingerprint {
        server.tls_fingerprint = fp.trim().to_string();
    }
    before != (server.terminal.clone(), server.files.clone(), server.ftp_port, server.tls_fingerprint.clone())
}

/// Whether the combination makes sense and the fields are well formed.
pub(crate) fn check(server: &RemoteServer) -> Result<(), &'static str> {
    if !TERMINALS.contains(&server.terminal.as_str()) {
        return Err("Unknown terminal protocol");
    }
    if !FILES.contains(&server.files.as_str()) {
        return Err("Unknown file protocol");
    }
    if server.terminal == "none" && server.files == "none" {
        return Err("Choose at least one way to connect");
    }
    if server.terminal == "telnet" && server.files == "sftp" {
        return Err("SFTP runs over SSH: choose SSH for the terminal, or no terminal");
    }
    if server.ftp_port == 0 {
        return Err("Invalid FTP port");
    }
    if !server.tls_fingerprint.is_empty()
        && crate::terminal::sftp::ftp::tls::parse_fingerprint(&server.tls_fingerprint).is_none()
    {
        return Err("The certificate fingerprint should be 32 bytes of SHA-256, like SHA256:AB:CD:…");
    }
    if !valid_user(&server.user, server.uses_ssh()) {
        return Err("Invalid username");
    }
    Ok(())
}

/// A login name. FTP hosting often logs in as `name@domain`; over SSH an `@`
/// would be read as the start of the host, so it is refused there.
pub(crate) fn valid_user(user: &str, ssh: bool) -> bool {
    !user.is_empty()
        && user.len() < 64
        && user.chars().all(|c| c.is_alphanumeric() || "_-.".contains(c) || (c == '@' && !ssh))
}

/// The port a new server gets when none is given: SSH's or Telnet's.
pub(crate) fn default_port(terminal: &str) -> u16 {
    if terminal == "telnet" { 23 } else { 22 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(terminal: &str, files: &str) -> RemoteServer {
        serde_json::from_value(serde_json::json!({
            "id": "srv", "name": "n", "host": "h", "port": 22, "user": "u", "auth_method": "key",
            "tags": [], "memo": "", "created_at": "", "terminal": terminal, "files": files,
        }))
        .unwrap()
    }

    #[test]
    fn records_from_before_protocols_are_ssh_and_sftp() {
        let old: RemoteServer = serde_json::from_value(serde_json::json!({
            "id": "srv", "name": "n", "host": "h", "port": 22, "user": "u", "auth_method": "key",
            "tags": [], "memo": "", "created_at": "",
        }))
        .unwrap();
        assert_eq!((old.terminal.as_str(), old.files.as_str(), old.ftp_port), ("ssh", "sftp", 21));
        assert!(old.uses_ssh() && !old.uses_ftp());
    }

    #[test]
    fn combinations_that_cannot_work_are_refused() {
        for (t, f) in [("ssh", "sftp"), ("none", "sftp"), ("ssh", "none"), ("telnet", "ftp"), ("none", "ftps"), ("ssh", "ftp")] {
            assert!(check(&server(t, f)).is_ok(), "{t}+{f}");
        }
        assert!(check(&server("none", "none")).is_err());
        assert!(check(&server("telnet", "sftp")).is_err());
        assert!(check(&server("rdp", "sftp")).is_err());
        let mut bad_pin = server("none", "ftps");
        bad_pin.tls_fingerprint = "SHA256:12".into();
        assert!(check(&bad_pin).is_err());
    }

    #[test]
    fn at_signs_are_for_users_without_ssh() {
        let mut ftp = server("none", "ftp");
        ftp.user = "web@example.com".into();
        assert!(check(&ftp).is_ok());
        ftp.files = "sftp".into();
        assert!(check(&ftp).is_err());
    }

    #[test]
    fn changes_report_whether_the_connection_changed() {
        let mut s = server("ssh", "sftp");
        let same = Change { terminal: Some("ssh"), files: None, ftp_port: Some(21), tls_fingerprint: None };
        assert!(!apply(&mut s, &same));
        let ftp = Change { terminal: Some("none"), files: Some("ftp"), ftp_port: None, tls_fingerprint: None };
        assert!(apply(&mut s, &ftp));
        assert_eq!((s.terminal.as_str(), s.files.as_str()), ("none", "ftp"));
    }
}
