//! The one way NabiMan runs a child process to completion: with a deadline,
//! bounded captured output, and the child's whole process group stopped when
//! time runs out (so `sshpass` takes its `ssh` with it).
//!
//! These calls block. Async handlers reach them through `web::block` (or
//! `spawn_blocking`), never directly, so a hung command costs one blocking
//! thread for at most its deadline and never an HTTP worker.
//!
//! The password/command content is never reflected in errors.
use std::{
    io::{Read, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

/// Per-operation deadlines, longest first. A deadline bounds the whole
/// command, connection included; SSH's own `ConnectTimeout` is shorter.
pub(crate) mod deadline {
    use std::time::Duration;
    /// A full package upgrade (`apt upgrade -y`, `dnf upgrade -y`).
    pub const SYSTEM_UPGRADE: Duration = Duration::from_secs(60 * 60);
    /// Installing, removing or upgrading one package; a package-list refresh.
    pub const PACKAGE: Duration = Duration::from_secs(15 * 60);
    /// Listing or searching packages; `apt update` before a check.
    pub const PACKAGE_QUERY: Duration = Duration::from_secs(3 * 60);
    /// Asking a CA for a certificate (certbot).
    pub const CERTIFICATE: Duration = Duration::from_secs(3 * 60);
    /// A traceroute: up to 30 hops of 3 probes each.
    pub const TRACEROUTE: Duration = Duration::from_secs(90);
    /// A command typed by an operator to run on a saved server.
    pub const REMOTE_EXEC: Duration = Duration::from_secs(60);
    /// A database query.
    pub const DATABASE: Duration = Duration::from_secs(60);
    /// A database dump or restore.
    pub const DATABASE_DUMP: Duration = Duration::from_secs(30 * 60);
    /// Installing NabiMan's key on a server (ssh-copy-id, then a fallback).
    pub const KEY_DEPLOY: Duration = Duration::from_secs(45);
    /// Reloading or testing a service's configuration (nginx -t, reload).
    pub const SERVICE: Duration = Duration::from_secs(60);
    /// A ping, a DNS lookup, a key-login test.
    pub const PROBE: Duration = Duration::from_secs(30);
    /// One question to the tmux server (has-session, capture-pane).
    pub const TMUX: Duration = Duration::from_secs(5);
    /// A local query that answers at once (which, ssh-keygen -l, ps).
    pub const QUICK: Duration = Duration::from_secs(10);
}

/// How much of each stream is kept by default; the rest is read and dropped.
pub(crate) const MAX_OUTPUT: usize = 4 * 1024 * 1024;
/// After a deadline: time between asking the group to stop and killing it.
const TERM_GRACE: Duration = Duration::from_secs(3);
/// After the child exits: how long a background grandchild may hold the pipes.
const EOF_GRACE: Duration = Duration::from_secs(2);

pub(crate) struct Captured {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// Some output went past the limit and was dropped.
    pub truncated: bool,
}

#[derive(Debug, PartialEq)]
pub(crate) enum RunError {
    Start,
    TimedOut(Duration),
    Io(&'static str),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Start => f.write_str("Command could not start"),
            RunError::TimedOut(d) => write!(
                f,
                "Timed out after {}s; the command was stopped and its outcome is unknown",
                d.as_secs()
            ),
            RunError::Io(what) => write!(f, "Command {what}; outcome unknown"),
        }
    }
}

/// Runs `cmd` to completion or to `deadline`, whichever is first, keeping
/// at most `max_output` bytes of each stream. Stdin is `input`, or empty.
pub(crate) fn capture(
    cmd: &mut Command,
    input: Option<&[u8]>,
    deadline: Duration,
    max_output: usize,
) -> Result<Captured, RunError> {
    let mut child = cmd
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|_| RunError::Start)?;
    let result = drive(&mut child, input, deadline, max_output);
    if result.is_err() {
        stop_group(&mut child);
    }
    result
}

/// `Command::output`, bounded: a timeout is an `io::ErrorKind::TimedOut`
/// error saying so, and output past `MAX_OUTPUT` ends in a note that it was cut.
pub(crate) trait OutputWithin {
    fn output_within(&mut self, deadline: Duration) -> std::io::Result<std::process::Output>;
}

impl OutputWithin for Command {
    fn output_within(&mut self, deadline: Duration) -> std::io::Result<std::process::Output> {
        match capture(self, None, deadline, MAX_OUTPUT) {
            Ok(mut c) => {
                if c.truncated {
                    let note = format!("\n[output cut at {} MiB]\n", MAX_OUTPUT >> 20);
                    c.stdout.extend_from_slice(note.as_bytes());
                }
                Ok(std::process::Output { status: c.status, stdout: c.stdout, stderr: c.stderr })
            }
            Err(RunError::TimedOut(d)) => {
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, RunError::TimedOut(d).to_string()))
            }
            Err(e) => Err(std::io::Error::other(e.to_string())),
        }
    }
}

/// `output_within` for an async handler: the command runs on the blocking
/// pool, so the HTTP worker carries on serving while it runs. The command is
/// taken out of `self`, which is left blank.
pub(crate) trait OutputOffThread {
    fn output_off_thread(&mut self, deadline: Duration) -> futures::future::BoxFuture<'static, std::io::Result<std::process::Output>>;
}

