//! Remote-only members (role `remote`) and the servers they register.
//!
//! A member sees the remote workspace and nothing else, and in it only the
//! servers they added themselves and those an administrator assigned them
//! (a server membership, with its on/off switch and expiry): never this
//! machine's own shell or files.
//!
//! Their own servers are reached with the password they gave, never with
//! NabiMan's SSH key or this machine's ssh settings, and only at a public
//! address: not this machine, nor the network it sits in. An assigned server
//! is reached the way the administrators set it up, but a member only works
//! on it — a terminal, its files, a check — and never edits it.

use std::net::{IpAddr, ToSocketAddrs};

use actix_web::{web, HttpRequest};

use crate::models::{NabimanUser, RemoteServer, ScopeKind, UserRole};

pub fn is_member(user: &NabimanUser) -> bool {
    user.role == UserRole::Remote
}

/// An administrators' server assigned to `user`, switched on and not expired.
pub fn assigned(user: &NabimanUser, server: &RemoteServer) -> bool {
    !server.member_owned()
        && user
            .memberships
            .iter()
            .any(|m| m.kind == ScopeKind::Server && m.scope_id == server.id && m.accessible())
}

/// Whether `server` shows in `user`'s lists: a member's own and assigned
/// servers for that member, the shared ones for everyone else.
pub fn listed(user: &NabimanUser, server: &RemoteServer) -> bool {
    if is_member(user) {
        usable(user, server)
    } else {
        !server.member_owned()
    }
}

/// Whether `user` may work on `server`: a terminal, its files, a check. An
/// administrator can still reach a member's server by its id.
pub fn usable(user: &NabimanUser, server: &RemoteServer) -> bool {
    !is_member(user) || server.owner_id == user.id || assigned(user, server)
}

/// Whether `user` may change or delete `server`: a member only their own.
pub fn manageable(user: &NabimanUser, server: &RemoteServer) -> bool {
    !is_member(user) || server.owner_id == user.id
}

/// Whether the signed-in account may open `server`'s files, or with `None`
/// this machine's own.
pub fn request_may_use(req: &HttpRequest, server: Option<&RemoteServer>) -> bool {
    match (crate::auth::verified_role(req), crate::auth::verified_user_id(req)) {
        (Some(UserRole::Remote), Some(id)) => server.is_some_and(|s| {
            s.owner_id == id
                || req
                    .app_data::<web::Data<crate::users::UserStore>>()
                    .and_then(|users| users.lock().ok()?.iter().find(|u| u.id == id).cloned())
                    .is_some_and(|u| assigned(&u, s))
        }),
        (Some(_), Some(_)) => true,
        _ => false,
    }
}

fn server(id: &str) -> Option<RemoteServer> {
    crate::remote_servers::find_server(id).ok().flatten()
}

/// Whether `user` may hold a share link: one that opens a terminal session
/// (`session`), grants files (`files`), or both. The administrators always
/// may; a member when an administrator granted it.
pub fn may_share(user: &NabimanUser, session: bool, files: bool) -> bool {
    user.super_admin()
        || (is_member(user)
            && user.active()
            && (!session || user.remote_permissions.share_sessions)
            && (!files || user.remote_permissions.share_files))
}

/// What a member's link must keep to, beyond being allowed at all: how long
/// it may last (`days`, `None` for permanent), a password, and only showing.
pub fn share_terms(user: &NabimanUser, days: Option<f64>, password: bool, read_only: bool, files_write: bool) -> Result<(), String> {
    if !is_member(user) {
        return Ok(());
    }
    let p = &user.remote_permissions;
    if p.share_max_days > 0 && days.is_none_or(|d| d > f64::from(p.share_max_days)) {
        return Err(format!("공유 링크는 {}일까지만 만들 수 있습니다", p.share_max_days));
    }
    if p.share_require_password && !password {
        return Err("공유 링크에는 비밀번호를 걸어야 합니다".into());
    }
    if p.share_view_only && (!read_only || files_write) {
        return Err("공유 링크는 보기 전용으로만 만들 수 있습니다".into());
    }
    Ok(())
}

/// Whether `user` has share links to look after: the administrators, and any
/// member (who may still revoke links once the permission is gone).
pub fn keeps_shares(user: &NabimanUser) -> bool {
    user.super_admin() || (is_member(user) && user.active())
}

