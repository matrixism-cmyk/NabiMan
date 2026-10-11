use super::*;

pub(super) fn parse_jail_list(output: &str) -> Vec<String> {
    for line in output.lines() {
        if let Some(list) = line.trim().strip_prefix("`- Jail list:") {
            return list
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    Vec::new()
}

pub(super) fn f2b_get(jail: &str, key: &str) -> String {
    run("fail2ban-client", &["get", jail, key])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub(super) fn parse_jail_status(name: &str, output: &str) -> JailInfo {
    let mut count = 0u32;
    let mut filter = String::new();
    for line in output.lines() {
        let t = line.trim();
        if t.contains("Currently banned:") {
            count = t
                .split(':')
                .last()
                .unwrap_or("0")
                .trim()
                .parse()
                .unwrap_or(0);
        } else if t.contains("File list:") {
            filter = t.split(':').last().unwrap_or("").trim().to_string();
        }
    }
    JailInfo {
        name: name.to_string(),
        enabled: true,
        banned_count: count,
        filter,
        max_retry: f2b_get(name, "maxretry").parse().unwrap_or(0),
        ban_time: f2b_get(name, "bantime"),
        find_time: f2b_get(name, "findtime"),
    }
}

pub(super) fn parse_banned_f2b(jail: &str, output: &str) -> Vec<BannedIp> {
    output
        .lines()
        .filter(|l| l.contains("Banned IP list:"))
        .flat_map(|l| {
            l.splitn(2, "Banned IP list:")
                .nth(1)
                .unwrap_or("")
                .split_whitespace()
                .filter(|s| !s.is_empty())
                .map(|ip| BannedIp {
                    ip: ip.to_string(),
                    jail: jail.to_string(),
                    ban_time: String::new(),
                    expires: String::new(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub(super) fn parse_iptables_banned(output: &str) -> Vec<BannedIp> {
    output
        .lines()
        .filter_map(|line| {
            let p: Vec<&str> = line.trim().split_whitespace().collect();
            if p.len() >= 5 && (p[0] == "DROP" || p[0] == "REJECT") && p[3] != "0.0.0.0/0" {
                Some(BannedIp {
                    ip: p[3].to_string(),
                    jail: "iptables".to_string(),
                    ban_time: String::new(),
                    expires: String::new(),
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn f2b_jails() -> Result<Vec<String>, String> {
    run("fail2ban-client", &["status"]).map(|o| parse_jail_list(&o))
}

pub(super) fn mk_jail(name: &str, count: u32) -> JailInfo {
    JailInfo {
        name: name.into(),
        enabled: true,
        banned_count: count,
        filter: String::new(),
        max_retry: 0,
        ban_time: String::new(),
        find_time: String::new(),
    }
}
