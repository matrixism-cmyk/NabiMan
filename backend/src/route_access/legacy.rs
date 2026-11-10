//! The rules `rbac` had before the allowlist, kept as the reference the
//! route-access tests compare against: on every registered route, the
//! allowlist must decide exactly as these did.

use crate::models::UserRole;

fn in_scope(path: &str, scope: &str) -> bool {
    path == scope || path.strip_prefix(scope).is_some_and(|rest| rest.starts_with('/'))
}

pub(super) fn check(role: &UserRole, method: &str, path: &str) -> bool {
    // `/metrics` and `/api/usage` came after the allowlist; they were admin-only from the start.
    if in_scope(path, "/api/backup") || in_scope(path, "/api/audit") || in_scope(path, "/api/apikeys") || path == "/metrics" || path == "/api/usage" {
        return *role == UserRole::Admin;
    }
    if path == "/api/diagnostics/support-report" {
        return *role == UserRole::Admin && method == "POST";
    }
    match role {
        UserRole::Admin => true,
        UserRole::Operator => is_operator_allowed(method, path),
        UserRole::Viewer => is_viewer_allowed(method, path),
        UserRole::Remote => false,
    }
}

fn is_viewer_allowed(method: &str, path: &str) -> bool {
    // A terminal and root-owned files remain privileged even when read with GET.
    if method == "GET" {
        if path == "/api/terminal"
            || path.starts_with("/api/terminal/")
            || path == "/api/files"
            || path.starts_with("/api/files/")
            || path == "/api/vhost/config"
            || path.starts_with("/api/vhost/config/")
        {
            return false;
        }
        if path.starts_with("/api/config/") && path != "/api/config/services" {
            return false;
        }
        return true;
    }
    if matches!(
        path,
        "/api/auth/change-password"
            | "/api/auth/logout"
            | "/api/auth/activity"
            | "/api/auth/2fa/setup"
            | "/api/auth/2fa/verify-setup"
            | "/api/auth/2fa/disable"
    ) {
        return true;
    }
    false
}

fn is_operator_allowed(method: &str, path: &str) -> bool {
    // These endpoints can read credentials, run arbitrary commands, or rewrite
    // root-owned files. They must not be available to operational accounts.
    if ["/api/terminal", "/api/files"]
        .iter()
        .any(|scope| in_scope(path, scope))
    {
        return false;
    }
    if in_scope(path, "/api/config") {
        return path == "/api/config/services" && method == "GET";
    }
    if in_scope(path, "/api/vhost/config") {
        return false;
    }
    if method == "GET" {
        return true;
    }
    if matches!(
        path,
        "/api/auth/change-password"
            | "/api/auth/logout"
            | "/api/auth/activity"
            | "/api/auth/2fa/setup"
            | "/api/auth/2fa/verify-setup"
            | "/api/auth/2fa/disable"
    ) {
        return true;
    }
    if path == "/api/packages/search" || path == "/api/logs" {
        return true;
    }
    if [
        "/api/users",
        "/api/accounts",
        "/api/cron",
        "/api/backup",
        "/api/packages",
        "/api/remote-servers",
        "/api/vhost",
        "/api/database",
        "/api/apikeys",
        "/api/auth/sessions",
        "/api/auth/ldap",
        "/api/auth/oauth",
    ]
    .iter()
    .any(|scope| in_scope(path, scope))
    {
        return false;
    }
    true
}

