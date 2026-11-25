//! NabiMan's own look at a device, from this server: one ICMP echo with the
//! system's `ping`, or one TCP connect. Nothing else is ever sent: no port
//! scan, no SNMP, no sign-in.

use std::net::Ipv4Addr;
use std::process::Command;
use std::time::{Duration, Instant};

use futures::future::BoxFuture;

use super::reach::{Method, Outcome, Prober, Target};
use crate::bounded_command::{capture, RunError};

/// `ping -W 1` gives up after a second; this stops one that does not.
const PING_DEADLINE: Duration = Duration::from_secs(3);
const TCP_TIMEOUT: Duration = Duration::from_secs(1);
/// ping's output is a few lines; the rest is not kept.
const PING_OUTPUT: usize = 16 * 1024;

pub(crate) struct SystemProber;

impl Prober for SystemProber {
    fn probe(&self, (address, method): Target) -> BoxFuture<'static, Outcome> {
        match method {
            Method::Ping => Box::pin(async move { actix_web::web::block(move || ping(address)).await.unwrap_or(Outcome::Unavailable("ping_failed")) }),
            Method::Tcp(port) => Box::pin(tcp(address, port)),
        }
    }
}

/// One echo, one second to answer. The address is the only thing that
/// varies in the arguments and it is an `Ipv4Addr` written back out, so it
/// cannot be an option or a name. No shell.
pub(crate) fn ping(address: Ipv4Addr) -> Outcome {
    let address = address.to_string();
    let mut cmd = Command::new("ping");
    cmd.args(["-n", "-c", "1", "-W", "1", address.as_str()]).env_clear().env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin").env("LC_ALL", "C");
    match capture(&mut cmd, None, PING_DEADLINE, PING_OUTPUT) {
        Ok(done) => judge_ping(done.status.code(), &String::from_utf8_lossy(&done.stdout), &String::from_utf8_lossy(&done.stderr)),
        Err(RunError::Start) => Outcome::Unavailable("ping_missing"),
        Err(RunError::TimedOut(_)) => Outcome::NoReply,
        Err(RunError::Io(_)) => Outcome::Unavailable("ping_failed"),
    }
}

/// What ping's exit says: 0 an answer, 1 none, anything else it could not
/// ask (not allowed to open the socket, no route from here, …).
pub(crate) fn judge_ping(code: Option<i32>, stdout: &str, stderr: &str) -> Outcome {
    match code {
        Some(0) => Outcome::Reply { ms: ping_time(stdout) },
        Some(1) => Outcome::NoReply,
        _ => {
            let said = stderr.to_ascii_lowercase();
            if said.contains("not permitted") || said.contains("permission denied") {
                Outcome::Unavailable("ping_denied")
            } else if said.contains("unreachable") {
                Outcome::Unavailable("no_route")
            } else {
                Outcome::Unavailable("ping_failed")
            }
        }
    }
}

/// `time=0.612 ms` in a reply line.
fn ping_time(stdout: &str) -> Option<f64> {
    let rest = stdout.split_once("time=")?.1;
    let number: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let unit = rest[number.len()..].trim_start();
    number.parse::<f64>().ok().filter(|ms| ms.is_finite() && unit.starts_with("ms"))
}

/// One connect, closed as soon as it opens; nothing is written or read.
async fn tcp(address: Ipv4Addr, port: u16) -> Outcome {
    let started = Instant::now();
    match tokio::time::timeout(TCP_TIMEOUT, tokio::net::TcpStream::connect((address, port))).await {
        Ok(Ok(stream)) => {
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            drop(stream);
            Outcome::Reply { ms: Some(ms) }
        }
        Ok(Err(e)) if e.raw_os_error() == Some(libc::ENETUNREACH) => Outcome::Unavailable("no_route"),
        // Refused, reset, timed out: the port did not open.
        _ => Outcome::NoReply,
    }
}
