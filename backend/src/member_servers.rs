//! Remote-only members (role `remote`) and the servers they register.
//!
//! A member sees the remote workspace and nothing else, and in it only the
//! servers they added themselves: never the administrators' servers, never
//! this machine's own shell or files. Their servers are reached with the
//! password they gave, never with NabiMan's SSH key or this machine's ssh
//! settings, and only at a public address: not this machine, nor the network
//! it sits in.

use std::net::{IpAddr, ToSocketAddrs};

use actix_web::HttpRequest;

use crate::models::{NabimanUser, RemoteServer, UserRole};

pub fn is_member(user: &NabimanUser) -> bool {
    user.role == UserRole::Remote
}

/// Whether `server` shows in `user`'s lists: a member's own servers for that
/// member, the shared ones for everyone else.
pub fn listed(user: &NabimanUser, server: &RemoteServer) -> bool {
    if is_member(user) {
        server.owner_id == user.id
    } else {
        !server.member_owned()
    }
}

/// Whether `user` may use `server`. An administrator can still reach a
/// member's server by its id; a member only their own.
pub fn usable(user: &NabimanUser, server: &RemoteServer) -> bool {
    !is_member(user) || server.owner_id == user.id
}

/// Whether the signed-in account may open `server`'s files, or with `None`
/// this machine's own.
pub fn request_may_use(req: &HttpRequest, server: Option<&RemoteServer>) -> bool {
    match (crate::auth::verified_role(req), crate::auth::verified_user_id(req)) {
        (Some(UserRole::Remote), Some(id)) => server.is_some_and(|s| s.owner_id == id),
        (Some(_), Some(_)) => true,
        _ => false,
    }
}

fn owns(user: &NabimanUser, id: &str) -> bool {
    crate::remote_servers::find_server(id)
        .ok()
        .flatten()
        .is_some_and(|s| s.owner_id == user.id)
}