/// What a member may call: their account, the terminal and its files, and
/// their own servers. Everything else stays closed.
pub fn permitted(user: &NabimanUser, method: &str, path: &str) -> bool {
    let granted = &user.remote_permissions;
    // What an administrator may switch off for this member.
    if path.starts_with("/api/terminal/dictation") && !granted.dictation {
        return false;
    }
    if method == "POST" && path == "/api/remote-servers" && !granted.register_servers {
        return false;
    }
    if method == "POST" && path == "/api/terminal/copies" && !granted.copy_between_servers {
        return false;
    }
    let fixed = match method {
        "GET" => matches!(
            path,
            "/api/users/me"
                | "/api/terminal/share-links"
                | "/api/app-settings"
                | "/api/terminal"
                | "/api/terminal/sessions"
                | "/api/terminal/settings"
                | "/api/terminal/dictation"
                | "/api/terminal/dictation/settings"
                | "/api/terminal/dictation/voiceprint"
                | "/api/terminal/sftp/columns"
                | "/api/terminal/copies"
                | "/api/remote-servers"
                | "/api/remote/consoles"
                | "/api/remote/console"
        ),
        "POST" => matches!(
            path,
            "/api/auth/logout"
                | "/api/auth/change-password"
                | "/api/auth/activity"
                | "/api/auth/2fa/setup"
                | "/api/auth/2fa/verify-setup"
                | "/api/auth/2fa/disable"
                | "/api/terminal/tickets"
                | "/api/terminal/dictation/speak"
                | "/api/terminal/copies"
                | "/api/remote-servers"
                | "/api/remote/consoles"
        ),
        "PUT" => matches!(path, "/api/terminal/settings" | "/api/terminal/sftp/columns"),
        _ => false,
    };
    if fixed {
        return true;
    }
    // Console registry sub-paths (edit/delete a console, its access, its share
    // links): the console handlers authorize by ownership or admin, so RBAC
    // only lets the request reach them.
    if path.starts_with("/api/remote/consoles/") {
        return true;
    }
    if let Some(rest) = path.strip_prefix("/api/terminal/sessions/") {
        // Their own sessions (the handlers check whose). A share link is
        // theirs to make when granted (checked where it is made); showing a
        // session to the other accounts is not.
        let action = rest.split('/').nth(1).unwrap_or("");
        return !rest.is_empty() && action != "share";
    }
    if let Some(token) = path.strip_prefix("/api/terminal/share-links/") {
        return method == "DELETE" && !token.contains('/');
    }
    if let Some(rest) = path.strip_prefix("/api/terminal/files/") {
        let mut parts = rest.split('/');
        let id = parts.next().unwrap_or("");
        return method == "POST"
            && parts.next() == Some("share-link")
            && parts.next().is_none()
            && server(id).is_some_and(|s| usable(user, &s));
    }
    if let Some(rest) = path.strip_prefix("/api/terminal/copies/") {
        return method == "POST" && rest.ends_with("/cancel");
    }
    if let Some(rest) = path.strip_prefix("/api/terminal/sftp/") {
        let id = rest.split('/').next().unwrap_or("");
        let op = rest.rsplit('/').next().unwrap_or("");
        let writes = ["upload", "upload-part", "upload-finish", "upload-abort", "mkdir", "rename", "delete", "attrs"];
        return server(id).is_some_and(|s| {
            usable(user, &s)
                && (!writes.contains(&op) || s.owner_id == user.id || granted.assigned_file_write)
        });
    }
    if let Some(rest) = path.strip_prefix("/api/remote-servers/") {
        let mut parts = rest.split('/');
        let id = parts.next().unwrap_or("");
        let action = parts.next();
        let Some(s) = server(id).filter(|_| parts.next().is_none()) else {
            return false;
        };
        return match action {
            None => {
                manageable(user, &s) && (method == "DELETE" || (method == "PUT" && granted.register_servers))
            }
            Some("host-trust") => method == "PUT" && manageable(user, &s),
            Some("detect" | "host-key") => method == "POST" && manageable(user, &s),
            Some("check") => method == "POST" && usable(user, &s),
            Some(_) => false,
        };
    }
    false
}

/// An idle session ends after this long when "세션 유지" is not allowed and
/// no shorter limit was set.
const DEFAULT_IDLE_SECS: u64 = 3600;

/// How long a member's session may run idle, and whether it may be kept
/// alive, within what an administrator allows: `(keepalive, timeout_secs)`,
/// where 0 seconds is no limit. Anyone else gets what they asked for.
pub fn session_terms(user: &NabimanUser, keepalive: bool, timeout_secs: u64) -> (bool, u64) {
    if !is_member(user) {
        return (keepalive, timeout_secs);
    }
    let p = &user.remote_permissions;
    let cap = u64::from(p.max_idle_minutes) * 60;
    let keep = keepalive && p.keep_sessions && cap == 0;
    let mut timeout = if keep { 0 } else { timeout_secs };
    if !keep && timeout == 0 && (!p.keep_sessions || cap > 0) {
        timeout = if cap > 0 { cap } else { DEFAULT_IDLE_SECS };
    }
    if cap > 0 {
        timeout = timeout.min(cap);
    }
    (keep, timeout)
}

