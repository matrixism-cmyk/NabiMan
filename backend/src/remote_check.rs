//! Checking a saved server the way it is actually reached, and finding out
//! which ways work: a shell over SSH, SFTP, FTP with or without TLS, Telnet.

use std::time::Duration;

use actix_web::{web, HttpResponse};
use serde::Serialize;

use super::*;
use crate::models::ProtocolCheck;
use crate::remote_probe::bare_status;
use crate::terminal::sftp::{ftp, sftp_check};

fn line(protocol: &str, result: Result<String, String>) -> ProtocolCheck {
    match result {
        Ok(message) => ProtocolCheck { protocol: protocol.into(), ok: true, message },
        Err(message) => ProtocolCheck { protocol: protocol.into(), ok: false, message },
    }
}

async fn tcp(server: &RemoteServer, port: u16) -> Result<String, String> {
    let host = crate::member_servers::dial_host(server, port)?;
    match tokio::time::timeout(Duration::from_secs(5), tokio::net::TcpStream::connect((host.as_str(), port))).await {
        Ok(Ok(_)) => Ok(format!("Port {port} answers")),
        Ok(Err(e)) => Err(format!("Cannot reach port {port}: {e}")),
        Err(_) => Err(format!("Port {port} did not answer")),
    }
}

/// Runs a command over SSH; true when the shell ran it.
async fn shell(server: &RemoteServer) -> Result<String, String> {
    let server = server.clone();
    tokio::task::spawn_blocking(move || {
        let (mut cmd, _pw) = ssh_command(&server, 8, "echo nabiman-shell-ok")?;
        let out = cmd.output().map_err(|e| format!("SSH error: {e}"))?;
        if String::from_utf8_lossy(&out.stdout).contains("nabiman-shell-ok") {
            return Ok("Shell available".into());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        let said = stderr.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("Warning:")).last();
        Err(match said {
            Some(l) if l.to_ascii_lowercase().contains("sftp") => format!("Shell refused: {l}"),
            _ => crate::remote_probe::ssh_failure(&stderr, out.status.code()).to_string(),
        })
    })
    .await
    .unwrap_or_else(|_| Err("The check stopped unexpectedly".into()))
}

/// Checks every way the server is set up to be reached. SSH also reads the
/// machine's vital signs; the others only say whether they work.
pub(crate) async fn probe(server: &RemoteServer) -> RemoteServerStatus {
    let mut checks = Vec::new();
    let mut measured = None;
    match server.terminal.as_str() {
        "ssh" => {
            let s = server.clone();
            let status = tokio::task::spawn_blocking(move || ssh_probe(&s))
                .await
                .unwrap_or_else(|_| bare_status(server, "offline", "The check stopped unexpectedly".into()));
            let ok = status.status == "online";
            checks.push(ProtocolCheck { protocol: "ssh".into(), ok, message: if ok { "Shell available".into() } else { status.hostname.clone() } });
            measured = Some(status);
        }
        "telnet" => checks.push(line("telnet", tcp(server, server.port).await)),
        _ => {}
    }
    match server.files.as_str() {
        // Without SSH there is no SFTP; there is no need to wait for it again.
        "sftp" if checks.first().is_some_and(|c| c.protocol == "ssh" && !c.ok) => checks.push(line("sftp", Err("Not checked: SSH failed".into()))),
        "sftp" => checks.push(line("sftp", sftp_check(server).await.map(|_| "SFTP available".into()))),
        "ftp" | "ftps" => checks.push(line(&server.files, ftp::check(server).await.map(|_| "Logged in".into()))),
        _ => {}
    }
    let ok = checks.iter().filter(|c| c.ok).count();
    let state = if ok == checks.len() { "online" } else if ok == 0 { "offline" } else { "partial" };
    let failure = checks.iter().find(|c| !c.ok).map(|c| c.message.clone());
    let mut status = match measured {
        Some(s) if s.status == "online" => s,
        _ => bare_status(server, state, String::new()),
    };
    status.status = state.into();
    if let Some(reason) = failure.filter(|_| status.hostname.is_empty() || state == "offline") {
        status.hostname = reason;
    }
    status.checks = checks;
    status
}

#[derive(Serialize)]
struct Detected {
    checks: Vec<ProtocolCheck>,
    /// The protocols that work, as the server record should have them.
    terminal: String,
    files: String,
    changed: bool,
}

/// Tries each way of reaching the server that could work and says which
/// do: for SSH, the shell and SFTP separately, since SFTP-only accounts and
/// SSH without SFTP are both common; for plain FTP, whether TLS also works.
pub(super) async fn detect(path: web::Path<String>) -> HttpResponse {
    let server = match find_server(&path) {
        Ok(Some(s)) => s,
        Ok(None) => return HttpResponse::Ok().json(ApiResponse::<()>::error("Server not found")),
        Err(e) => return storage_error(e),
    };
    let (mut terminal, mut files) = (server.terminal.clone(), server.files.clone());
    let mut checks = Vec::new();
    if server.terminal != "telnet" && server.host_trust.is_some() {
        let shell = shell(&server).await;
        if server.uses_ftp() {
            if shell.is_ok() {
                terminal = "ssh".into();
            }
        } else {
            let sftp = sftp_check(&server).await.map(|_| "SFTP available".to_string());
            // Only a working login tells a refused shell from a refused login.
            match (shell.is_ok(), sftp.is_ok()) {
                (true, true) => (terminal, files) = ("ssh".into(), "sftp".into()),
                (false, true) => terminal = "none".into(),
                (true, false) => (terminal, files) = ("ssh".into(), "none".into()),
                (false, false) => {}
            }
            checks.push(line("ssh", shell));
            checks.push(line("sftp", sftp));
            return answer(&server, checks, terminal, files);
        }
        checks.push(line("ssh", shell));
    } else if server.uses_ssh() {
        checks.push(line("ssh", Err("Approve the host key first".into())));
    }
    if server.terminal == "telnet" {
        checks.push(line("telnet", tcp(&server, server.port).await));
    }
    if server.uses_ftp() {
        checks.push(line(&server.files, ftp::check(&server).await.map(|_| "Logged in".into())));
        if server.files == "ftp" && ftp::offers_tls(&server).await == Ok(true) {
            // Suggest TLS only once it has been seen to work.
            let mut tls = server.clone();
            tls.files = "ftps".into();
            let result = ftp::check(&tls).await.map(|_| "TLS works too".to_string());
            if result.is_ok() {
                files = "ftps".into();
            }
            checks.push(line("ftps", result));
        }
    }
    answer(&server, checks, terminal, files)
}

fn answer(server: &RemoteServer, checks: Vec<ProtocolCheck>, terminal: String, files: String) -> HttpResponse {
    let changed = terminal != server.terminal || files != server.files;
    HttpResponse::Ok().json(ApiResponse::ok(Detected { checks, terminal, files, changed }))
}
