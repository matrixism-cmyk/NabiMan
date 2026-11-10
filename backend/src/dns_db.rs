//! How the DNS panel signs in to the `nabidns` MySQL database for its
//! statistics. The password is never on the command line (where `ps` shows
//! it) and never in the source: the client reads it from an option file, or
//! is handed it in its environment.
//!
//! - `NABIMAN_DNS_DB_DEFAULTS_FILE` (default `<data_dir>/nabidns-mysql.cnf`):
//!   a MySQL option file with a `[client]` section holding `password=…`
//!   (and optionally `user=`/`host=`). It must be readable by its owner only
//!   (0600); a file others can read is ignored.
//! - otherwise `NABIMAN_DNS_DB_PASSWORD`, passed to the client as `MYSQL_PWD`.
//!
//! With neither, the client runs without a password and the statistics stay
//! empty, which is how the panel looks on a machine with no NabiDNS.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bounded_command::{deadline, OutputWithin};

#[derive(Debug, PartialEq)]
enum Auth {
    OptionFile(PathBuf),
    Env(String),
    None,
}

fn option_file() -> PathBuf {
    std::env::var_os("NABIMAN_DNS_DB_DEFAULTS_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| nabiman_server::data_dir().join("nabidns-mysql.cnf"))
}

fn private_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o077 == 0)
}

fn auth(file: &Path, password: Option<String>) -> Auth {
    if private_file(file) {
        return Auth::OptionFile(file.to_path_buf());
    }
    if file.exists() {
        eprintln!("DNS statistics: {} is readable by others and is ignored; chmod 600 it", file.display());
    }
    match password {
        Some(pw) if !pw.is_empty() => Auth::Env(pw),
        _ => Auth::None,
    }
}

fn command(auth: Auth, sql: &str) -> Command {
    let mut cmd = Command::new("mysql");
    // --defaults-extra-file is only honoured as the very first option.
    if let Auth::OptionFile(path) = &auth {
        cmd.arg(format!("--defaults-extra-file={}", path.display()));
    }
    if let Auth::Env(pw) = &auth {
        cmd.env("MYSQL_PWD", pw);
    }
    cmd.args(["-u", "nabidns", "-h", "127.0.0.1", "-BNe", sql, "--", "nabidns"]);
    cmd
}

/// Runs one statistics query; empty on any failure.
pub(crate) fn query(sql: &str) -> String {
    let auth = auth(&option_file(), std::env::var("NABIMAN_DNS_DB_PASSWORD").ok());
    command(auth, sql)
        .output_within(deadline::QUICK)
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(cmd: &Command) -> Vec<String> {
        cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn the_password_never_reaches_argv() {
        let cmd = command(Auth::Env("s3cret-value".into()), "SELECT 1");
        assert!(args(&cmd).iter().all(|a| !a.contains("s3cret") && !a.starts_with("-p")));
        assert!(cmd.get_envs().any(|(k, v)| k == "MYSQL_PWD" && v.is_some_and(|v| v == "s3cret-value")));
        let cmd = command(Auth::OptionFile("/x/nabidns.cnf".into()), "SELECT 1");
        assert_eq!(args(&cmd)[0], "--defaults-extra-file=/x/nabidns.cnf");
        assert!(cmd.get_envs().next().is_none());
    }

    #[test]
    fn only_a_private_option_file_is_used() {
        let dir = std::env::temp_dir().join(format!("nabiman-dnsdb-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("nabidns-mysql.cnf");
        assert_eq!(auth(&file, None), Auth::None);
        assert_eq!(auth(&file, Some("pw".into())), Auth::Env("pw".into()));
        std::fs::write(&file, "[client]\npassword=x\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(auth(&file, None), Auth::None, "world-readable is ignored");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(auth(&file, Some("pw".into())), Auth::OptionFile(file.clone()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
