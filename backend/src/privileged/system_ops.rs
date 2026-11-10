//! The helper's system operations: a fixed list (firewall rules, package
//! upgrades), each with typed, validated arguments, a fixed absolute program
//! path, a fixed argv and a deadline. Nothing here takes a path, an option,
//! a shell string, an environment variable or a program name from the API.
use super::policy::validate_parent;
use super::protocol::{Operation, State, Status};
use crate::bounded_command::{self, deadline, RunError};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Allow,
    Deny,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Tcp,
    Udp,
    Any,
}

/// What a system operation did. `output` is the command's text, cut to
/// `KEEP` bytes (the start of a listing, the end of an upgrade log).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub state: State,
    pub exit_code: Option<i32>,
    /// `ufw`, `firewalld`, `iptables`, `apt`, `dnf`, `yum`; none when nothing was found.
    pub backend: Option<String>,
    pub output: String,
    pub truncated: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SystemReply {
    pub version: u32,
    pub request_id: String,
    pub status: Status,
    pub outcome: Option<Outcome>,
    /// Why a request was refused, for the operator (never command output).
    pub reason: Option<String>,
}

/// How much output a reply carries (JSON escaping at most doubles it).
pub const KEEP: usize = 24 * 1024;

/// The root-owned approval for system operations, next to the service policy
/// (`/etc/nabiman/privileged-system.json`): which families the API may ask for.
/// Missing: none. Same ownership/mode/parent rules as the service policy.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SystemPolicy {
    pub api_uid: u32,
    #[serde(default)]
    pub firewall: bool,
    #[serde(default)]
    pub package_upgrade: bool,
}
impl SystemPolicy {
    pub fn none(api_uid: u32) -> Self {
        Self { api_uid, firewall: false, package_upgrade: false }
    }
    pub fn load(path: &Path, api_uid: u32) -> io::Result<Self> {
        if !path.exists() {
            return Ok(Self::none(api_uid));
        }
        validate_parent(path)?;
        let file = fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 || metadata.len() > 4096 {
            return Err(unsafe_config());
        }
        let mut bytes = Vec::new();
        file.take(4097).read_to_end(&mut bytes)?;
        let policy: Self = serde_json::from_slice(&bytes).map_err(|_| unsafe_config())?;
        if policy.api_uid != api_uid || api_uid == 0 {
            return Err(unsafe_config());
        }
        Ok(policy)
    }
    /// Recheck the file right before running a command: revoking takes effect at once.
    pub(crate) fn still_approved(&self, path: &Path) -> bool {
        Self::load(path, self.api_uid).is_ok_and(|now| &now == self)
    }
    /// The operation is a system one, its arguments are valid and the policy allows its family.
    pub fn permits(&self, uid: u32, operation: &Operation) -> bool {
        uid == self.api_uid
            && validate(operation)
            && match operation {
                Operation::FirewallStatus {} | Operation::FirewallAdd { .. } | Operation::FirewallDelete { .. } => self.firewall,
                Operation::PackageRefresh {} | Operation::PackageUpgrade { .. } => self.package_upgrade,
                _ => false,
            }
    }
}
fn unsafe_config() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "Unsafe privileged-helper system policy")
}

pub fn is_system(operation: &Operation) -> bool {
    !matches!(operation, Operation::Execute { .. } | Operation::Inspect { .. })
}

/// A package name the package manager can only read as a name: starts with a
/// letter or digit (never an option), then letters, digits and `-_.+:`.
pub fn valid_package(name: &str) -> bool {
    (1..=128).contains(&name.len())
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.+:".contains(&b))
}

/// Arguments within their ranges (the types already rule out everything else).
pub fn validate(operation: &Operation) -> bool {
    match operation {
        Operation::FirewallStatus {} | Operation::PackageRefresh {} => true,
        Operation::FirewallAdd { port, port_end, protocol, .. } => {
            *port >= 1 && port_end.map_or(true, |end| end > *port && *protocol != Protocol::Any)
        }
        Operation::FirewallDelete { number } => (1..=10_000).contains(number),
        Operation::PackageUpgrade { package } => package.as_deref().map_or(true, valid_package),
        _ => false,
    }
}

/// One fixed command: an absolute program and its argv.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub program: PathBuf,
    pub args: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub backend: &'static str,
    pub steps: Vec<Step>,
    pub deadline: Duration,
    /// Keep the end of the output (an upgrade log) rather than its start (a listing).
    pub tail: bool,
}

