//! The optional programs NabiMan uses, checked once at startup: which are
//! there, and which features each one enables. A missing program turns off
//! only its features, and says so here (and in `GET /api/diagnostics/runtime`)
//! instead of failing when someone first uses them.
//!
//! Nothing is checked on the request path: the report is made at startup,
//! and again in the background when an administrator asks (`?refresh=1`).

#[cfg(test)]
mod tests;

use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;

use crate::bounded_command::{deadline, OutputWithin};
use crate::install_config::{settings, values};
use crate::lock_safe::LockExt;

/// One program, as found.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Tool {
    pub name: &'static str,
    pub found: bool,
    /// Where it is (or, for guacd, the address it was reached at).
    pub path: Option<String>,
    pub version: Option<String>,
    /// What it enables, as keys the web app words (`diagnostics.feature.<key>`).
    pub features: &'static [&'static str],
    /// What not having it means, for the log.
    pub missing: &'static str,
}

#[derive(Serialize, Clone, Debug)]
pub(crate) struct Report {
    pub checked_at: String,
    pub tools: Vec<Tool>,
}

/// Where each program comes from: its setting or `PATH`.
pub(crate) struct Sources<'a> {
    pub get: values::Lookup<'a>,
    pub guacd: String,
}

fn tool(name: &'static str, path: Option<PathBuf>, features: &'static [&'static str], missing: &'static str) -> Tool {
    let found = path.as_deref().is_some_and(values::runnable);
    Tool { name, found, path: path.map(|p| p.display().to_string()), version: None, features, missing }
}

fn on_path(get: values::Lookup, names: &[&str]) -> Option<PathBuf> {
    values::find_program(names, &get("PATH").unwrap_or_default(), &[])
}

/// `tmux -V`'s answer ("tmux 3.8"), bounded.
fn version(program: &Path) -> Option<String> {
    let out = Command::new(program).arg("-V").output_within(deadline::QUICK).ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !text.is_empty()).then_some(text)
}

fn reachable(addr: &str) -> bool {
    addr.to_socket_addrs().ok().and_then(|mut a| a.next())
        .is_some_and(|a| TcpStream::connect_timeout(&a, Duration::from_secs(1)).is_ok())
}

/// What is there, looking where `sources` say.
pub(crate) fn detect(sources: &Sources) -> Vec<Tool> {
    let get = sources.get;
    let mut tmux = tool("tmux", on_path(get, &["tmux"]), &["terminal"], "terminals cannot be opened");
    if tmux.found {
        tmux.version = tmux.path.as_deref().and_then(|p| version(Path::new(p)));
    }
    let claude = settings::claude_in(get).or_else(|| {
        let places: Vec<PathBuf> = ["/usr/local/bin/claude", "/usr/bin/claude", "/opt/claude/bin/claude"].iter().map(PathBuf::from).collect();
        values::find_program(&["claude"], &get("PATH").unwrap_or_default(), &places)
    });
    let guacd_up = reachable(&sources.guacd);
    vec![
        tmux,
        tool("ssh", on_path(get, &["ssh"]), &["remote_ssh", "sftp"], "remote servers cannot be reached over SSH"),
        tool("sshpass", on_path(get, &["sshpass"]), &["ssh_password"], "servers that sign in with a stored password cannot be used"),
        tool("ssh-keygen", on_path(get, &["ssh-keygen"]), &["ssh_keys", "ssh_certificates"], "SSH keys and certificates cannot be made"),
        tool("socat", on_path(get, &["socat"]), &["serial_bridge"], "browser serial devices cannot be bridged"),
        Tool { name: "guacd", found: guacd_up, path: Some(sources.guacd.clone()), version: None, features: &["rdp_console"], missing: "RDP consoles cannot be opened" },
        tool("soffice", settings::soffice_in(get), &["office_preview"], "Office documents are not previewed"),
        tool("rhwp", settings::rhwp_in(get), &["hangul_preview"], "Hangul documents are previewed only through LibreOffice, if at all"),
        tool("ffmpeg", on_path(get, &["ffmpeg"]), &[], "nothing in this version needs it"),
        tool("claude", claude, &["claude_voice"], "voice chat with Claude is off"),
    ]
}

static REPORT: Mutex<Option<Report>> = Mutex::new(None);
static REFRESHING: AtomicBool = AtomicBool::new(false);

fn check_now() -> Report {
    let env = values::env;
    let tools = detect(&Sources { get: &env, guacd: settings::guacd_addr() });
    Report { checked_at: chrono::Utc::now().to_rfc3339(), tools }
}

/// Run once at startup (before the server takes requests): check, keep, and log.
pub(crate) fn check_at_startup() {
    let report = check_now();
    log_info!("external_tools", summary(&report.tools));
    *REPORT.lock_safe() = Some(report);
}

/// One line for the log: what was found, and what is off for lack of what.
pub(crate) fn summary(tools: &[Tool]) -> String {
    let found: Vec<String> = tools.iter().filter(|t| t.found)
        .map(|t| t.version.clone().unwrap_or_else(|| t.name.to_string())).collect();
    let missing: Vec<String> = tools.iter().filter(|t| !t.found).map(|t| format!("{} ({})", t.name, t.missing)).collect();
    let mut line = format!("External programs: {}", if found.is_empty() { "none".into() } else { found.join(", ") });
    if !missing.is_empty() {
        line += &format!("; not available: {}", missing.join("; "));
    }
    line
}

/// The last report, as kept; `refresh` also starts a new check in the
/// background (its result shows on a later call).
pub(crate) fn report(refresh: bool) -> Option<Report> {
    if refresh && !REFRESHING.swap(true, Ordering::SeqCst) {
        std::thread::spawn(|| {
            let report = check_now();
            *REPORT.lock_safe() = Some(report);
            REFRESHING.store(false, Ordering::SeqCst);
        });
    }
    REPORT.lock_safe().clone()
}
