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