/// What a member may call: their account, the terminal and its files, and
/// their own servers. Everything else stays closed.
pub fn permitted(user: &NabimanUser, method: &str, path: &str) -> bool {
    let fixed = match method {
        "GET" => matches!(
            path,
            "/api/users/me"
                | "/api/app-settings"
                | "/api/terminal"
                | "/api/terminal/sessions"
                | "/api/terminal/settings"
                | "/api/terminal/dictation"
                | "/api/terminal/dictation/settings"
                | "/api/terminal/dictation/voiceprint"
                | "/api/terminal/copies"
                | "/api/remote-servers"
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
                | "/api/terminal/copies"
                | "/api/remote-servers"
        ),
        "PUT" => path == "/api/terminal/settings",
        _ => false,
    };
    if fixed {
        return true;
    }
    if let Some(rest) = path.strip_prefix("/api/terminal/sessions/") {
        // Their own sessions (the handlers check whose); sharing stays with
        // the administrators.
        let action = rest.split('/').nth(1).unwrap_or("");
        return !rest.is_empty() && !matches!(action, "share" | "share-link");
    }
    if let Some(rest) = path.strip_prefix("/api/terminal/copies/") {
        return method == "POST" && rest.ends_with("/cancel");
    }
    if let Some(rest) = path.strip_prefix("/api/terminal/sftp/") {
        let id = rest.split('/').next().unwrap_or("");
        return owns(user, id);
    }
    if let Some(rest) = path.strip_prefix("/api/remote-servers/") {
        let mut parts = rest.split('/');
        let id = parts.next().unwrap_or("");
        let action = parts.next();
        if parts.next().is_some() || !owns(user, id) {
            return false;
        }
        return match action {
            None => matches!(method, "PUT" | "DELETE"),
            Some("host-trust") => method == "PUT",
            Some("check" | "detect") => method == "POST",
            Some(_) => false,
        };
    }
    false
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
mod tests {
    use super::*;

    fn member() -> NabimanUser {
        serde_json::from_value(serde_json::json!({
            "id": "m1", "username": "member", "password_hash": "", "role": "remote",
            "created_at": "now", "status": "active"
        }))
        .unwrap()
    }

    fn server(owner: &str, host: &str) -> RemoteServer {
        serde_json::from_value(serde_json::json!({
            "id": "srv", "name": "n", "host": host, "port": 22, "user": "u",
            "auth_method": "password", "tags": [], "memo": "", "created_at": "now",
            "owner_id": owner
        }))
        .unwrap()
    }

    #[test]
    fn a_member_sees_only_their_own_servers_and_others_never_see_them() {
        let m = member();
        let mut admin = m.clone();
        admin.id = "a1".into();
        admin.role = UserRole::Admin;
        let own = server("m1", "example.com");
        let shared = server("", "example.com");
        let someone_elses = server("m2", "example.com");
        assert!(listed(&m, &own) && usable(&m, &own));
        assert!(!listed(&m, &shared) && !usable(&m, &shared));
        assert!(!listed(&m, &someone_elses) && !usable(&m, &someone_elses));
        assert!(listed(&admin, &shared) && !listed(&admin, &own));
        assert!(usable(&admin, &own));
    }

    #[test]
    fn a_member_may_call_the_terminal_and_nothing_of_the_machine() {
        let m = member();
        for (method, path) in [
            ("GET", "/api/terminal"),
            ("POST", "/api/terminal/tickets"),
            ("GET", "/api/terminal/sessions"),
            ("PUT", "/api/terminal/sessions/pty_1/name"),
            ("DELETE", "/api/terminal/sessions/pty_1"),
            ("GET", "/api/remote-servers"),
            ("POST", "/api/remote-servers"),
            ("POST", "/api/auth/change-password"),
            ("GET", "/api/users/me"),
        ] {
            assert!(permitted(&m, method, path), "{method} {path}");
        }
        for (method, path) in [
            ("GET", "/api/server/status"),
            ("GET", "/api/files/list"),
            ("POST", "/api/terminal/claude/chats"),
            ("PUT", "/api/terminal/dictation/settings"),
            ("POST", "/api/terminal/dictation/test"),
            ("POST", "/api/terminal/paste-file"),
            ("GET", "/api/terminal/share-links"),
            ("POST", "/api/terminal/sessions/pty_1/share-link"),
            ("PUT", "/api/terminal/sessions/pty_1/share"),
            ("GET", "/api/terminal/sftp/local/list"),
            ("POST", "/api/remote-servers/check-all"),
            ("GET", "/api/remote-servers/ssh-key"),
            ("POST", "/api/remote-servers/srv_x/exec"),
            ("PUT", "/api/users/m1"),
            ("PUT", "/api/app-settings"),
        ] {
            assert!(!permitted(&m, method, path), "{method} {path}");
        }
    }

    #[test]
    fn a_members_server_is_refused_inside_this_machine_or_its_network() {
        for host in [
            "127.0.0.1", "localhost", "10.1.2.3", "172.16.0.9", "192.168.0.1", "169.254.169.254",
            "100.64.0.1", "0.0.0.0", "::1", "fd00::1", "fe80::1", "::ffff:127.0.0.1",
        ] {
            assert!(dial_host(&server("m1", host), 22).is_err(), "{host}");
        }
        assert_eq!(dial_host(&server("m1", "8.8.8.8"), 22).unwrap(), "8.8.8.8");
        // The administrators' servers are theirs to point anywhere.
        assert_eq!(dial_host(&server("", "127.0.0.1"), 22).unwrap(), "127.0.0.1");
    }

    #[test]
    fn a_members_server_gets_no_keys_or_settings_of_this_machine() {
        let opts = ssh_isolation(&server("m1", "example.com"));
        assert!(opts.windows(2).any(|w| w == ["-F", "/dev/null"]));
        assert!(opts.contains(&"PubkeyAuthentication=no"));
        assert!(ssh_isolation(&server("", "example.com")).is_empty());
    }
}
