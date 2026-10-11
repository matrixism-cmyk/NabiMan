use super::*;

/// Resolve the SSH target of a new session: either a saved server (`server_id`,
/// which may carry a stored password) or explicit host/port/user parameters.
/// Returns the shell command tmux should run, plus a label for the UI.
pub(super) fn build_ssh_command(
    query: &str,
    keepalive: bool,
    ka_interval: u64,
) -> Result<Option<(String, String, remote_access::RemoteBinding)>, String> {
    let (host, port, user, password, trust, binding) =
        if let Some(server_id) = parse_query_param(query, "server_id") {
            let srv = crate::remote_servers::find_server(&server_id)?
                .ok_or_else(|| "Saved server not found".to_string())?;
            // `no_saved_password=1` connects as if nothing were stored, so the
            // operator can type the password (or a one-time code) in the pane.
            let skip = parse_query_param(query, "no_saved_password")
                .map(|v| v == "1")
                .unwrap_or(false);
            let pw = if skip {
                None
            } else {
                crate::remote_servers::server_password(&srv)
            };
            let binding = remote_access::RemoteBinding::from_server(&srv)?;
            let trust = crate::remote_host_trust::shell_options(&srv)?;
            (srv.host, srv.port.to_string(), srv.user, pw, trust, binding)
        } else if let Some(host) = parse_query_param(query, "ssh_host") {
            let port = parse_query_param(query, "ssh_port").unwrap_or_else(|| "22".into());
            let user = parse_query_param(query, "ssh_user").unwrap_or_else(|| "root".into());
            let saved = crate::remote_servers::load_servers()?
                .into_iter()
                .find(|s| s.host == host && s.port.to_string() == port && s.user == user)
                .ok_or("Register and verify this server before opening a terminal")?;
            let binding = remote_access::RemoteBinding::from_server(&saved)?;
            let trust = crate::remote_host_trust::shell_options(&saved)?;
            (host, port, user, None, trust, binding)
        } else {
            return Ok(None);
        };

    if !validate_ssh_input(&host) || !validate_ssh_input(&port) || !validate_ssh_input(&user) {
        return Err("Invalid SSH params".into());
    }

    let mut cmd = String::new();
    let using_saved_password = password.is_some();
    if let Some(pw) = password {
        // sshpass reads the password from a 0600 file that is deleted shortly
        // after: it never appears in `ps` output or in the environment.
        let pw_file = ssh_auth::write_password_file(&pw)?;
        let path = pw_file.delete_after(PW_FILE_TTL_SECS);
        cmd.push_str(&format!("sshpass -f {} ", path));
    }
    cmd.push_str(&format!("ssh {} ", trust));
    if using_saved_password {
        // Go straight to password auth. Offering keys first can exhaust the
        // server's MaxAuthTries before the password is ever tried, which the
        // server then reports as a plain "Permission denied".
        cmd.push_str(&ssh_auth::password_auth_opts().join(" "));
        cmd.push(' ');
    }
    if keepalive {
        cmd.push_str(&ssh_auth::keepalive_opts(ka_interval).join(" "));
        cmd.push(' ');
    } else {
        cmd.push_str("-o ServerAliveInterval=30 ");
    }
    cmd.push_str(&format!("-p {} {}@{}", port, user, host));

    // When ssh fails (refused, wrong password, unreachable) it exits in well
    // under a second. Without this guard tmux would tear the pane down before
    // the browser ever rendered the reason, and the client — seeing only a
    // closed socket — would reconnect forever. Holding the pane open on a
    // non-zero exit keeps the message on screen; a clean logout still ends the
    // session, which the browser is told about explicitly.
    let hint = if using_saved_password {
        "저장된 비밀번호가 거부되면(exit 5) 서버 편집에서 비밀번호를 다시 저장하거나,          \"직접 입력\" 버튼으로 접속해 보세요."
    } else {
        "위의 메시지에서 원인을 확인하세요."
    };
    let guarded = format!(
        concat!(
            r#"{}; __code=$?; if [ "$__code" -ne 0 ]; then "#,
            r#"printf '
[1;31m[SSH 연결 종료 · exit %s][0m "#,
            r#"{} Enter 키를 누르면 세션이 닫힙니다.
' "$__code"; "#,
            // Bounded wait: the operator can close it with Enter, and an
            // unattended failed session does not linger for ever.
            r#"if command -v timeout >/dev/null 2>&1; then "#,
            r#"timeout --foreground {} sh -c 'read -r __nabiman_wait'; else read -r __nabiman_wait; fi; fi"#,
        ),
        cmd, hint, FAILED_PANE_LINGER_SECS,
    );

    Ok(Some((
        guarded,
        format!("ssh:{}@{}:{}", user, host, port),
        binding,
    )))
}
