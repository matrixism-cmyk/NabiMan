//! Reaching a saved server over SSH: how the command is built (key or stored
//! password) and what a health probe reads back.

use std::process::Command;

use crate::models::{RemoteServer, RemoteServerStatus};
use crate::ssh_auth;

use crate::remote_servers::{now_iso, server_password};

pub(crate) fn ssh_command(srv: &RemoteServer, connect_timeout: u32, remote_cmd: &str)
    -> Result<(Command, Option<ssh_auth::PwFile>), String>
{
    let port = srv.port.to_string();
    let target = format!("{}@{}", srv.user, srv.host);
    let timeout = format!("ConnectTimeout={}", connect_timeout);

    let password = server_password(srv);
    if let Some(pw) = password {
        let pw_file = ssh_auth::write_password_file(&pw)?;
        let mut cmd = Command::new("sshpass");
        cmd.args(["-f", pw_file.path(), "ssh",
                  "-o", "StrictHostKeyChecking=accept-new",
                  "-o", &timeout]);
        cmd.args(ssh_auth::password_auth_opts());
        cmd.args(["-p", &port, &target, remote_cmd]);
        return Ok((cmd, Some(pw_file)));
    }

    let mut cmd = Command::new("ssh");
    cmd.args(["-o", "StrictHostKeyChecking=accept-new",
              "-o", &timeout,
              "-o", "BatchMode=yes",
              "-o", "ServerAliveInterval=15",
              "-p", &port, &target, remote_cmd]);
    Ok((cmd, None))
}

pub(crate) fn ssh_probe(server: &RemoteServer) -> RemoteServerStatus {
    let checked_at = now_iso();
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
        Err(e) => return RemoteServerStatus {
            id: server.id.clone(), name: server.name.clone(), host: server.host.clone(),
            status: "offline".into(), hostname: format!("SSH setup error: {}", e),
            os: String::new(), uptime: String::new(), cpu_usage: String::new(),
            memory: String::new(), disk: String::new(), load: String::new(), checked_at,
        },
    };
    let output = cmd.output();

    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            let lines: Vec<&str> = stdout.lines().collect();
            RemoteServerStatus {
                id: server.id.clone(), name: server.name.clone(), host: server.host.clone(),
                status: "online".into(),
                hostname: lines.first().unwrap_or(&"unknown").to_string(),
                os: lines.get(1).unwrap_or(&"unknown").to_string(),
                uptime: lines.get(2).unwrap_or(&"unknown").to_string(),
                cpu_usage: format!("{}%", lines.get(3).unwrap_or(&"0").trim()),
                memory: lines.get(4).unwrap_or(&"unknown").to_string(),
                disk: lines.get(5).unwrap_or(&"unknown").to_string(),
                load: lines.get(6).unwrap_or(&"unknown").to_string(),
                checked_at,
            }
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();
            // sshpass swallows ssh's own message, so fall back to its exit code:
            // 5 = password rejected, 6/7 = host key unknown/changed.
            let code_reason = match result.status.code() {
                Some(5) => Some("Auth failed (password rejected)"),
                Some(6) => Some("Host key unknown"),
                Some(7) => Some("Host key changed"),
                _ => None,
            };
            let reason = if stderr.contains("Connection refused") { "Connection refused" }
                else if stderr.contains("timed out") { "Connection timed out" }
                else if stderr.contains("Permission denied") { "Auth failed (Permission denied)" }
                else if stderr.contains("No route to host") { "No route to host" }
                else if stderr.contains("Host key verification") { "Host key verification failed" }
                else if let Some(r) = code_reason { r }
                else { "Connection failed" };
            RemoteServerStatus {
                id: server.id.clone(), name: server.name.clone(), host: server.host.clone(),
                status: "offline".into(), hostname: reason.into(),
                os: String::new(), uptime: String::new(), cpu_usage: String::new(),
                memory: String::new(), disk: String::new(), load: String::new(), checked_at,
            }
        }
        Err(e) => RemoteServerStatus {
            id: server.id.clone(), name: server.name.clone(), host: server.host.clone(),
            status: "offline".into(), hostname: format!("SSH error: {}", e),
            os: String::new(), uptime: String::new(), cpu_usage: String::new(),
            memory: String::new(), disk: String::new(), load: String::new(), checked_at,
        },
    }
}
