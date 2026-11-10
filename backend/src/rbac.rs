use crate::models::UserRole;

/// Check if a role is allowed to access a given method+path. Operators and
/// viewers get only what `route_access` lists for the route; a path it does
/// not list is refused to them.
pub fn check_permission(role: &UserRole, method: &str, path: &str) -> bool {
    if in_scope(path, "/api/backup") || in_scope(path, "/api/audit") || in_scope(path, "/api/apikeys") {
        return *role == UserRole::Admin;
    }
    if path == "/api/diagnostics/support-report" {
        return *role == UserRole::Admin && method == "POST";
    }
    match role {
        UserRole::Admin => true,
        UserRole::Operator | UserRole::Viewer => crate::route_access::staff_allowed(role, method, path),
        // Remote-only members are let in by `member_servers`, never by role.
        UserRole::Remote => false,
    }
}

fn in_scope(path: &str, scope: &str) -> bool {
    path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_route_nobody_classified_is_closed_to_everyone_but_admins() {
        for role in [UserRole::Operator, UserRole::Viewer, UserRole::Remote] {
            for method in ["GET", "POST"] {
                assert!(!check_permission(&role, method, "/api/brand-new-endpoint"), "{role:?} {method}");
                assert!(!check_permission(&role, method, "/api/server/status/extra"), "{role:?} {method}");
            }
        }
        assert!(check_permission(&UserRole::Admin, "POST", "/api/brand-new-endpoint"));
    }

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
