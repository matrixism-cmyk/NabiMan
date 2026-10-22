//! Reaching a saved server over SSH: how the command is built (key or stored
//! password) and what a health probe reads back.

use std::process::Command;

use crate::models::{RemoteServer, RemoteServerStatus};
use crate::ssh_auth;

use crate::remote_servers::{now_iso, server_password};

pub(crate) fn ssh_command(
    srv: &RemoteServer,
    connect_timeout: u32,
    remote_cmd: &str,
) -> Result<(Command, Option<ssh_auth::PwFile>), String> {
    ssh_invocation(srv, connect_timeout, &[], remote_cmd)
}

/// [`ssh_command`] with extra options placed before the destination, such as
/// `-s` to ask for a subsystem (SFTP) instead of a command.
pub(crate) fn ssh_invocation(
    srv: &RemoteServer,
    connect_timeout: u32,
    extra: &[&str],
    remote_cmd: &str,
) -> Result<(Command, Option<ssh_auth::PwFile>), String> {
    let trust = crate::remote_host_trust::options(srv)?;
    let port = srv.port.to_string();
    let target = format!("{}@{}", srv.user, crate::member_servers::dial_host(srv, srv.port)?);
    let timeout = format!("ConnectTimeout={}", connect_timeout);
    let isolation = crate::member_servers::ssh_isolation(srv);

    let password = server_password(srv);
    if let Some(pw) = password {
        let pw_file = ssh_auth::write_password_file(&pw)?;
        let mut cmd = Command::new("sshpass");
        cmd.args(["-f", pw_file.path(), "ssh"]);
        cmd.args(isolation);
        cmd.args(["-o", &timeout]);
        cmd.args(&trust);
        cmd.args(ssh_auth::password_auth_opts());
        cmd.args(extra);
        cmd.args(["-p", &port, &target, remote_cmd]);
        return Ok((cmd, Some(pw_file)));
    }
    if srv.member_owned() {
        // Without a password there is nothing a member's server can be
        // logged into with: this machine's own keys are not theirs to use.
        return Err("서버 편집에서 비밀번호를 저장해야 확인하거나 파일을 열 수 있습니다".into());
    }

    let mut cmd = Command::new("ssh");
    cmd.args(&trust);
    cmd.args([
        "-o",
        &timeout,
        "-o",
        "BatchMode=yes",
        "-o",
        "ServerAliveInterval=15",
    ]);
    cmd.args(extra);
    cmd.args(["-p", &port, &target, remote_cmd]);
    Ok((cmd, None))
}

/// A status with nothing measured: what an offline server, or one reached
/// without a shell, reports.
pub(crate) fn bare_status(server: &RemoteServer, status: &str, hostname: String) -> RemoteServerStatus {
    RemoteServerStatus {
        id: server.id.clone(),
        name: server.name.clone(),
        host: server.host.clone(),
        status: status.into(),
        hostname,
        os: String::new(),
        uptime: String::new(),
        cpu_usage: String::new(),
        memory: String::new(),
        disk: String::new(),
        load: String::new(),
        checked_at: now_iso(),
        checks: Vec::new(),
    }
}

/// Why ssh failed, from what it printed and how it exited.
pub(crate) fn ssh_failure(stderr: &str, code: Option<i32>) -> &'static str {
    // sshpass swallows ssh's own message, so fall back to its exit code:
    // 5 = password rejected, 6/7 = host key unknown/changed.
    let code_reason = match code {
        Some(5) => Some("Auth failed (password rejected)"),
        Some(6) => Some("Host key unknown"),
        Some(7) => Some("Host key changed"),
        _ => None,
    };
    if stderr.contains("Connection refused") {
        "Connection refused"
    } else if stderr.contains("timed out") {
        "Connection timed out"
    } else if stderr.contains("Permission denied") {
        "Auth failed (Permission denied)"
    } else if stderr.contains("No route to host") {
        "No route to host"
    } else if stderr.contains("Host key verification") {
        "Host key verification failed"
    } else if let Some(r) = code_reason {
        r
    } else {
        "Connection failed"
    }
}

/// Logs in over SSH and reads the machine's vital signs.
pub(crate) fn ssh_probe(server: &RemoteServer) -> RemoteServerStatus {
    let remote_cmd = concat!(
        "hostname 2>/dev/null || echo unknown;",
        "cat /etc/os-release 2>/dev/null | grep PRETTY_NAME | head -1 | cut -d= -f2 | tr -d '\"' || uname -s;",
        "uptime -p 2>/dev/null || uptime | sed 's/.*up/up/';",
        "top -bn1 2>/dev/null | grep 'Cpu(s)' | awk '{print $2}' || echo 0;",
        "free -m 2>/dev/null | awk '/^Mem:/{printf \"%d/%dMB (%.1f%%)\", $3, $2, $3/$2*100}' || echo unknown;",
        "df -h / 2>/dev/null | awk 'NR==2{printf \"%s/%s (%s)\", $3, $2, $5}' || echo unknown;",
        "cat /proc/loadavg 2>/dev/null | awk '{print $1, $2, $3}' || echo unknown"
    );
    let (mut cmd, _pw_guard) = match ssh_command(server, 5, remote_cmd) {
        Ok(c) => c,
        Err(e) => return bare_status(server, "offline", format!("SSH setup error: {}", e)),
    };
    match cmd.output() {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            let lines: Vec<&str> = stdout.lines().collect();
            let line = |i: usize, default: &str| lines.get(i).unwrap_or(&default).to_string();
            RemoteServerStatus {
                os: line(1, "unknown"),
                uptime: line(2, "unknown"),
                cpu_usage: format!("{}%", line(3, "0").trim()),
                memory: line(4, "unknown"),
                disk: line(5, "unknown"),
                load: line(6, "unknown"),
                ..bare_status(server, "online", line(0, "unknown"))
            }
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let reason = ssh_failure(&stderr, result.status.code());
            // A server that only offers SFTP says so instead of running anything.
            let reason = if stderr.contains("only") && stderr.to_ascii_lowercase().contains("sftp") {
                "Shell refused (SFTP only)"
            } else {
                reason
            };
            bare_status(server, "offline", reason.into())
        }
        Err(e) => bare_status(server, "offline", format!("SSH error: {}", e)),
    }
}