impl OutputOffThread for Command {
    fn output_off_thread(&mut self, deadline: Duration) -> futures::future::BoxFuture<'static, std::io::Result<std::process::Output>> {
        let mut cmd = std::mem::replace(self, Command::new("/bin/false"));
        Box::pin(async move {
            actix_web::web::block(move || cmd.output_within(deadline))
                .await
                .unwrap_or_else(|_| Err(std::io::Error::other("Command runner unavailable")))
        })
    }
}

pub(crate) struct Output {
    pub status: ExitStatus,
    pub out: String,
    pub err: String,
}

/// A local tool, run with a fixed PATH and C locale; output past 1 MiB or
/// not UTF-8 is an error rather than something half-read.
pub(crate) fn run(
    program: &Path,
    args: &[&str],
    input: Option<&str>,
    timeout: Duration,
) -> Result<Output, String> {
    let mut cmd = Command::new(program);
    cmd.args(args).env_clear().env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin").env("LC_ALL", "C");
    let c = capture(&mut cmd, input.map(str::as_bytes), timeout, 1024 * 1024).map_err(|e| match e {
        RunError::Start => "Command tool could not start; no change dispatched".to_string(),
        RunError::TimedOut(_) => "Command deadline exceeded; outcome unknown. Reload before retrying".into(),
        RunError::Io(_) => "Command output failed; outcome unknown".into(),
    })?;
    if c.truncated {
        return Err("Command output limit exceeded; outcome unknown".into());
    }
    Ok(Output {
        status: c.status,
        out: String::from_utf8(c.stdout).map_err(|_| "Command output is not UTF-8")?,
        err: String::from_utf8(c.stderr).map_err(|_| "Command diagnostics are not UTF-8")?,
    })
}

fn nonblocking(fd: i32) -> Result<(), RunError> {
    // SAFETY: fcntl on a pipe fd this process owns.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(RunError::Io("pipes could not be bounded"));
        }
    }
    Ok(())
}

fn drive(child: &mut Child, input: Option<&[u8]>, deadline: Duration, max: usize) -> Result<Captured, RunError> {
    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take().ok_or(RunError::Io("output is missing"))?;
    let mut stderr = child.stderr.take().ok_or(RunError::Io("diagnostics are missing"))?;
    nonblocking(stdout.as_raw_fd())?;
    nonblocking(stderr.as_raw_fd())?;
    if let Some(s) = &stdin {
        nonblocking(s.as_raw_fd())?;
    }
    let started = Instant::now();
    let input = input.unwrap_or_default();
    let (mut written, mut truncated) = (0, false);
    let (mut out, mut err) = (vec![], vec![]);
    let mut eof = [false, false];
    let mut status: Option<(ExitStatus, Instant)> = None;
    loop {
        let now = Instant::now();
        if now.duration_since(started) > deadline {
            return Err(RunError::TimedOut(deadline));
        }
        if let Some((st, exited)) = status {
            if eof.iter().all(|x| *x) || now.duration_since(exited) > EOF_GRACE {
                if !eof.iter().all(|x| *x) {
                    // Whatever still holds the pipes belongs to the finished command.
                    kill_group(child.id());
                }
                return Ok(Captured { status: st, stdout: out, stderr: err, truncated });
            }
        }
        let mut fds = vec![];
        for (i, fd) in [stdout.as_raw_fd(), stderr.as_raw_fd()].into_iter().enumerate() {
            if !eof[i] {
                fds.push(libc::pollfd { fd, events: libc::POLLIN, revents: 0 });
            }
        }
        if let Some(s) = &stdin {
            fds.push(libc::pollfd { fd: s.as_raw_fd(), events: libc::POLLOUT, revents: 0 });
        }
        // SAFETY: `fds` is a valid array of pollfd for its length. A short
        // timeout so a child that exits without closing its pipes is noticed.
        unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 50) };
        if let Some(s) = &mut stdin {
            match s.write(&input[written..]) {
                Ok(n) => written += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(_) => written = input.len(), // the child closed its stdin
            }
            if written == input.len() {
                stdin.take();
            }
        }
        for (i, reader, dest) in [(0, &mut stdout as &mut dyn Read, &mut out), (1, &mut stderr as &mut dyn Read, &mut err)] {
            if eof[i] {
                continue;
            }
            let mut buf = [0u8; 16384];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        eof[i] = true;
                        break;
                    }
                    Ok(n) => {
                        let room = max.saturating_sub(dest.len());
                        dest.extend_from_slice(&buf[..n.min(room)]);
                        truncated |= n > room;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => (),
                    Err(_) => return Err(RunError::Io("output failed")),
                }
            }
        }
        if status.is_none() {
            if let Some(st) = child.try_wait().map_err(|_| RunError::Io("wait failed"))? {
                status = Some((st, Instant::now()));
            }
        }
    }
}

fn kill_group(pgid: u32) {
    // SAFETY: signals the process group this module created for the child.
    unsafe { libc::kill(-(pgid as i32), libc::SIGKILL) };
}

/// Asks the child's group to stop, then kills it; reaps the child.
fn stop_group(child: &mut Child) {
    let pgid = child.id() as i32;
    // SAFETY: as above; the child is not yet reaped, so its pid names its group.
    unsafe { libc::kill(-pgid, libc::SIGTERM) };
    let asked = Instant::now();
    while asked.elapsed() < TERM_GRACE {
        if matches!(child.try_wait(), Ok(Some(_))) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    unsafe { libc::kill(-pgid, libc::SIGKILL) };
    let _ = child.wait();
}

#[cfg(test)]
#[path = "bounded_command_tests.rs"]
mod tests;
