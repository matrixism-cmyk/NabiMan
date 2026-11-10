//! Who may call which API route, as an allowlist: every route the App
//! registers is listed here with its class, and a path that matches no
//! listed route is refused to everyone but the super administrators. A new
//! endpoint is therefore closed to operators, viewers and members until it
//! is classified — and the route-access test fails until it is, and until
//! `docs/security/route-access.md` (generated from these tables) is updated.
//!
//! The classes reproduce the rules that were in `rbac` (operators: every
//! read and most writes; viewers: reads) and in `member_servers` (members:
//! their terminal, files and servers); the tests check, route by route and
//! method by method, that nothing changed.
//!
//! Super administrators are not judged here, and scope-restricted accounts
//! follow `scope_access`'s own allowlist.

mod core;
mod mec;
#[cfg(test)]
mod doc;
#[cfg(test)]
mod inventory;
#[cfg(test)]
mod legacy;
#[cfg(test)]
mod tests;

use std::sync::OnceLock;

use crate::models::UserRole;

/// What an operator or a viewer (one not limited to a scope) may do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    /// Reached without signing in; the handler checks what it needs (a
    /// share link's secret, a sign-in's password).
    Public,
    /// Super administrators only.
    Admin,
    /// Operators and viewers may read it (GET); changes are for administrators.
    Read,
    /// Operators may use it with any method; viewers may read it.
    Operate,
    /// Anyone signed in: their own sign-in, password and second factor.
    Account,
}

/// What a remote-only member may reach. `member_servers` then still applies
/// its own rules (switches an administrator set, whose server it is).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Member {
    No,
    /// These methods.
    Only(&'static [&'static str]),
    /// Any method; the handler allows only what is the member's own.
    Handler,
    /// Any method; `member_servers` decides from the server in the path.
    Servers,
}

#[derive(Debug)]
pub(crate) struct Route {
    pub pattern: &'static str,
    pub class: Class,
    pub member: Member,
}

pub(super) const fn r(pattern: &'static str, class: Class, member: Member) -> Route {
    Route { pattern, class, member }
}

pub(crate) fn all() -> impl Iterator<Item = &'static Route> {
    core::ROUTES.iter().chain(mec::ROUTES.iter())
}

fn split(pattern: &'static str) -> Vec<&'static str> {
    pattern.split('/').collect()
}

fn compiled() -> &'static [(Vec<&'static str>, &'static Route)] {
    static TABLE: OnceLock<Vec<(Vec<&'static str>, &'static Route)>> = OnceLock::new();
    TABLE.get_or_init(|| all().map(|route| (split(route.pattern), route)).collect())
}

fn variable(segment: &str) -> bool {
    segment.starts_with('{') && segment.ends_with('}')
}

/// The listed route `path` is served by, as the router would pick it: a
/// literal segment wins over a `{variable}` one.
pub(crate) fn lookup(path: &str) -> Option<&'static Route> {
    let segments: Vec<&str> = path.split('/').collect();
    compiled()
        .iter()
        .filter(|(pattern, _)| {
            pattern.len() == segments.len()
                && pattern.iter().zip(&segments).all(|(p, s)| if variable(p) { !s.is_empty() } else { p == s })
        })
        .max_by_key(|(pattern, _)| pattern.iter().map(|p| !variable(p)).collect::<Vec<_>>())
        .map(|(_, route)| *route)
}

impl Class {
    /// Whether an operator or viewer may make this call.
    pub(crate) fn allows(self, role: &UserRole, method: &str) -> bool {
        match (self, role) {
            (Class::Account, UserRole::Operator | UserRole::Viewer) => true,
            (Class::Operate, UserRole::Operator) => true,
            (Class::Operate | Class::Read, UserRole::Operator | UserRole::Viewer) => method == "GET",
            _ => false,
        }
    }
}

/// Whether an operator or a viewer may call `method path` (see `Class`).
pub(crate) fn staff_allowed(role: &UserRole, method: &str, path: &str) -> bool {
    lookup(path).is_some_and(|route| route.class.allows(role, method))
}

/// Whether a member's call may go on to `member_servers`' rules at all.
pub(crate) fn member_reachable(method: &str, path: &str) -> bool {
    match lookup(path).map(|route| route.member) {
        Some(Member::Only(methods)) => methods.contains(&method),
        Some(Member::Handler | Member::Servers) => true,
        Some(Member::No) | None => false,
    }
}
