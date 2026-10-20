use super::*;

pub(super) fn save_probe_results(
    snapshot: &[RemoteServer],
    results: &[RemoteServerStatus],
) -> Result<(), String> {
    let _guard = SERVER_WRITES.lock().unwrap_or_else(|e| e.into_inner());
    let mut latest = load_servers()?;
    merge_probe_results(&mut latest, snapshot, results);
    save_servers(&mut latest)
}

pub(super) fn merge_probe_results(
    latest: &mut [RemoteServer],
    snapshot: &[RemoteServer],
    results: &[RemoteServerStatus],
) {
    for result in results {
        if let Some(server) = latest.iter_mut().find(|s| s.id == result.id) {
            // A probe of the old target says nothing about an edited target.
            let unchanged = snapshot.iter().any(|old| {
                old.id == server.id
                    && old.host == server.host
                    && old.port == server.port
                    && old.user == server.user
                    && old.auth_method == server.auth_method
                    && old.password_enc == server.password_enc
                    && old.terminal == server.terminal
                    && old.files == server.files
                    && old.ftp_port == server.ftp_port
                    && old.tls_fingerprint == server.tls_fingerprint
                    && old.host_trust_version == server.host_trust_version
                    && old.host_trust.as_ref().map(|k| &k.fingerprint)
                        == server.host_trust.as_ref().map(|k| &k.fingerprint)
            });
            if unchanged {
                server.status = result.status.clone();
                server.last_checked = result.checked_at.clone();
            }
        }
    }
}

pub(super) async fn check_server(path: web::Path<String>) -> HttpResponse {
    let server_id = path.into_inner();
    let servers = match load_servers() {
        Ok(servers) => servers,
        Err(error) => return storage_error(error),
    };
    let srv = match servers.iter().find(|s| s.id == server_id) {
        Some(s) => s.clone(),
        None => {
            return HttpResponse::Ok()
                .json(ApiResponse::<RemoteServerStatus>::error("Server not found"))
        }
    };
    let result = super::check::probe(&srv).await;
    if let Err(error) = save_probe_results(&servers, std::slice::from_ref(&result)) {
        return storage_error(error);
    }
    HttpResponse::Ok().json(ApiResponse::ok(result))
}

pub(super) async fn check_all_servers() -> HttpResponse {
    let servers = match load_servers() {
        Ok(servers) => servers,
        Err(error) => return storage_error(error),
    };
    let results: Vec<RemoteServerStatus> =
        futures::future::join_all(servers.iter().map(super::check::probe)).await;
    if let Err(error) = save_probe_results(&servers, &results) {
        return storage_error(error);
    }
    HttpResponse::Ok().json(ApiResponse::ok(results))
}

pub(super) async fn exec_on_server(
    path: web::Path<String>,
    body: web::Json<RemoteExecRequest>,
) -> HttpResponse {
    let server_id = path.into_inner();
    let servers = match load_servers() {
        Ok(servers) => servers,
        Err(error) => return storage_error(error),
    };
    let srv = match servers.iter().find(|s| s.id == server_id) {
        Some(s) => s,
        None => return HttpResponse::Ok().json(ApiResponse::<String>::error("Server not found")),
    };
    if srv.terminal != "ssh" {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Commands run over SSH, which this server does not offer"));
    }

    let cmd = body.command.trim();
    if cmd.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Empty command"));
    }
    if cmd.len() > 1024 {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Command too long"));
    }
    let dangerous = ["rm -rf /", "mkfs", "dd if=", "> /dev/sd", ":(){ :|:& };:"];
    for pattern in &dangerous {
        if cmd.contains(pattern) {
            return HttpResponse::Ok()
                .json(ApiResponse::<String>::error("Dangerous command blocked"));
        }
    }

    let (mut ssh, _pw_guard) = match ssh_command(srv, 10, cmd) {
        Ok(c) => c,
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
    };
    let output = ssh.output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();
            let combined = if stderr.is_empty() {
                stdout
            } else if stdout.is_empty() {
                stderr
            } else {
                format!("{}\n--- stderr ---\n{}", stdout, stderr)
            };
            if result.status.success() {
                HttpResponse::Ok().json(ApiResponse::ok(combined))
            } else {
                HttpResponse::Ok().json(ApiResponse::<String>::error(&combined))
            }
        }
        Err(e) => {
            HttpResponse::Ok().json(ApiResponse::<String>::error(&format!("SSH error: {}", e)))
        }
    }
}
