use crate::models::UserRole;

/// Check if a role is allowed to access a given method+path
pub fn check_permission(role: &UserRole, method: &str, path: &str) -> bool {
    if in_scope(path, "/api/backup") || in_scope(path, "/api/audit") || in_scope(path, "/api/apikeys") {
        return *role == UserRole::Admin;
    }
    if path == "/api/diagnostics/support-report" {
        return *role == UserRole::Admin && method == "POST";
    }
    match role {
        UserRole::Admin => true,
        UserRole::Operator => is_operator_allowed(method, path),
        UserRole::Viewer => is_viewer_allowed(method, path),
    }
}

fn in_scope(path: &str, scope: &str) -> bool {
    path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|rest| rest.starts_with('/'))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_can_do_everything() {
        assert!(check_permission(
            &UserRole::Admin,
            "GET",
            "/api/server/status"
        ));
        assert!(check_permission(&UserRole::Admin, "POST", "/api/users"));
        assert!(check_permission(
            &UserRole::Admin,
            "DELETE",
            "/api/users/123"
        ));
    }

    #[test]
    fn viewer_read_only() {
        assert!(check_permission(
            &UserRole::Viewer,
            "GET",
            "/api/server/status"
        ));
        assert!(!check_permission(
            &UserRole::Viewer,
            "POST",
            "/api/packages/install"
        ));
        assert!(!check_permission(
            &UserRole::Viewer,
            "DELETE",
            "/api/users/123"
        ));
        assert!(!check_permission(&UserRole::Viewer, "GET", "/api/terminal"));
        assert!(!check_permission(
            &UserRole::Viewer,
            "GET",
            "/api/terminal/sessions/abc/scrollback"
        ));
        assert!(!check_permission(
            &UserRole::Viewer,
            "GET",
            "/api/files/download"
        ));
        assert!(!check_permission(
            &UserRole::Viewer,
            "GET",
            "/api/config/mysql"
        ));
        assert!(!check_permission(
            &UserRole::Viewer,
            "GET",
            "/api/vhost/config/example.com"
        ));
        assert!(check_permission(
            &UserRole::Viewer,
            "GET",
            "/api/config/services"
        ));
        // Viewer can change own password and logout
        assert!(check_permission(
            &UserRole::Viewer,
            "POST",
            "/api/auth/change-password"
        ));
        assert!(check_permission(
            &UserRole::Viewer,
            "POST",
            "/api/auth/logout"
        ));
        assert!(check_permission(
            &UserRole::Viewer,
            "POST",
            "/api/auth/2fa/setup"
        ));
    }

    #[test]
    fn operator_limited_write() {
        assert!(check_permission(
            &UserRole::Operator,
            "GET",
            "/api/server/status"
        ));
        assert!(check_permission(
            &UserRole::Operator,
            "POST",
            "/api/services/action"
        ));
        assert!(check_permission(
            &UserRole::Operator,
            "POST",
            "/api/containers/restart"
        ));
        // Operator blocked from user/account management
        assert!(!check_permission(&UserRole::Operator, "POST", "/api/users"));
        assert!(!check_permission(
            &UserRole::Operator,
            "POST",
            "/api/accounts"
        ));
        assert!(!check_permission(
            &UserRole::Operator,
            "GET",
            "/api/files/download"
        ));
        assert!(!check_permission(
            &UserRole::Operator,
            "GET",
            "/api/terminal"
        ));
        assert!(!check_permission(
            &UserRole::Operator,
            "POST",
            "/api/cron/add"
        ));
        assert!(!check_permission(
            &UserRole::Operator,
            "POST",
            "/api/backup/restore"
        ));
        assert!(!check_permission(
            &UserRole::Operator,
            "POST",
            "/api/packages/install"
        ));
        assert!(!check_permission(
            &UserRole::Operator,
            "POST",
            "/api/remote-servers/123/exec"
        ));
        assert!(check_permission(
            &UserRole::Operator,
            "POST",
            "/api/packages/search"
        ));
        assert!(check_permission(
            &UserRole::Operator,
            "POST",
            "/api/containers/restart"
        ));
    }
}
