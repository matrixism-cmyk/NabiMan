//! Account-wide enrollment gate: the strongest accessible scope policy wins.
use crate::models::NabimanUser;

pub fn required(user: &NabimanUser) -> Result<bool, String> {
    // Explicit recovery authority; scope administrators cannot change this role.
    if user.super_admin() {
        return Ok(false);
    }
    for membership in user.memberships.iter().filter(|m| m.accessible()) {
        if crate::scope_settings::read(&membership.kind, &membership.scope_id)?.require_mfa {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn recovery_route(method: &str, path: &str) -> bool {
    (method == "GET"
        && matches!(
            path,
            "/api/users/me" | "/api/auth/2fa/status" | "/api/auth/sessions"
        ))
        || (method == "POST"
            && matches!(
                path,
                "/api/auth/logout"
                    | "/api/auth/change-password"
                    | "/api/auth/2fa/setup"
                    | "/api/auth/2fa/verify-setup"
                    | "/api/auth/2fa/recovery-codes"
                    | "/api/auth/sessions/revoke"
                    | "/api/auth/sessions/revoke-all"
                    | "/api/auth/sessions/revoke-others"
            ))
}

pub fn permitted(user: &NabimanUser, verified: bool, method: &str, path: &str) -> bool {
    recovery_route(method, path)
        || match required(user) {
            Ok(false) => true,
            Ok(true) => user.totp_enabled && verified,
            Err(_) => false,
        }
}

#[cfg(test)]
#[path = "mfa_policy_tests.rs"]
mod tests;
