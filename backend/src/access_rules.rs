//! When and from where an account may be used (`AccessRules`): checked when
//! it signs in, on every request, and while a terminal stays open. The super
//! administrators are never held to them, so nobody can lock the service out.

use std::net::IpAddr;

use actix_web::HttpRequest;
use chrono::{Datelike, Timelike};

use crate::models::{AccessRules, HourWindow, NabimanUser};

/// At most this many addresses or networks per account.
pub const MAX_NETWORKS: usize = 32;

/// The address the person is at. NabiMan normally runs behind a proxy on the
/// same machine, which adds the address it was reached from to
/// `X-Forwarded-For`; only then is that header believed, and its right-most
/// public address is taken (the left can be anything the browser sent). On
/// a private network with no public address in it, its left-most is.
pub fn client_ip(req: &HttpRequest) -> Option<IpAddr> {
    let peer = req.peer_addr()?.ip();
    if !peer.is_loopback() {
        return Some(peer);
    }
    let chain: Vec<IpAddr> = req
        .headers()
        .get_all("x-forwarded-for")
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|a| a.trim().parse().ok())
        .collect();
    chain
        .iter()
        .rev()
        .find(|ip| public(**ip))
        .or(chain.first())
        .copied()
        .or(Some(peer))
}

fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !(v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
            || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64)),
        IpAddr::V6(v6) => !(v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00
            || (v6.segments()[0] & 0xffc0) == 0xfe80),
    }
}

/// `203.0.113.7` or `203.0.113.0/24` (IPv6 too), as an address and prefix.
pub fn parse_network(text: &str) -> Option<(IpAddr, u8)> {
    let (addr, bits) = match text.trim().split_once('/') {
        Some((a, b)) => (a.parse::<IpAddr>().ok()?, b.parse::<u8>().ok()?),
        None => {
            let a = text.trim().parse::<IpAddr>().ok()?;
            (a, if a.is_ipv4() { 32 } else { 128 })
        }
    };
    let max = if addr.is_ipv4() { 32 } else { 128 };
    (bits <= max).then_some((addr, bits))
}

fn in_network(ip: IpAddr, (net, bits): (IpAddr, u8)) -> bool {
    let ip = match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    };
    match (ip, net) {
        (IpAddr::V4(a), IpAddr::V4(n)) => {
            let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits) };
            u32::from(a) & mask == u32::from(n) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(n)) => {
            let mask = if bits == 0 { 0 } else { u128::MAX << (128 - bits) };
            u128::from(a) & mask == u128::from(n) & mask
        }
        _ => false,
    }
}

/// Korean time, which the hours are written in.
fn korean_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(9 * 3600).expect("valid offset"))
}

fn within(window: &HourWindow, weekday: u8, hour: u8) -> bool {
    let day_ok = window.days.is_empty() || window.days.contains(&weekday);
    let hour_ok = if window.start < window.end {
        (window.start..window.end).contains(&hour)
    } else {
        hour >= window.start || hour < window.end
    };
    day_ok && hour_ok
}

impl AccessRules {
    /// Whether the account's time is over.
    pub fn expired(&self) -> bool {
        self.expires_at.as_deref().is_some_and(|at| {
            chrono::DateTime::parse_from_rfc3339(at).map_or(true, |at| chrono::Utc::now() >= at)
        })
    }
}

/// Whether `user` may be used now from `ip`; the reason, in their words, when not.
pub fn admits(user: &NabimanUser, ip: Option<IpAddr>) -> Result<(), String> {
    let now = korean_now();
    admits_at(user, ip, now.weekday().num_days_from_monday() as u8, now.hour() as u8)
}

fn admits_at(user: &NabimanUser, ip: Option<IpAddr>, weekday: u8, hour: u8) -> Result<(), String> {
    if user.role == crate::models::UserRole::Admin {
        return Ok(());
    }
    let rules = &user.access;
    if !rules.allowed_networks.is_empty() {
        let allowed = ip.is_some_and(|ip| {
            rules.allowed_networks.iter().filter_map(|n| parse_network(n)).any(|n| in_network(ip, n))
        });
        if !allowed {
            let at = ip.map(|ip| ip.to_string()).unwrap_or_else(|| "알 수 없음".into());
            return Err(format!("이 계정은 지금 위치({at})에서 쓸 수 없습니다. 관리자에게 문의하세요."));
        }
    }
    if let Some(window) = &rules.allowed_hours {
        if !within(window, weekday, hour) {
            return Err(format!(
                "이 계정은 {:02}:00~{:02}:00(한국 시간){}에만 쓸 수 있습니다.",
                window.start,
                window.end,
                if window.days.is_empty() { String::new() } else { format!(" {}", day_names(&window.days)) }
            ));
        }
    }
    Ok(())
}

