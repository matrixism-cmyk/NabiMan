//! Plans and their limits, carried as data in the signed licence (`info.limits`).
//!
//! Only a licence that carries `limits` changes anything: without one (no
//! licence, a trial, an older licence, one that does not verify) nothing here
//! refuses or warns, exactly as before plans existed. With one, the limits are
//! soft: adding a user or a server beyond them is refused, nothing that exists
//! is locked, hidden or removed. When such a licence expires it keeps working
//! for `grace_days` (default 14); after that growth stops (no new users or
//! servers) and everything else carries on.

use super::LicenseStatus;
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Grace after expiry when the licence does not name one.
pub(crate) const DEFAULT_GRACE_DAYS: u32 = 14;
const MAX_GRACE_DAYS: u32 = 90;
// Plan features this version knows: `orchestration`, `graph_3d`, `recording`,
// `mec`. A licence may name others (a newer issuer's): they grant nothing here
// but do not make the licence invalid.

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct PlanLimits {
    /// The plan's name, e.g. `team`, `business`, `hosted-remote`.
    pub plan: String,
    /// `installed` (on the customer's machine) or `hosted` (an instance we run).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<String>,
    /// Active accounts allowed; none means no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_users: Option<u32>,
    /// Saved remote servers allowed; none means no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_servers: Option<u32>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace_days: Option<u32>,
}

impl PlanLimits {
    pub(crate) fn grace(&self) -> u32 {
        self.grace_days.unwrap_or(DEFAULT_GRACE_DAYS)
    }
}

fn name_ok(value: &str, extra: &[u8]) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || extra.contains(&b))
}

/// Checked with the rest of the signed claims; `max_servers` is the info's.
pub(super) fn validate(limits: &PlanLimits, info_max_servers: Option<u32>) -> Result<(), String> {
    if !name_ok(&limits.plan, b"-_") {
        return Err("Invalid plan name".into());
    }
    if !matches!(limits.deployment.as_deref(), None | Some("installed" | "hosted")) {
        return Err("Invalid plan deployment".into());
    }
    if limits.max_users == Some(0) {
        return Err("A plan must allow at least one user".into());
    }
    if limits.max_servers != info_max_servers {
        return Err("Plan server limit must match the licence server limit".into());
    }
    if limits.grace_days.is_some_and(|d| d > MAX_GRACE_DAYS) {
        return Err("Grace period is too long".into());
    }
    let mut seen = HashSet::new();
    if limits.features.len() > 32 || limits.features.iter().any(|f| !name_ok(f, b"_") || !seen.insert(f)) {
        return Err("Invalid plan features".into());
    }
    Ok(())
}

/// Where an expired licence with a plan stands on `today`.
pub(super) enum AfterExpiry {
    Grace { ends: NaiveDate, days_left: i64 },
    Ended { ended: NaiveDate },
}

pub(super) fn after_expiry(limits: &PlanLimits, expired: NaiveDate, today: NaiveDate) -> AfterExpiry {
    // `expires_at` is the last valid day; grace runs for the days after it.
    let ends = expired + Duration::days(i64::from(limits.grace()));
    if today <= ends {
        AfterExpiry::Grace { ends, days_left: (ends - today).num_days() }
    } else {
        AfterExpiry::Ended { ended: ends }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Resource {
    Users,
    Servers,
}

/// Whether one more `resource` may be added when `current` exist.
pub(crate) fn admit_with(status: &LicenseStatus, resource: Resource, current: usize) -> Result<(), String> {
    let Some(limits) = status.limits.as_ref() else { return Ok(()) };
    let what = match resource {
        Resource::Users => "사용자",
        Resource::Servers => "서버",
    };
    if status.validation_state == "license_expired" {
        return Err(format!(
            "라이선스가 만료되고 유예 기간도 끝나 새 {what}를 추가할 수 없습니다. 기존 {what}는 그대로 사용할 수 있습니다. 라이선스를 갱신하세요."
        ));
    }
    let max = match resource {
        Resource::Users => limits.max_users,
        Resource::Servers => limits.max_servers,
    };
    match max {
        Some(max) if current >= max as usize => Err(format!(
            "라이선스의 {what} 한도({max})에 도달해 더 추가할 수 없습니다. 기존 {what}는 그대로 사용할 수 있습니다. 한도를 늘리려면 라이선스를 갱신하세요."
        )),
        _ => Ok(()),
    }
}

/// Refuses adding a user or a server beyond the installed plan's limit.
pub(crate) fn admit(resource: Resource, current: usize) -> Result<(), String> {
    admit_with(&super::current(), resource, current)
}

/// Whether a plan feature may be switched on. True unless a plan is installed
/// that leaves it out; what is already on stays on either way.
pub(crate) fn entitled_with(status: &LicenseStatus, feature: &str) -> bool {
    status.limits.as_ref().map_or(true, |l| l.features.iter().any(|f| f == feature))
}

pub(crate) fn entitled(feature: &str) -> bool {
    entitled_with(&super::current(), feature)
}

/// Active accounts: what a user limit counts.
pub(crate) fn seats(users: &[crate::models::NabimanUser]) -> usize {
    users.iter().filter(|u| u.status == crate::models::AccountStatus::Active).count()
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub(crate) struct Counts {
    pub users: usize,
    pub servers: usize,
}

/// What the banner and the licence screen warn about, as codes the UI
/// translates. Empty without a plan.
pub(crate) fn notices(status: &LicenseStatus, counts: &Counts) -> Vec<&'static str> {
    let Some(limits) = status.limits.as_ref() else { return vec![] };
    let mut out = vec![];
    match status.validation_state.as_str() {
        "grace" => out.push("grace"),
        "license_expired" => out.push("grace_ended"),
        _ if status.days_remaining.is_some_and(|d| d <= 30) => out.push("expiring_soon"),
        _ => {}
    }
    for (max, count, at, over) in [
        (limits.max_users, counts.users, "users_at_limit", "users_over_limit"),
        (limits.max_servers, counts.servers, "servers_at_limit", "servers_over_limit"),
    ] {
        match max.map(|m| m as usize) {
            Some(m) if count > m => out.push(over),
            Some(m) if count == m => out.push(at),
            _ => {}
        }
    }
    out
}
