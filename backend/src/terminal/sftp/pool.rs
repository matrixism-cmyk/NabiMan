//! SFTP connections to saved servers, kept open while someone is browsing.
//!
//! A connection is the system `ssh` client asked for the `sftp` subsystem,
//! built exactly like the terminal's: the same pinned host key, the same
//! stored password handed over through a short-lived file, the same key.
//! Nothing about reaching a server is decided twice. The SFTP protocol then
//! runs over that process's stdin and stdout.
//!
//! Each operator gets their own connection per server, reused across requests
//! so that opening a folder does not mean a fresh SSH handshake, and closed
//! after a while of not being used.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use russh_sftp::client::{Config, SftpSession};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

use crate::models::RemoteServer;

/// A connection nobody has used for this long is closed.
const IDLE: Duration = Duration::from_secs(10 * 60);
/// How long the SSH handshake and SFTP greeting may take together.
const CONNECT: Duration = Duration::from_secs(20);

pub(crate) struct Conn {
    pub(crate) sftp: SftpSession,
    /// The ssh process; dropping the connection kills it.
    child: Mutex<Child>,
    last_used: Mutex<Instant>,
}

impl Conn {
    fn alive(&self) -> bool {
        let mut child = self.child.lock().unwrap_or_else(|e| e.into_inner());
        matches!(child.try_wait(), Ok(None))
    }
    fn idle(&self) -> bool {
        self.last_used.lock().map(|t| t.elapsed() > IDLE).unwrap_or(true)
    }
    fn touch(&self) {
        if let Ok(mut t) = self.last_used.lock() {
            *t = Instant::now();
        }
    }
}

fn pool() -> &'static Mutex<HashMap<String, Arc<Conn>>> {
    static POOL: OnceLock<Mutex<HashMap<String, Arc<Conn>>>> = OnceLock::new();
    POOL.get_or_init(Default::default)
}

/// Who is connecting, and to exactly which approved target. Editing the server
/// (address, user, password, host key) yields a different key, so a stale
/// connection is never reused for the new target.
fn key(user_id: &str, srv: &RemoteServer) -> String {
    let parts = [
        user_id,
        &srv.id,
        &srv.host,
        &srv.port.to_string(),
        &srv.user,
        &srv.auth_method,
        &srv.host_trust_version,
        &srv.password_enc,
    ];
    hex::encode(Sha256::digest(parts.join("\0")))
}

fn sweep(map: &mut HashMap<String, Arc<Conn>>) {
    map.retain(|_, c| c.alive() && !c.idle());
}

/// The operator's open connection to `srv`, or a new one.
pub(crate) async fn session(user_id: &str, srv: &RemoteServer) -> Result<Arc<Conn>, String> {
    let key = key(user_id, srv);
    {
        let mut map = pool().lock().unwrap_or_else(|e| e.into_inner());
        sweep(&mut map);
        if let Some(conn) = map.get(&key) {
            conn.touch();
            return Ok(conn.clone());
        }
    }
    let conn = Arc::new(open(srv).await?);
    pool()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, conn.clone());
    Ok(conn)
}

/// Drops the operator's connection to `srv`, after it broke mid-request.
pub(crate) fn forget(user_id: &str, srv: &RemoteServer) {
    pool()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&key(user_id, srv));
}

async fn open(srv: &RemoteServer) -> Result<Conn, String> {
    let (command, password_file) =
        crate::remote_probe::ssh_invocation(srv, 10, &["-s"], "sftp")?;
    let mut command = Command::from(command);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|_| "Cannot start the ssh client".to_string())?;
    let (Some(stdin), Some(stdout), Some(mut stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err("Cannot talk to the ssh client".into());
    };
    let config = Config {
        request_timeout_secs: 60,
        ..Config::default()
    };
    let started = tokio::time::timeout(
        CONNECT,
        SftpSession::new_with_config(tokio::io::join(stdout, stdin), config),
    )
    .await;
    // ssh has read the password by now, or never will.
    drop(password_file);
    match started {
        Ok(Ok(sftp)) => {
            // Keep reading ssh's warnings so a full pipe never stalls it.
            tokio::spawn(async move {
                let _ = tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await;
            });
            Ok(Conn {
                sftp,
                child: Mutex::new(child),
                last_used: Mutex::new(Instant::now()),
            })
        }
        failed => {
            let _ = child.start_kill();
            let mut said = Vec::new();
            let _ = tokio::time::timeout(
                Duration::from_secs(2),
                (&mut stderr).take(8192).read_to_end(&mut said),
            )
            .await;
            Err(failure_message(
                &String::from_utf8_lossy(&said),
                matches!(failed, Err(_)),
            ))
        }
    }
}

/// What to tell the operator when SFTP did not come up: ssh's own last word
/// when it said something, since that names the cause (a refused password, an
/// unreachable host, a changed host key).
pub(crate) fn failure_message(stderr: &str, timed_out: bool) -> String {
    let said = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("Warning: Permanently added"))
        .last()
        .map(|l| l.chars().take(300).collect::<String>());
    match (said, timed_out) {
        (Some(line), _) => format!("SFTP connection failed: {line}"),
        (None, true) => "SFTP connection timed out".into(),
        (None, false) => "SFTP is not available on this server".into(),
    }
}