fn day_names(days: &[u8]) -> String {
    const NAMES: [&str; 7] = ["월", "화", "수", "목", "금", "토", "일"];
    let mut sorted = days.to_vec();
    sorted.sort_unstable();
    sorted.iter().filter_map(|d| NAMES.get(*d as usize)).copied().collect::<Vec<_>>().join("·")
}

/// Whether the rules make sense before they are saved.
pub fn validate(rules: &AccessRules) -> Result<(), String> {
    if let Some(at) = &rules.expires_at {
        chrono::DateTime::parse_from_rfc3339(at).map_err(|_| "이용 만료일 형식이 올바르지 않습니다".to_string())?;
    }
    if rules.allowed_networks.len() > MAX_NETWORKS {
        return Err(format!("허용 주소는 {MAX_NETWORKS}개까지 넣을 수 있습니다"));
    }
    if let Some(bad) = rules.allowed_networks.iter().find(|n| parse_network(n).is_none()) {
        return Err(format!("허용 주소 '{bad}' 를 읽을 수 없습니다 (예: 203.0.113.7, 203.0.113.0/24)"));
    }
    if let Some(w) = &rules.allowed_hours {
        if w.start > 23 || w.end > 24 || w.start == w.end || w.days.iter().any(|d| *d > 6) {
            return Err("허용 시간대가 올바르지 않습니다".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(rules: AccessRules) -> NabimanUser {
        let mut u: NabimanUser = serde_json::from_value(serde_json::json!({
            "id": "m", "username": "m", "password_hash": "", "role": "remote", "created_at": "now", "status": "active"
        }))
        .unwrap();
        u.access = rules;
        u
    }

    #[test]
    fn networks_and_hours_admit_only_what_they_say() {
        let net = member(AccessRules { allowed_networks: vec!["203.0.113.0/24".into(), "2001:db8::1".into()], ..Default::default() });
        assert!(admits_at(&net, "203.0.113.9".parse().ok(), 0, 10).is_ok());
        assert!(admits_at(&net, "::ffff:203.0.113.9".parse().ok(), 0, 10).is_ok());
        assert!(admits_at(&net, "2001:db8::1".parse().ok(), 0, 10).is_ok());
        assert!(admits_at(&net, "198.51.100.1".parse().ok(), 0, 10).is_err());
        assert!(admits_at(&net, None, 0, 10).is_err());
        let office = member(AccessRules {
            allowed_hours: Some(HourWindow { start: 9, end: 18, days: vec![0, 1, 2, 3, 4] }),
            ..Default::default()
        });
        assert!(admits_at(&office, None, 2, 9).is_ok());
        assert!(admits_at(&office, None, 2, 18).is_err());
        assert!(admits_at(&office, None, 5, 10).is_err());
        let night = member(AccessRules { allowed_hours: Some(HourWindow { start: 22, end: 6, days: vec![] }), ..Default::default() });
        assert!(admits_at(&night, None, 6, 23).is_ok() && admits_at(&night, None, 6, 3).is_ok());
        assert!(admits_at(&night, None, 6, 12).is_err());
        let mut admin = net.clone();
        admin.role = crate::models::UserRole::Admin;
        assert!(admits_at(&admin, None, 5, 3).is_ok());
    }

    #[test]
    fn an_expired_account_is_no_longer_active() {
        let past = member(AccessRules { expires_at: Some("2000-01-01T00:00:00Z".into()), ..Default::default() });
        assert!(past.access.expired() && !past.active());
        let future = member(AccessRules { expires_at: Some("2999-01-01T00:00:00+09:00".into()), ..Default::default() });
        assert!(!future.access.expired() && future.active());
    }

    #[test]
    fn rules_are_checked_before_they_are_saved() {
        assert!(validate(&AccessRules { allowed_networks: vec!["10.0.0.0/33".into()], ..Default::default() }).is_err());
        assert!(validate(&AccessRules { allowed_networks: vec!["example.com".into()], ..Default::default() }).is_err());
        assert!(validate(&AccessRules { expires_at: Some("tomorrow".into()), ..Default::default() }).is_err());
        assert!(validate(&AccessRules { allowed_hours: Some(HourWindow { start: 9, end: 9, days: vec![] }), ..Default::default() }).is_err());
        assert!(validate(&AccessRules {
            expires_at: Some("2030-01-01T00:00:00Z".into()),
            allowed_networks: vec!["192.168.0.0/16".into()],
            allowed_hours: Some(HourWindow { start: 9, end: 18, days: vec![0, 4] }),
            require_mfa: true,
        })
        .is_ok());
    }
}
