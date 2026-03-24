use crate::models::UserRole;

/// Check if a role is allowed to access a given method+path
pub fn check_permission(role: &UserRole, method: &str, path: &str) -> bool {
    match role {
        UserRole::Admin => true,
        UserRole::Operator => is_operator_allowed(method, path),
        UserRole::Viewer => is_viewer_allowed(method, path),
    }
}

fn is_viewer_allowed(method: &str, path: &str) -> bool {
    // Viewers can only GET (read-only) + change own password + logout
    if method == "GET" { return true; }
    if path == "/api/auth/change-password" || path == "/api/auth/logout" { return true; }
    false
}

fn is_operator_allowed(method: &str, path: &str) -> bool {
    // Operators can GET everything
    if method == "GET" { return true; }
    // Always allowed POST endpoints
    if path == "/api/auth/change-password" || path == "/api/auth/logout" { return true; }

    // Blocked: user management, system accounts, host firewall rules deletion
    let blocked = [
        "/api/users",
        "/api/accounts",         // host account creation/deletion
    ];
    for b in &blocked {
        if path.starts_with(b) && method != "GET" { return false; }
    }

    // Allow most operational POSTs
    true
}

pub fn role_from_str(s: &str) -> UserRole {
    match s {
        "admin" => UserRole::Admin,
        "operator" => UserRole::Operator,
        _ => UserRole::Viewer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_can_do_everything() {
        assert!(check_permission(&UserRole::Admin, "GET", "/api/server/status"));
        assert!(check_permission(&UserRole::Admin, "POST", "/api/users"));
        assert!(check_permission(&UserRole::Admin, "DELETE", "/api/users/123"));
    }

    #[test]
    fn viewer_read_only() {
        assert!(check_permission(&UserRole::Viewer, "GET", "/api/server/status"));
        assert!(!check_permission(&UserRole::Viewer, "POST", "/api/packages/install"));
        assert!(!check_permission(&UserRole::Viewer, "DELETE", "/api/users/123"));
        // Viewer can change own password and logout
        assert!(check_permission(&UserRole::Viewer, "POST", "/api/auth/change-password"));
        assert!(check_permission(&UserRole::Viewer, "POST", "/api/auth/logout"));
    }

    #[test]
    fn operator_limited_write() {
        assert!(check_permission(&UserRole::Operator, "GET", "/api/server/status"));
        assert!(check_permission(&UserRole::Operator, "POST", "/api/services/action"));
        assert!(check_permission(&UserRole::Operator, "POST", "/api/containers/restart"));
        // Operator blocked from user/account management
        assert!(!check_permission(&UserRole::Operator, "POST", "/api/users"));
        assert!(!check_permission(&UserRole::Operator, "POST", "/api/accounts"));
    }

    #[test]
    fn role_from_str_works() {
        assert_eq!(role_from_str("admin"), UserRole::Admin);
        assert_eq!(role_from_str("operator"), UserRole::Operator);
        assert_eq!(role_from_str("viewer"), UserRole::Viewer);
        assert_eq!(role_from_str("unknown"), UserRole::Viewer);
    }
}