/// Where the tools are: fixed absolute paths only, never `PATH`.
pub struct Tools {
    pub firewall: Option<(&'static str, PathBuf)>,
    pub packages: Option<(&'static str, PathBuf)>,
}
fn first(places: &[&str]) -> Option<PathBuf> {
    places.iter().map(PathBuf::from).find(|p| {
        fs::metadata(p).is_ok_and(|m| m.is_file() && m.mode() & 0o111 != 0 && m.uid() == 0 && m.mode() & 0o022 == 0)
    })
}
impl Tools {
    /// The firewall in the order the API's root mode picks it (ufw, firewalld,
    /// iptables) and the package manager (apt, dnf, yum); root-owned files only.
    pub fn detect() -> Self {
        let firewall = first(&["/usr/sbin/ufw", "/sbin/ufw"]).map(|p| ("ufw", p))
            .or_else(|| first(&["/usr/bin/firewall-cmd", "/bin/firewall-cmd"]).map(|p| ("firewalld", p)))
            .or_else(|| first(&["/usr/sbin/iptables", "/sbin/iptables"]).map(|p| ("iptables", p)));
        let packages = first(&["/usr/bin/apt-get"]).map(|p| ("apt", p))
            .or_else(|| first(&["/usr/bin/dnf"]).map(|p| ("dnf", p)))
            .or_else(|| first(&["/usr/bin/yum"]).map(|p| ("yum", p)));
        Self { firewall, packages }
    }
}

/// `firewall-cmd` arguments that drop new connections to `ports` (`22`, or
/// firewalld's range form `8000-8100`): one permanent rich rule per protocol
/// (`any` is tcp and udp). firewalld's `--add-port` can only open a port.
/// `ports` must already be digits with at most one `-` (it goes into the rule).
pub fn firewalld_drop_args(ports: &str, protocol: Protocol) -> Vec<String> {
    let protocols: &[&str] = match protocol {
        Protocol::Tcp => &["tcp"],
        Protocol::Udp => &["udp"],
        Protocol::Any => &["tcp", "udp"],
    };
    std::iter::once("--permanent".to_string())
        .chain(protocols.iter().map(|p| format!("--add-rich-rule=rule port port=\"{ports}\" protocol=\"{p}\" drop")))
        .collect()
}

fn step(program: &Path, args: &[&str]) -> Step {
    Step { program: program.to_path_buf(), args: args.iter().map(|a| a.to_string()).collect() }
}

/// The commands for `operation` with these tools; `Err` says why there are none.
pub fn plan(operation: &Operation, tools: &Tools) -> Result<Plan, String> {
    if !validate(operation) {
        return Err("Invalid arguments".into());
    }
    let quick = |backend, steps| Ok(Plan { backend, steps, deadline: Duration::from_secs(30), tail: false });
    match operation {
        Operation::FirewallStatus {} | Operation::FirewallAdd { .. } | Operation::FirewallDelete { .. } => {
            let (backend, fw) = tools.firewall.as_ref().ok_or("No firewall backend found")?;
            let fw = fw.as_path();
            match (operation, *backend) {
                (Operation::FirewallStatus {}, "ufw") => quick(backend, vec![step(fw, &["status"])]),
                (Operation::FirewallStatus {}, "firewalld") => quick(backend, vec![step(fw, &["--state"]), step(fw, &["--list-all"])]),
                (Operation::FirewallStatus {}, _) => quick(backend, vec![step(fw, &["-L", "INPUT", "-n", "--line-numbers"])]),
                (Operation::FirewallAdd { verdict, protocol, port, port_end }, _) => {
                    let ports = port_end.map_or(port.to_string(), |end| format!("{port}:{end}"));
                    let proto = match protocol { Protocol::Tcp => "tcp", Protocol::Udp => "udp", Protocol::Any => "any" };
                    let spec = if *protocol == Protocol::Any { ports.clone() } else { format!("{ports}/{proto}") };
                    match *backend {
                        "ufw" => quick(backend, vec![step(fw, &[if *verdict == Verdict::Deny { "deny" } else { "allow" }, &spec])]),
                        "firewalld" if *verdict == Verdict::Deny => {
                            let ranged = port_end.map_or(port.to_string(), |end| format!("{port}-{end}"));
                            let args = firewalld_drop_args(&ranged, *protocol);
                            let args: Vec<&str> = args.iter().map(String::as_str).collect();
                            quick(backend, vec![step(fw, &args), step(fw, &["--reload"])])
                        }
                        "firewalld" => quick(backend, vec![step(fw, &["--add-port", &spec, "--permanent"]), step(fw, &["--reload"])]),
                        _ => {
                            let proto = if *protocol == Protocol::Any { "tcp" } else { proto };
                            let target = if *verdict == Verdict::Deny { "DROP" } else { "ACCEPT" };
                            quick(backend, vec![step(fw, &["-A", "INPUT", "-p", proto, "--dport", &ports, "-j", target])])
                        }
                    }
                }
                (Operation::FirewallDelete { number }, "ufw") => quick(backend, vec![step(fw, &["--force", "delete", &number.to_string()])]),
                (Operation::FirewallDelete { number }, "iptables") => quick(backend, vec![step(fw, &["-D", "INPUT", &number.to_string()])]),
                _ => Err("Delete by number only supported for ufw/iptables".into()),
            }
        }
        Operation::PackageRefresh {} | Operation::PackageUpgrade { .. } => {
            let (backend, pm) = tools.packages.as_ref().ok_or("No supported package manager found")?;
            let pm = pm.as_path();
            let (steps, deadline) = match (operation, *backend) {
                (Operation::PackageRefresh {}, "apt") => (vec![step(pm, &["update"])], deadline::PACKAGE_QUERY),
                (Operation::PackageRefresh {}, _) => (vec![step(pm, &["makecache", "-q"])], deadline::PACKAGE_QUERY),
                (Operation::PackageUpgrade { package: None }, "apt") => (vec![step(pm, &["upgrade", "-y"])], deadline::SYSTEM_UPGRADE),
                (Operation::PackageUpgrade { package: None }, "dnf") => (vec![step(pm, &["upgrade", "-y"])], deadline::SYSTEM_UPGRADE),
                (Operation::PackageUpgrade { package: None }, _) => (vec![step(pm, &["update", "-y"])], deadline::SYSTEM_UPGRADE),
                (Operation::PackageUpgrade { package: Some(name) }, "apt") => (vec![step(pm, &["install", "--only-upgrade", "-y", "--", name])], deadline::PACKAGE),
                (Operation::PackageUpgrade { package: Some(name) }, "dnf") => (vec![step(pm, &["upgrade", "-y", "--", name])], deadline::PACKAGE),
                (Operation::PackageUpgrade { package: Some(name) }, _) => (vec![step(pm, &["update", "-y", "--", name])], deadline::PACKAGE),
                _ => unreachable!("package operations only"),
            };
            Ok(Plan { backend, steps, deadline, tail: true })
        }
        _ => Err("Not a system operation".into()),
    }
}

/// How long the API waits for the reply: the plan's deadline and some.
pub fn reply_timeout(operation: &Operation) -> Duration {
    let longest = match operation {
        Operation::PackageRefresh {} => deadline::PACKAGE_QUERY,
        Operation::PackageUpgrade { package: None } => deadline::SYSTEM_UPGRADE,
        Operation::PackageUpgrade { .. } => deadline::PACKAGE,
        _ => Duration::from_secs(60),
    };
    longest + Duration::from_secs(30)
}

/// Printable text only (no terminal escapes), cut to `KEEP` bytes.
pub fn bounded_text(bytes: &[u8], tail: bool) -> (String, bool) {
    let text: String = String::from_utf8_lossy(bytes)
        .chars()
        .map(|c| if c == '\n' || c == '\t' || !c.is_control() { c } else { ' ' })
        .collect();
    if text.len() <= KEEP {
        return (text, false);
    }
    let cut = if tail {
        (text.len() - KEEP..text.len()).find(|&i| text.is_char_boundary(i)).unwrap_or(text.len())
    } else {
        (0..=KEEP).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0)
    };
    (if tail { text[cut..].to_string() } else { text[..cut].to_string() }, true)
}

/// Runs the plan's steps in order with the bounded runner (fixed environment,
/// the whole process group stopped at the deadline); stops at the first failure.
pub fn execute(plan: &Plan) -> Outcome {
    let started = std::time::Instant::now();
    let mut all = Vec::new();
    let mut last = (State::CommandSucceeded, Some(0));
    for (i, step) in plan.steps.iter().enumerate() {
        let left = plan.deadline.saturating_sub(started.elapsed());
        let mut command = Command::new(&step.program);
        command.args(&step.args).env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin").env("LC_ALL", "C").env("DEBIAN_FRONTEND", "noninteractive");
        match bounded_command::capture(&mut command, None, left, 1024 * 1024) {
            Ok(done) => {
                all.extend_from_slice(&done.stdout);
                all.extend_from_slice(&done.stderr);
                last = (if done.status.success() { State::CommandSucceeded } else { State::CommandFailed }, done.status.code());
            }
            // Nothing ran: the first step could not start.
            Err(RunError::Start) if i == 0 => { last = (State::NotDispatched, None); }
            Err(_) => { last = (State::UnknownResult, None); }
        }
        if last.0 != State::CommandSucceeded {
            break;
        }
    }
    let (output, truncated) = bounded_text(&all, plan.tail);
    Outcome { state: last.0, exit_code: last.1, backend: Some(plan.backend.to_string()), output, truncated }
}
