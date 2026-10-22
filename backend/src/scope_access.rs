//! Closed by default: membership accounts cannot access global host administration.
use crate::models::{NabimanUser, ScopeKind, UserRole};

pub fn permission(user: &NabimanUser, method: &str, path: &str) -> bool {
    if !user.active() {
        return false;
    }
    if !crate::mfa_policy::recovery_route(method, path)
        && crate::mfa_policy::required(user)
            .map(|required| required && !user.totp_enabled)
            .unwrap_or(true)
    {
        return false;
    }
    if (path == "/api/auth/2fa/status" && method == "GET")
        || (path == "/api/auth/2fa/recovery-codes" && method == "POST")
    {
        return true;
    }
    if (path == "/api/auth/sessions" && method == "GET")
        || (matches!(
            path,
            "/api/auth/sessions/revoke"
                | "/api/auth/sessions/revoke-all"
                | "/api/auth/sessions/revoke-others"
        ) && method == "POST")
    {
        return true;
    }
    if user.super_admin() {
        return true;
    }
    if crate::member_servers::is_member(user) {
        return crate::member_servers::permitted(user, method, path);
    }
    if let Some(rest) = path.strip_prefix("/api/scope-settings/") {
        let parts: Vec<_> = rest.split('/').collect();
        if parts.len() != 2 && !(parts.len() == 3 && parts[2] == "history" && method == "GET") {
            return false;
        }
        let kind = match parts[0] {
            "server" => ScopeKind::Server,
            "tenant" => ScopeKind::Tenant,
            _ => return false,
        };
        if parts.len() == 3 {
            return user.manages(&kind, parts[1]);
        }
        return (method == "GET" && user.scope_role(&kind, parts[1]).is_some())
            || (method == "PUT" && user.manages(&kind, parts[1]));
    }
    if !user.restricted() {
        return crate::rbac::check_permission(&user.role, method, path);
    }
    if matches!(path, "/api/users/me" | "/api/users") && method == "GET" {
        return true;
    }
    if path == "/api/users/memberships" && method == "POST" {
        return user
            .memberships
            .iter()
            .any(|m| m.role == UserRole::Admin && m.accessible());
    }
    if matches!(
        path,
        "/api/auth/logout"
            | "/api/auth/change-password"
            | "/api/auth/2fa/setup"
            | "/api/auth/2fa/verify-setup"
            | "/api/auth/2fa/disable"
    ) && method == "POST"
    {
        return true;
    }
    if matches!(path, "/api/remote-servers" | "/api/mec/v1/tenants") && method == "GET" {
        return true;
    }
    if let Some(rest) = path.strip_prefix("/api/remote-servers/") {
        let mut parts = rest.split('/');
        let id = parts.next().unwrap_or("");
        let action = parts.next().unwrap_or("");
        if parts.next().is_some() {
            return false;
        }
        let Some(role) = user.scope_role(&ScopeKind::Server, id) else {
            return false;
        };
        if action == "host-trust" {
            return method == "PUT" && user.manages(&ScopeKind::Server, id);
        }
        return method == "POST"
            && (action == "check"
                || action == "exec"
                    && *role != UserRole::Viewer
                    && user.memberships.iter().any(|m| {
                        m.kind == ScopeKind::Server
                            && m.scope_id == id
                            && m.allow_exec != Some(false)
                    })
                    && crate::scope_settings::read(&ScopeKind::Server, id)
                        .map(|s| s.allow_commands)
                        .unwrap_or(false));
    }
    if let Some(rest) = path.strip_prefix("/api/mec/v1/tenants/") {
        let parts: Vec<_> = rest.split('/').collect();
        if parts.len() == 4 && parts[1] == "members" && parts[3] == "access-plan" {
            return method == "GET" && user.manages(&ScopeKind::Tenant, parts[0]);
        }
        if parts.len() == 4 && parts[1] == "managed-memberships" && parts[3] == "revoke" {
            return method == "POST" && user.manages(&ScopeKind::Tenant, parts[0]);
        }
        let mut parts = rest.split('/');
        let id = parts.next().unwrap_or("");
        let action = parts.next().unwrap_or("");
        if parts.next().is_some() || user.scope_role(&ScopeKind::Tenant, id).is_none() {
            return false;
        }
        if user.manages(&ScopeKind::Tenant, id) {
            if method == "GET"
                && matches!(
                    action,
                    "delegated-access-context" | "scoped-managed-memberships"
                )
            {
                return true;
            }
            if method == "POST" && action == "managed-memberships" {
                return true;
            }
        }
        // Tenant lifecycle and infrastructure credentials stay with the super administrator.
        return method == "GET" && matches!(action, "" | "resources" | "quota-usage");
    }
    false
}
