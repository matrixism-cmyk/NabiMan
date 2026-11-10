//! `nabiman-claude open --server`: a new SSH tile to a saved server that
//! starts Claude Code there, in a directory, as the server's saved user.
//!
//! It is the dashboard's SSH tile, built by the same code (host key
//! approval, saved password or certificate, the member rules, the shared
//! connection the Claude probe looks through), with one difference: ssh is
//! given a command — a login shell that changes to the directory and runs
//! `claude` — and the shell stays when Claude Code exits. The new tile
//! belongs to the account whose tile asked, like a local one.

use crate::lock_safe::LockExt;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::claude_control_ops::{Caller, Ctx};
use super::session::{self, TmuxSession};
use crate::models::RemoteServer;

const MAX_DIR: usize = 1024;
const SCROLLBACK: u32 = 5000;
/// The probe looks every 15 s, and only once the connection is up.
const ADDRESS_WAIT: Duration = Duration::from_secs(40);

/// What `open --server` asks for.
pub(crate) struct RemoteOpen {
    pub server: String,
    pub dir: String,
    pub title: Option<String>,
    pub resume: bool,
    pub skip_permissions: bool,
    pub prompt: Option<String>,
    pub wait: bool,
}

fn refuse(why: impl Into<String>) -> Value {
    json!({"ok": false, "error": why.into()})
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// A directory on the server: absolute, or under its user's home (`~`, `~/…`).
pub(crate) fn valid_dir(dir: &str) -> bool {
    !dir.is_empty() && dir.len() <= MAX_DIR && !dir.chars().any(char::is_control) && (dir.starts_with('/') || dir == "~" || dir.starts_with("~/"))
}

/// What the server's shell runs: a login shell (so PATH finds `claude` as
/// when the user signs in) that goes to `dir`, runs Claude Code, and stays.
pub(crate) fn remote_command(dir: &str, skip_permissions: bool, resume: bool, prompt: Option<&str>) -> String {
    let mut claude = String::from("claude");
    if skip_permissions {
        claude.push_str(" --dangerously-skip-permissions");
    }
    if resume {
        claude.push_str(" -c");
    }
    if let Some(p) = prompt.filter(|p| !p.is_empty()) {
        claude.push_str(" -- ");
        claude.push_str(&quote(p));
    }
    let cd = match dir.strip_prefix("~/") {
        _ if dir == "~" => "cd".to_string(),
        Some(rest) => format!("cd \"$HOME\"/{}", quote(rest)),
        None => format!("cd {}", quote(dir)),
    };
    let inner = format!("{cd} && {claude}; exec \"${{SHELL:-/bin/sh}}\" -l");
    format!("exec \"${{SHELL:-/bin/sh}}\" -l -c {}", quote(&inner))
}

/// The saved server `name` is: by id, else by its name (which must be unique).
pub(crate) fn pick<'a>(servers: &'a [RemoteServer], name: &str) -> Result<&'a RemoteServer, String> {
    if let Some(s) = servers.iter().find(|s| s.id == name) {
        return Ok(s);
    }
    let named: Vec<&RemoteServer> = servers.iter().filter(|s| s.name == name).collect();
    match named.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("no saved server is called {name:?}")),
        many => Err(format!(
            "{} saved servers are called {name:?}; name one by its id: {}",
            many.len(),
            many.iter().map(|s| s.id.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

pub(crate) fn open(ask: RemoteOpen, caller: &Caller, ctx: &Ctx) -> Value {
    let dir = ask.dir.trim().to_string();
    if !valid_dir(&dir) {
        return refuse(format!("the directory on the server must be absolute (or ~/…): {dir:?}"));
    }
    if ask.prompt.as_ref().is_some_and(|p| p.len() > 8 * 1024) {
        return refuse("the first message is too long");
    }
    let Some(account) = ctx.users.lock_safe().iter().find(|u| u.id == caller.account_id).cloned() else {
        return refuse("the calling session's NabiMan account is gone");
    };
    if !crate::scope_access::permission(&account, "GET", "/api/terminal") {
        return refuse("this account may not open terminals");
    }
    let member = crate::member_servers::is_member(&account).then_some(&account);
    let servers = match crate::remote_servers::load_servers() {
        Ok(s) => s,
        Err(e) => return refuse(e),
    };
    let server = match pick(&servers, ask.server.trim()) {
        Ok(s) if member.is_none_or(|m| crate::member_servers::usable(m, s)) => s.clone(),
        Ok(_) => return refuse(format!("no saved server is called {:?}", ask.server)),
        Err(e) => return refuse(e),
    };
    if server.terminal != "ssh" {
        return refuse(format!("{} is not reached over SSH; Claude Code can only be started over SSH", server.name));
    }
    if !server.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') || server.id.is_empty() {
        return refuse("that saved server has an id this cannot pass on");
    }
    let (keepalive, timeout_secs) = member.map_or((true, 0), |m| crate::member_servers::session_terms(m, true, 0));
    let interval = super::settings::TerminalSettings::default().keepalive_interval_secs;
    let sid = session::generate_session_id();
    let remote = remote_command(&dir, ask.skip_permissions, ask.resume, ask.prompt.as_deref().filter(|p| !p.trim().is_empty()));
    let built = super::ssh_command::build_ssh_command(&format!("server_id={}", server.id), keepalive, interval, member, Some(&sid), Some(&remote));
    let (cmd, label, binding) = match built {
        Ok(Some(b)) => b,
        Ok(None) => return refuse("that saved server has no terminal"),
        Err(e) => return refuse(e),
    };
    let title: String = ask
        .title
        .unwrap_or_else(|| format!("{}:{}", server.name, dir.rsplit('/').find(|p| !p.is_empty()).unwrap_or("claude")))
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect();
    let tmux_name = {
        let mut map = ctx.store.lock_safe();
        let mine = map.values().filter(|s| s.owned_by(&account.id)).count();
        if let Some(why) = member.and_then(|m| crate::member_servers::session_limit(m, mine)) {
            return refuse(why);
        }
        let safe_user: String = caller.account.chars().filter(|c| c.is_alphanumeric() || *c == '_').take(64).collect();
        let tmux_name = format!("nb_{safe_user}_{sid}");
        let candidate = match super::registry::prepare_insert(
            &map,
            sid.clone(),
            TmuxSession {
                tmux_name: tmux_name.clone(),
                owner: caller.account.clone(),
                owner_id: account.id.clone(),
                label,
                ssh_cmd: Some(cmd.clone()),
                remote_binding: Some(binding),
                created_unix: session::now_unix(),
                last_active_unix: session::now_unix(),
                ws_count: 0,
                shared: false,
                timeout_secs,
                scrollback: SCROLLBACK,
                keepalive,
                title: title.clone(),
            },
        ) {
            Ok(c) => c,
            Err(e) => return refuse(e.to_string()),
        };
        if !super::remote_access::allowed(&candidate[&sid]) {
            return refuse("the server's approval changed; approve its host key again");
        }
        let size = (200, 50);
        if let Err(e) = session::create_tmux_session(&tmux_name, Some(&cmd), SCROLLBACK, size) {
            return refuse(format!("tmux: {e}"));
        }
        *map = candidate;
        session::persist(&map);
        if let Some(s) = map.get(&sid) {
            super::recording::start(&tmux_name, &sid, &s.owner, &s.owner_id, &s.label, size);
        }
        tmux_name
    };
    let claude = if ask.wait { wait_for_claude(ctx, &tmux_name) } else { None };
    json!({"ok": true, "session_id": sid, "title": title, "server": server.name, "claude": claude})
}

/// The new tile's Claude address, once the probe has seen it there.
fn wait_for_claude(ctx: &Ctx, tmux_name: &str) -> Option<String> {
    let until = Instant::now() + ADDRESS_WAIT;
    while Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2000));
        super::claude_remote::want(&ctx.store);
        if let Some(p) = super::claude_peers::look().get(tmux_name).and_then(|s| s.claude.first().cloned()) {
            return Some(p.address);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_runs_claude_in_a_login_shell_and_keeps_it() {
        let c = remote_command("/srv/my app", true, false, Some("it's here"));
        assert_eq!(
            c,
            r#"exec "${SHELL:-/bin/sh}" -l -c 'cd '\''/srv/my app'\'' && claude --dangerously-skip-permissions -- '\''it'\''\'\'''\''s here'\''; exec "${SHELL:-/bin/sh}" -l'"#
        );
        assert!(remote_command("~/x", false, true, None).contains(r#"cd "$HOME"/'\''x'\'' && claude -c;"#));
        assert!(remote_command("~", false, false, None).contains("'cd && claude;"));
    }

    #[test]
    fn a_server_directory_is_absolute_or_under_home() {
        assert!(valid_dir("/w") && valid_dir("~") && valid_dir("~/p"));
        assert!(!valid_dir("w") && !valid_dir("") && !valid_dir("~other") && !valid_dir("/a\nb"));
    }

    #[test]
    fn a_server_is_named_by_id_or_by_a_unique_name() {
        let s = |id: &str, name: &str| -> RemoteServer {
            serde_json::from_value(serde_json::json!({"id":id,"name":name,"host":"h","port":22,"user":"u","auth_method":"key","tags":[],"memo":"","created_at":"now"})).unwrap()
        };
        let all = vec![s("a1", "gpu"), s("b2", "gpu"), s("c3", "web")];
        assert_eq!(pick(&all, "web").unwrap().id, "c3");
        assert_eq!(pick(&all, "b2").unwrap().id, "b2");
        let e = pick(&all, "gpu").map(|s| s.id.clone()).unwrap_err();
        assert!(e.contains("a1") && e.contains("b2"), "{e}");
        assert!(pick(&all, "none").is_err());
    }
}
