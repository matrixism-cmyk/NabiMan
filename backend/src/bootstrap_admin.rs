//! `nabiman-server --bootstrap-admin <name> --password-file <path|->`: make
//! the first administrator of an empty data directory and exit, without a
//! browser or `NABIMAN_PASSWORD` in the server's environment. For whatever
//! provisions an instance (a hosting controller, an installer, Ansible).
//!
//! Refuses when the directory already has users (exit 2), when another
//! server holds the directory (exit 3), and on a name or password the web
//! form would refuse (exit 1). The password is read from the file, or stdin
//! for `-`, never from the command line; it is not printed.

use std::io::Read;
use std::path::Path;

pub(crate) const FLAG: &str = "--bootstrap-admin";
const PASSWORD_FLAG: &str = "--password-file";
const MAX_PASSWORD_FILE: u64 = 4096;

#[derive(Debug, PartialEq)]
pub(crate) enum Failure {
    /// Exit 1: what was asked cannot be done as asked.
    Invalid(String),
    /// Exit 2: the directory already has users; nothing was changed.
    HasUsers,
    /// Exit 3: the directory is in use or cannot be written.
    Storage(String),
}

impl Failure {
    fn code(&self) -> i32 {
        match self {
            Failure::Invalid(_) => 1,
            Failure::HasUsers => 2,
            Failure::Storage(_) => 3,
        }
    }
    fn message(&self) -> String {
        match self {
            Failure::Invalid(why) => why.clone(),
            Failure::HasUsers => "The data directory already has users; nothing was changed".into(),
            Failure::Storage(why) => why.clone(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Request {
    pub username: String,
    pub password_file: String,
}

/// The two options, in either order, and nothing else.
pub(crate) fn parse(args: &[String]) -> Result<Request, Failure> {
    let usage = || Failure::Invalid(format!("Usage: nabiman-server {FLAG} <name> {PASSWORD_FLAG} <file|->"));
    let (mut username, mut password_file) = (None, None);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let slot = match arg.as_str() {
            FLAG => &mut username,
            PASSWORD_FLAG => &mut password_file,
            _ => return Err(usage()),
        };
        if slot.is_some() {
            return Err(usage());
        }
        *slot = Some(rest.next().ok_or_else(usage)?.clone());
    }
    match (username, password_file) {
        (Some(username), Some(password_file)) => Ok(Request { username, password_file }),
        _ => Err(usage()),
    }
}

fn read_password(source: &str) -> Result<String, Failure> {
    let mut text = String::new();
    let read = if source == "-" {
        std::io::stdin().take(MAX_PASSWORD_FILE + 1).read_to_string(&mut text)
    } else {
        std::fs::File::open(source).and_then(|f| f.take(MAX_PASSWORD_FILE + 1).read_to_string(&mut text))
    };
    read.map_err(|e| Failure::Invalid(format!("Cannot read the password file: {e}")))?;
    if text.len() as u64 > MAX_PASSWORD_FILE {
        return Err(Failure::Invalid("The password file is too large".into()));
    }
    // One trailing newline is the file's, not the password's.
    Ok(text.strip_suffix('\n').map(|t| t.strip_suffix('\r').unwrap_or(t)).unwrap_or(&text).to_string())
}

/// The same name rule as `POST /api/users`.
fn name_ok(name: &str) -> bool {
    (2..=32).contains(&name.len()) && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Creates `username` as the only (super) administrator in `dir`.
pub(crate) fn create(dir: &Path, username: &str, password: &str) -> Result<(), Failure> {
    if !name_ok(username) {
        return Err(Failure::Invalid("Name: 2-32 letters, digits, _ or -".into()));
    }
    if !crate::password_policy::acceptable(password) {
        return Err(Failure::Invalid(crate::password_policy::RULE.into()));
    }
    crate::auth::validate_bootstrap_password(password, true).map_err(Failure::Invalid)?;
    // Held while writing: a server cannot start on the directory meanwhile.
    let _lock = crate::data_dir::lock::acquire(dir).map_err(|e| Failure::Storage(e.to_string()))?;
    let users = dir.join("users.json");
    match std::fs::symlink_metadata(&users) {
        Ok(_) => return Err(Failure::HasUsers),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Failure::Storage(format!("Cannot inspect the user database: {e}"))),
    }
    let hash = bcrypt::hash(password, 10).map_err(|e| Failure::Storage(e.to_string()))?;
    crate::users::publish_at(&[crate::users::first_admin(username, &hash)], &users).map_err(Failure::Storage)
}

/// Run before the server when the command line asks for it; never returns then.
pub fn run_if_asked() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a == FLAG) {
        return;
    }
    let dir = nabiman_server::data_dir();
    let result = parse(&args).and_then(|req| {
        let password = read_password(&req.password_file)?;
        create(&dir, &req.username, &password).map(|()| req.username)
    });
    match result {
        Ok(name) => {
            println!("Created administrator {name} in {}", dir.display());
            std::process::exit(0);
        }
        Err(failure) => {
            eprintln!("Bootstrap refused: {}", failure.message());
            std::process::exit(failure.code());
        }
    }
}

#[cfg(test)]
#[path = "bootstrap_admin_tests.rs"]
mod tests;
