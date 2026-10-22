use super::*;
fn user(name: &str, role: UserRole, memberships: Vec<Membership>) -> NabimanUser {
    NabimanUser {
        share_authorization_version: uuid::Uuid::new_v4().to_string(),
        id: name.into(),
        username: name.into(),
        password_hash: "hash".into(),
        role,
        status: AccountStatus::Active,
        memberships,
        requested_memberships: vec![],
        scope_restricted: true,
        created_at: "now".into(),
        last_login: None,
        totp_enabled: false,
        totp_secret: None,
        mfa: Default::default(),
    }
}
fn scope(id: &str, role: UserRole) -> Membership {
    Membership {
        kind: ScopeKind::Server,
        scope_id: id.into(),
        role,
        enabled: true,
        allow_exec: None,
        expires_at: None,
    }
}
#[test]
fn manager_cannot_see_other_scopes_or_super_administrators() {
    let manager = user(
        "manager",
        UserRole::Viewer,
        vec![scope("one", UserRole::Admin)],
    );
    assert!(visible(
        &manager,
        &user(
            "own",
            UserRole::Viewer,
            vec![scope("one", UserRole::Viewer)]
        )
    ));
    assert!(!visible(
        &manager,
        &user(
            "other",
            UserRole::Viewer,
            vec![scope("two", UserRole::Viewer)]
        )
    ));
    assert!(!visible(
        &manager,
        &user(
            "root",
            UserRole::Admin,
            vec![scope("one", UserRole::Viewer)]
        )
    ));
}
#[test]
fn access_is_limited_to_assigned_resources_and_role() {
    let mut u = user(
        "member",
        UserRole::Viewer,
        vec![scope("one", UserRole::Viewer)],
    );
    assert!(crate::scope_access::permission(
        &u,
        "POST",
        "/api/remote-servers/one/check"
    ));
    for path in [
        "/api/remote-servers/two/check",
        "/api/remote-servers/one/exec",
        "/api/users/memberships",
        "/api/remote-servers/check-all",
    ] {
        assert!(!crate::scope_access::permission(&u, "POST", path), "{path}");
    }
    assert!(!crate::scope_access::permission(
        &u,
        "GET",
        "/api/files/download"
    ));
    assert!(!crate::scope_access::permission(&u, "GET", "/api/terminal"));
    u.memberships[0].role = UserRole::Operator;
    assert!(crate::scope_access::permission(
        &u,
        "POST",
        "/api/remote-servers/one/exec"
    ));
    u.status = AccountStatus::Suspended;
    assert!(!crate::scope_access::permission(&u, "GET", "/api/users/me"));
}
#[test]
fn old_accounts_deserialize_active_without_memberships() {
    let u: NabimanUser = serde_json::from_value(serde_json::json!({
        "id":"old", "username":"admin", "password_hash":"hash", "role":"admin", "created_at":"now"
    }))
    .unwrap();
    assert!(u.super_admin());
    assert!(u.memberships.is_empty());
}

#[test]
fn disabled_expired_and_command_limited_memberships_are_enforced() {
    let mut u = user(
        "member",
        UserRole::Viewer,
        vec![scope("one", UserRole::Operator)],
    );
    u.memberships[0].allow_exec = Some(false);
    assert!(!crate::scope_access::permission(
        &u,
        "POST",
        "/api/remote-servers/one/exec"
    ));
    assert!(crate::scope_access::permission(
        &u,
        "POST",
        "/api/remote-servers/one/check"
    ));
    u.memberships[0].enabled = false;
    assert!(!crate::scope_access::permission(
        &u,
        "POST",
        "/api/remote-servers/one/check"
    ));
    u.memberships[0].enabled = true;
    for expiry in ["2000-01-01T00:00:00Z", "invalid"] {
        u.memberships[0].expires_at = Some(expiry.into());
        assert!(u.scope_role(&ScopeKind::Server, "one").is_none());
    }
    u.memberships[0].expires_at = Some("2099-01-01T00:00:00Z".into());
    assert!(u.scope_role(&ScopeKind::Server, "one").is_some());
    u.memberships[0].role = UserRole::Admin;
    assert!(u.manages(&ScopeKind::Server, "one"));
    u.memberships[0].enabled = false;
    assert!(!u.manages(&ScopeKind::Server, "one"));
    assert!(!crate::scope_access::permission(
        &u,
        "PUT",
        "/api/scope-settings/server/one"
    ));
}
#[test]
fn old_memberships_keep_access_without_new_fields() {
    let m: Membership = serde_json::from_value(
        serde_json::json!({"kind":"tenant", "scope_id":"one", "role":"viewer"}),
    )
    .unwrap();
    assert!(m.accessible());
    assert_eq!(m.allow_exec, None);
    assert_eq!(m.expires_at, None);
}

#[test]
fn a_sign_up_password_needs_eight_characters_and_fits_bcrypt() {
    assert!(super::valid_signup_password("abcd1234"));
    assert!(!super::valid_signup_password("abcd123"));
    assert!(super::valid_signup_password("비밀번호여덟자다"), "eight Korean letters are eight characters");
    assert!(!super::valid_signup_password(&"a".repeat(73)), "bcrypt reads only 72 bytes");
}
