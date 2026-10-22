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
fn an_assigned_server_is_used_but_never_edited_and_ends_with_its_assignment() {
    let mut m = member();
    let shared = server("", "example.com");
    m.memberships = vec![serde_json::from_value(serde_json::json!({
        "kind": "server", "scope_id": "srv", "role": "viewer"
    }))
    .unwrap()];
    assert!(listed(&m, &shared) && usable(&m, &shared) && !manageable(&m, &shared));
    // Assigning someone else's own server gives nothing.
    assert!(!usable(&m, &server("m2", "example.com")));
    m.memberships[0].enabled = false;
    assert!(!listed(&m, &shared) && !usable(&m, &shared));
    m.memberships[0].enabled = true;
    m.memberships[0].expires_at = Some("2000-01-01T00:00:00Z".into());
    assert!(!usable(&m, &shared));
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
        // Their share links: whether they may make one is checked where it is made.
        ("GET", "/api/terminal/share-links"),
        ("POST", "/api/terminal/sessions/pty_1/share-link"),
        ("DELETE", "/api/terminal/share-links/abc"),
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
        ("POST", "/api/terminal/sessions/pty_1/share"),
        ("POST", "/api/terminal/files/srv_x/share-link"),
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
fn a_member_shares_only_what_an_administrator_granted() {
    let mut m = member();
    assert!(!may_share(&m, true, false) && !may_share(&m, false, true));
    m.remote_permissions.share_sessions = true;
    assert!(may_share(&m, true, false));
    // A session's link with its files needs the files permission too.
    assert!(!may_share(&m, true, true) && !may_share(&m, false, true));
    m.remote_permissions.share_files = true;
    assert!(may_share(&m, true, true) && may_share(&m, false, true));
    m.status = crate::models::AccountStatus::Suspended;
    assert!(!may_share(&m, true, false));
    let before = m.remote_permissions.clone();
    m.remote_permissions.share_files = false;
    assert!(m.remote_permissions.narrowed_from(&before) && !before.narrowed_from(&m.remote_permissions));
}

#[test]
fn what_an_administrator_switches_off_is_closed() {
    let mut m = member();
    m.remote_permissions.dictation = false;
    m.remote_permissions.register_servers = false;
    m.remote_permissions.copy_between_servers = false;
    for (method, path) in [
        ("GET", "/api/terminal/dictation"),
        ("GET", "/api/terminal/dictation/settings"),
        ("POST", "/api/remote-servers"),
        ("POST", "/api/terminal/copies"),
    ] {
        assert!(!permitted(&m, method, path), "{method} {path}");
    }
    assert!(permitted(&m, "GET", "/api/terminal/copies"));
}

#[test]
fn sessions_keep_within_the_members_limits() {
    let mut m = member();
    assert_eq!(session_terms(&m, true, 0), (true, 0));
    m.remote_permissions.keep_sessions = false;
    assert_eq!(session_terms(&m, true, 0), (false, DEFAULT_IDLE_SECS));
    assert_eq!(session_terms(&m, false, 600), (false, 600));
    m.remote_permissions.keep_sessions = true;
    m.remote_permissions.max_idle_minutes = 30;
    assert_eq!(session_terms(&m, true, 0), (false, 1800));
    assert_eq!(session_terms(&m, false, 7200), (false, 1800));
    assert_eq!(session_terms(&m, false, 60), (false, 60));
    assert!(session_limit(&m, 99).is_none());
    m.remote_permissions.max_sessions = 2;
    assert!(session_limit(&m, 1).is_none() && session_limit(&m, 2).is_some());
    let mut admin = m.clone();
    admin.role = UserRole::Admin;
    assert_eq!(session_terms(&admin, true, 0), (true, 0));
    assert!(session_limit(&admin, 50).is_none());
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

#[test]
fn a_members_links_keep_to_the_terms_set_for_them() {
    let mut m = member();
    assert!(share_terms(&m, None, false, false, true).is_ok());
    m.remote_permissions.share_max_days = 7;
    assert!(share_terms(&m, None, false, false, false).is_err(), "no permanent links");
    assert!(share_terms(&m, Some(8.0), false, false, false).is_err());
    assert!(share_terms(&m, Some(7.0), false, false, false).is_ok());
    m.remote_permissions.share_require_password = true;
    assert!(share_terms(&m, Some(1.0), false, false, false).is_err());
    assert!(share_terms(&m, Some(1.0), true, false, false).is_ok());
    m.remote_permissions.share_view_only = true;
    assert!(share_terms(&m, Some(1.0), true, false, false).is_err());
    assert!(share_terms(&m, Some(1.0), true, true, true).is_err());
    assert!(share_terms(&m, Some(1.0), true, true, false).is_ok());
    let mut admin = m.clone();
    admin.role = UserRole::Admin;
    assert!(share_terms(&admin, None, false, false, true).is_ok());
}
