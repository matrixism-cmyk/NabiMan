use super::*;

/// Resolve the SSH target of a new session: either a saved server (`server_id`,
/// which may carry a stored password) or explicit host/port/user parameters.
/// Returns the shell command tmux should run, plus a label for the UI.
///
/// `member` is the id of a remote-only member opening it: they may open only
/// their own saved servers, never a shell on this machine.
pub(super) fn build_ssh_command(
    query: &str,
    keepalive: bool,
    ka_interval: u64,
    member: Option<&str>,
) -> Result<Option<(String, String, remote_access::RemoteBinding)>, String> {
    let (host, port, user, password, trust, binding, isolation) =
        if let Some(server_id) = parse_query_param(query, "server_id") {
            let srv = crate::remote_servers::find_server(&server_id)?
                .filter(|s| member.is_none_or(|id| s.owner_id == id))
                .ok_or_else(|| "Saved server not found".to_string())?;
            match srv.terminal.as_str() {
                "ssh" => {}
                "telnet" => return build_telnet_command(&srv, query).map(Some),
                _ => return Err("이 서버에는 터미널 접속 방식이 없습니다. 파일 탭을 이용하세요.".into()),
            }
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
            let host = crate::member_servers::dial_host(&srv, srv.port)?;
            let isolation = crate::member_servers::ssh_isolation(&srv);
            (host, srv.port.to_string(), srv.user, pw, trust, binding, isolation)
        } else if member.is_some() {
            return Err("원격 전용 회원은 등록한 서버로만 접속할 수 있습니다.".into());
        } else if let Some(host) = parse_query_param(query, "ssh_host") {
            let port = parse_query_param(query, "ssh_port").unwrap_or_else(|| "22".into());
            let user = parse_query_param(query, "ssh_user").unwrap_or_else(|| "root".into());
            let saved = crate::remote_servers::load_servers()?
                .into_iter()
                .find(|s| s.terminal == "ssh" && s.host == host && s.port.to_string() == port && s.user == user && !s.member_owned())
                .ok_or("Register and verify this server before opening a terminal")?;
            let binding = remote_access::RemoteBinding::from_server(&saved)?;
            let trust = crate::remote_host_trust::shell_options(&saved)?;
            (host, port, user, None, trust, binding, &[][..])
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
    cmd.push_str("ssh ");
    for opt in isolation {
        cmd.push_str(opt);
        cmd.push(' ');
    }
    cmd.push_str(&format!("{} ", trust));
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

    let hint = if using_saved_password {
        "저장된 비밀번호가 거부되면(exit 5) 서버 편집에서 비밀번호를 다시 저장하거나,          \"직접 입력\" 버튼으로 접속해 보세요."
    } else {
        "위의 메시지에서 원인을 확인하세요."
    };
    let guarded = guard(&cmd, "SSH", hint);
    Ok(Some((
        guarded,
        format!("ssh:{}@{}:{}", user, host, port),
        binding,
    )))
}

/// Keeps a pane whose connection failed open until the reason has been read.
///
/// When ssh or telnet fails (refused, wrong password, unreachable) it exits in
/// well under a second. Without this guard tmux would tear the pane down
/// before the browser ever rendered the reason, and the client — seeing only a
/// closed socket — would reconnect forever. Holding the pane open on a
/// non-zero exit keeps the message on screen; a clean logout still ends the
/// session, which the browser is told about explicitly.
fn guard(cmd: &str, what: &str, hint: &str) -> String {
    format!(
        concat!(
            r#"{}; __code=$?; if [ "$__code" -ne 0 ]; then "#,
            r#"printf '\r\n\033[1;31m[{} 연결 종료 · exit %s]\033[0m "#,
            r#"{} Enter 키를 누르면 세션이 닫힙니다.\r\n' "$__code"; "#,
            // Bounded wait: the operator can close it with Enter, and an
            // unattended failed session does not linger for ever.
            r#"if command -v timeout >/dev/null 2>&1; then "#,
            r#"timeout --foreground {} sh -c 'read -r __nabiman_wait'; else read -r __nabiman_wait; fi; fi"#,
        ),
        cmd, what, hint, FAILED_PANE_LINGER_SECS,
    )
}

/// The Telnet client: this very binary, started with its client subcommand.
fn telnet_client() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|_| "Cannot locate the Telnet client")?;
    // A binary replaced while running reads back as "… (deleted)"; the new
    // one at the same path is the one to start.
    let exe = exe.to_string_lossy().trim_end_matches(" (deleted)").to_string();
    Ok(format!("{} {}", quote(&exe), super::telnet_client::SUBCOMMAND))
}

fn quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', "'\\''"))
}

/// A saved Telnet server. Telnet sends everything, the password included, as
/// plain text; the client logs in with the stored password when the server
/// asks for one, and otherwise the operator types it.
fn build_telnet_command(
    srv: &crate::models::RemoteServer,
    query: &str,
) -> Result<(String, String, remote_access::RemoteBinding), String> {
    let binding = remote_access::RemoteBinding::from_server(srv)?;
    if !validate_ssh_input(&srv.host) || !validate_ssh_input(&srv.user) {
        return Err("Invalid Telnet params".into());
    }
    let skip = parse_query_param(query, "no_saved_password").is_some_and(|v| v == "1");
    let host = crate::member_servers::dial_host(srv, srv.port)?;
    let mut cmd = format!("{} --host {} --port {} --user {}", telnet_client()?, quote(&host), srv.port, quote(&srv.user));
    let password = if skip { None } else { crate::remote_servers::server_password(srv) };
    if let Some(pw) = password {
        let pw_file = ssh_auth::write_password_file(&pw)?;
        cmd.push_str(&format!(" --password-file {}", quote(&pw_file.delete_after(PW_FILE_TTL_SECS))));
    }
    let hint = "Telnet은 암호화되지 않습니다. 위의 메시지에서 원인을 확인하세요.";
    Ok((guard(&cmd, "Telnet", hint), format!("telnet:{}@{}:{}", srv.user, srv.host, srv.port), binding))
}