/// Why a member may not open one more session, when they have `running`.
pub fn session_limit(user: &NabimanUser, running: usize) -> Option<String> {
    let max = user.remote_permissions.max_sessions as usize;
    (is_member(user) && max > 0 && running >= max)
        .then(|| format!("세션은 동시에 {max}개까지 열 수 있습니다. 쓰지 않는 세션을 종료한 뒤 다시 여세요."))
}

/// The largest single upload a member may make, in bytes; `None`: no limit.
pub fn upload_limit(req: &HttpRequest) -> Option<u64> {
    let user = signed_in(req)?;
    let mb = user.remote_permissions.max_upload_mb;
    (is_member(&user) && mb > 0).then(|| u64::from(mb) * 1024 * 1024)
}

/// Whether the signed-in member may change files on `server` (always on their
/// own; on an assigned one when allowed). Everyone else: yes.
pub fn may_write(req: &HttpRequest, server: &RemoteServer) -> bool {
    match signed_in(req) {
        Some(u) if is_member(&u) => server.owner_id == u.id || u.remote_permissions.assigned_file_write,
        _ => true,
    }
}

/// Whether the signed-in account may copy files between servers.
pub fn may_copy(req: &HttpRequest) -> bool {
    signed_in(req).is_none_or(|u| !is_member(&u) || u.remote_permissions.copy_between_servers)
}

fn signed_in(req: &HttpRequest) -> Option<NabimanUser> {
    let id = crate::auth::verified_user_id(req)?;
    req.app_data::<web::Data<crate::users::UserStore>>()?
        .lock()
        .ok()?
        .iter()
        .find(|u| u.id == id)
        .cloned()
}

/// ssh options for a member's server, given before any other so they win:
/// none of this machine's ssh settings, keys or agent, the password alone.
pub fn ssh_isolation(server: &RemoteServer) -> &'static [&'static str] {
    if !server.member_owned() {
        return &[];
    }
    &[
        "-F",
        "/dev/null",
        "-o",
        "PubkeyAuthentication=no",
        "-o",
        "HostbasedAuthentication=no",
        "-o",
        "GSSAPIAuthentication=no",
        "-o",
        "PreferredAuthentications=password,keyboard-interactive",
        "-o",
        "ForwardAgent=no",
        "-o",
        "ClearAllForwardings=yes",
    ]
}

/// Where to connect for `server` on `port`. A member's server is looked up
/// now and refused when any address it has is inside this machine or a
/// private network; the address returned is the one checked, so a second
/// lookup cannot lead somewhere else. Other servers keep their saved name.
pub fn dial_host(server: &RemoteServer, port: u16) -> Result<String, String> {
    if !server.member_owned() {
        return Ok(server.host.clone());
    }
    let addrs: Vec<IpAddr> = (server.host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("{}: 주소를 찾을 수 없습니다 ({e})", server.host))?
        .map(|a| a.ip())
        .collect();
    if let Some(inner) = addrs.iter().find(|ip| !public(**ip)) {
        return Err(format!(
            "{} 은(는) 내부 주소({inner})라서 접속할 수 없습니다. 인터넷에서 닿는 주소를 입력하세요.",
            server.host
        ));
    }
    addrs
        .iter()
        .find(|ip| ip.is_ipv4())
        .or(addrs.first())
        .map(|ip| ip.to_string())
        .ok_or_else(|| format!("{}: 주소를 찾을 수 없습니다", server.host))
}

/// An address on the public internet: not this machine, a private or shared
/// network, link-local (cloud metadata), multicast or reserved space.
fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_unspecified()
                || v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || o[0] == 0
                || (o[0] == 100 && (o[1] & 0xc0) == 64)
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                || (o[0] == 198 && (o[1] & 0xfe) == 18)
                || o[0] >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return public(IpAddr::V4(v4));
            }
            let s = v6.segments();
            !(v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                || s[0] == 0
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80
                || (s[0] == 0x2001 && s[1] == 0x0db8)
                || (s[0] == 0x64 && s[1] == 0xff9b))
        }
    }
}

#[cfg(test)]
#[path = "member_servers_tests.rs"]
mod tests;
