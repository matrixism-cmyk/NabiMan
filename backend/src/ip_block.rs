use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Serialize, Clone)]
pub struct JailInfo {
    pub name: String, pub enabled: bool, pub banned_count: u32,
    pub filter: String, pub max_retry: u32, pub ban_time: String, pub find_time: String,
}

#[derive(Serialize, Clone)]
pub struct IpBlockStatus { pub backend: String, pub active: bool, pub jails: Vec<JailInfo> }

#[derive(Serialize, Clone)]
pub struct BannedIp { pub ip: String, pub jail: String, pub ban_time: String, pub expires: String }

#[derive(Serialize, Clone)]
pub struct BlockLogEntry { pub timestamp: String, pub action: String, pub ip: String, pub jail: String }

#[derive(Deserialize)]
pub struct UnbanRequest { pub ip: String, pub jail: Option<String> }

#[derive(Deserialize)]
pub struct BanRequest { pub ip: String, pub jail: Option<String>, pub duration: Option<u64> }

enum Backend { Fail2ban, Iptables, Nftables, None }

fn cmd_exists(cmd: &str) -> bool {
    std::process::Command::new("which").arg(cmd)
        .output().map(|o| o.status.success()).unwrap_or(false)
}

fn detect() -> Backend {
    if cmd_exists("fail2ban-client") { Backend::Fail2ban }
    else if cmd_exists("iptables") { Backend::Iptables }
    else if cmd_exists("nft") { Backend::Nftables }
    else { Backend::None }
}

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let o = std::process::Command::new(cmd).args(args).output()
        .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;
    if o.status.success() { Ok(String::from_utf8_lossy(&o.stdout).to_string()) }
    else {
        let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
        Err(if e.is_empty() { String::from_utf8_lossy(&o.stdout).trim().to_string() } else { e })
    }
}

fn validate_ip(ip: &str) -> bool {
    let t = ip.trim();
    if t.is_empty() || t.len() > 45 { return false; }
    if t.chars().any(|c| !c.is_ascii_hexdigit() && c != '.' && c != ':') { return false; }
    t.parse::<Ipv4Addr>().is_ok() || t.parse::<Ipv6Addr>().is_ok()
}

fn validate_jail(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64
        && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn parse_jail_list(output: &str) -> Vec<String> {
    for line in output.lines() {
        if let Some(list) = line.trim().strip_prefix("`- Jail list:") {
            return list.split(',').map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()).collect();
        }
    }
    Vec::new()
}

fn f2b_get(jail: &str, key: &str) -> String {
    run("fail2ban-client", &["get", jail, key]).map(|s| s.trim().to_string()).unwrap_or_default()
}

fn parse_jail_status(name: &str, output: &str) -> JailInfo {
    let mut count = 0u32;
    let mut filter = String::new();
    for line in output.lines() {
        let t = line.trim();
        if t.contains("Currently banned:") {
            count = t.split(':').last().unwrap_or("0").trim().parse().unwrap_or(0);
        } else if t.contains("File list:") {
            filter = t.split(':').last().unwrap_or("").trim().to_string();
        }
    }
    JailInfo {
        name: name.to_string(), enabled: true, banned_count: count, filter,
        max_retry: f2b_get(name, "maxretry").parse().unwrap_or(0),
        ban_time: f2b_get(name, "bantime"), find_time: f2b_get(name, "findtime"),
    }
}

fn parse_banned_f2b(jail: &str, output: &str) -> Vec<BannedIp> {
    output.lines().filter(|l| l.contains("Banned IP list:"))
        .flat_map(|l| l.splitn(2, "Banned IP list:").nth(1).unwrap_or("").split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|ip| BannedIp { ip: ip.to_string(), jail: jail.to_string(),
                ban_time: String::new(), expires: String::new() })
            .collect::<Vec<_>>())
        .collect()
}

fn parse_iptables_banned(output: &str) -> Vec<BannedIp> {
    output.lines().filter_map(|line| {
        let p: Vec<&str> = line.trim().split_whitespace().collect();
        if p.len() >= 5 && (p[0] == "DROP" || p[0] == "REJECT") && p[3] != "0.0.0.0/0" {
            Some(BannedIp { ip: p[3].to_string(), jail: "iptables".to_string(),
                ban_time: String::new(), expires: String::new() })
        } else { None }
    }).collect()
}

fn f2b_jails() -> Result<Vec<String>, String> {
    run("fail2ban-client", &["status"]).map(|o| parse_jail_list(&o))
}

fn mk_jail(name: &str, count: u32) -> JailInfo {
    JailInfo { name: name.into(), enabled: true, banned_count: count,
        filter: String::new(), max_retry: 0, ban_time: String::new(), find_time: String::new() }
}

async fn get_status() -> HttpResponse {
    match detect() {
        Backend::Fail2ban => {
            let jails = match f2b_jails() {
                Ok(names) => names.iter().filter_map(|n| run("fail2ban-client", &["status", n])
                    .ok().map(|o| parse_jail_status(n, &o))).collect(),
                Err(e) => return HttpResponse::Ok().json(ApiResponse::<IpBlockStatus>::error(&e)),
            };
            HttpResponse::Ok().json(ApiResponse::ok(IpBlockStatus {
                backend: "fail2ban".into(), active: true, jails }))
        }
        Backend::Iptables => {
            let n = parse_iptables_banned(&run("iptables", &["-L", "INPUT", "-n"]).unwrap_or_default()).len();
            HttpResponse::Ok().json(ApiResponse::ok(IpBlockStatus {
                backend: "iptables".into(), active: true, jails: vec![mk_jail("iptables", n as u32)] }))
        }
        Backend::Nftables => HttpResponse::Ok().json(ApiResponse::ok(IpBlockStatus {
            backend: "nftables".into(), active: true, jails: vec![mk_jail("nftables", 0)] })),
        Backend::None => HttpResponse::Ok().json(ApiResponse::<IpBlockStatus>::error("No IP blocking backend found")),
    }
}

async fn get_banned() -> HttpResponse {
    match detect() {
        Backend::Fail2ban => {
            let names = match f2b_jails() {
                Ok(n) => n,
                Err(e) => return HttpResponse::Ok().json(ApiResponse::<Vec<BannedIp>>::error(&e)),
            };
            let banned: Vec<BannedIp> = names.iter()
                .flat_map(|j| run("fail2ban-client", &["status", j]).ok()
                    .map(|o| parse_banned_f2b(j, &o)).unwrap_or_default())
                .collect();
            HttpResponse::Ok().json(ApiResponse::ok(banned))
        }
        Backend::Iptables => {
            let banned = parse_iptables_banned(&run("iptables", &["-L", "INPUT", "-n"]).unwrap_or_default());
            HttpResponse::Ok().json(ApiResponse::ok(banned))
        }
        Backend::Nftables => {
            let output = run("nft", &["list", "ruleset"]).unwrap_or_default();
            let banned: Vec<BannedIp> = output.lines().filter_map(|l| {
                let t = l.trim();
                if (t.contains("drop") || t.contains("reject")) && t.contains("ip saddr") {
                    let after = &t[t.find("ip saddr")? + 9..];
                    let ip = after.split_whitespace().next()?;
                    Some(BannedIp { ip: ip.into(), jail: "nftables".into(),
                        ban_time: String::new(), expires: String::new() })
                } else { None }
            }).collect();
            HttpResponse::Ok().json(ApiResponse::ok(banned))
        }
        Backend::None => HttpResponse::Ok().json(ApiResponse::<Vec<BannedIp>>::error("No backend found")),
    }
}

async fn unban_ip(body: web::Json<UnbanRequest>) -> HttpResponse {
    let ip = body.ip.trim();
    if !validate_ip(ip) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid IP address"));
    }
    match detect() {
        Backend::Fail2ban => {
            match &body.jail {
                Some(j) if !validate_jail(j) =>
                    return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid jail name")),
                Some(j) => match run("fail2ban-client", &["set", j, "unbanip", ip]) {
                    Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Unbanned {} from {}", ip, j))),
                    Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
                },
                None => {
                    let jails = f2b_jails().unwrap_or_default();
                    for j in &jails { let _ = run("fail2ban-client", &["set", j, "unbanip", ip]); }
                    HttpResponse::Ok().json(ApiResponse::ok(format!("Unbanned {} from all jails", ip)))
                }
            }
        }
        Backend::Iptables => match run("iptables", &["-D", "INPUT", "-s", ip, "-j", "DROP"]) {
            Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Unbanned {} via iptables", ip))),
            Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
        },
        Backend::Nftables => {
            let ruleset = run("nft", &["-a", "list", "ruleset"]).unwrap_or_default();
            let mut ok = false;
            for line in ruleset.lines() {
                let t = line.trim();
                if t.contains(ip) && (t.contains("drop") || t.contains("reject")) {
                    if let Some(h) = t.rsplit("# handle ").next().and_then(|h| h.trim().parse::<u64>().ok()) {
                        let _ = run("nft", &["delete", "rule", "inet", "filter", "input", "handle", &h.to_string()]);
                        ok = true;
                    }
                }
            }
            if ok { HttpResponse::Ok().json(ApiResponse::ok(format!("Unbanned {} via nftables", ip))) }
            else { HttpResponse::Ok().json(ApiResponse::<String>::error("IP not found in nftables")) }
        }
        Backend::None => HttpResponse::Ok().json(ApiResponse::<String>::error("No backend found")),
    }
}

async fn ban_ip(body: web::Json<BanRequest>) -> HttpResponse {
    let ip = body.ip.trim();
    if !validate_ip(ip) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid IP address"));
    }
    match detect() {
        Backend::Fail2ban => {
            let jail = match &body.jail {
                Some(j) if validate_jail(j) => j.clone(),
                Some(_) => return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid jail name")),
                None => "sshd".to_string(),
            };
            if let Some(d) = body.duration {
                let _ = run("fail2ban-client", &["set", &jail, "bantime", &d.to_string()]);
            }
            match run("fail2ban-client", &["set", &jail, "banip", ip]) {
                Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Banned {} in jail {}", ip, jail))),
                Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
            }
        }
        Backend::Iptables => match run("iptables", &["-I", "INPUT", "-s", ip, "-j", "DROP"]) {
            Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Banned {} via iptables", ip))),
            Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
        },
        Backend::Nftables => match run("nft", &["add", "rule", "inet", "filter", "input", "ip", "saddr", ip, "drop"]) {
            Ok(_) => HttpResponse::Ok().json(ApiResponse::ok(format!("Banned {} via nftables", ip))),
            Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e)),
        },
        Backend::None => HttpResponse::Ok().json(ApiResponse::<String>::error("No backend found")),
    }
}

fn extract_jail_from_log(line: &str) -> Option<String> {
    let start = line.find("NOTICE").or_else(|| line.find("WARNING")).or_else(|| line.find("INFO"))?;
    let after = &line[start..];
    let b = after.find('[')? + 1;
    let e = after[b..].find(']')?;
    Some(after[b..b + e].to_string())
}

async fn get_log() -> HttpResponse {
    let content = match std::fs::read_to_string("/var/log/fail2ban.log") {
        Ok(c) => c,
        Err(_) => return HttpResponse::Ok().json(ApiResponse::ok(Vec::<BlockLogEntry>::new())),
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(200);
    let mut entries: Vec<BlockLogEntry> = lines[start..].iter().filter_map(|line| {
        let t = line.trim();
        let (action, kw) = if t.contains("] Ban ") { ("Ban", "] Ban ") }
            else if t.contains("] Unban ") { ("Unban", "] Unban ") }
            else { return None; };
        let timestamp = t.get(..19)?.to_string();
        let jail = extract_jail_from_log(t).unwrap_or_default();
        let ip = t.rsplit(kw).next()?.split_whitespace().next()?.to_string();
        if ip.is_empty() { return None; }
        Some(BlockLogEntry { timestamp, action: action.into(), ip, jail })
    }).collect();
    entries.reverse();
    HttpResponse::Ok().json(ApiResponse::ok(entries))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/ipblock")
            .route("/status", web::get().to(get_status))
            .route("/banned", web::get().to(get_banned))
            .route("/unban", web::post().to(unban_ip))
            .route("/ban", web::post().to(ban_ip))
            .route("/log", web::get().to(get_log)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_ip() {
        assert!(validate_ip("192.168.1.1"));
        assert!(validate_ip("10.0.0.1"));
        assert!(validate_ip("::1"));
        assert!(validate_ip("fe80::1"));
        assert!(validate_ip("2001:db8::1"));
        assert!(!validate_ip(""));
        assert!(!validate_ip("1.2.3.4; rm -rf /"));
        assert!(!validate_ip("$(whoami)"));
        assert!(!validate_ip("1.2.3.4 && echo pwned"));
        assert!(!validate_ip("not-an-ip"));
        assert!(!validate_ip("192.168.1.999"));
    }

    #[test]
    fn test_validate_jail() {
        assert!(validate_jail("sshd"));
        assert!(validate_jail("apache-auth"));
        assert!(!validate_jail(""));
        assert!(!validate_jail("jail; rm -rf /"));
    }

    #[test]
    fn test_parse_jail_list() {
        let o = "Status\n|- Number of jail:\t2\n`- Jail list:\tsshd, apache-auth";
        assert_eq!(parse_jail_list(o), vec!["sshd", "apache-auth"]);
        let o2 = "Status\n|- Number of jail:\t0\n`- Jail list:\t";
        assert!(parse_jail_list(o2).is_empty());
    }

    #[test]
    fn test_parse_banned_f2b() {
        let o = "Status for the jail: sshd\n|- Filter\n`- Actions\n   \
            |- Currently banned:\t2\n   `- Banned IP list:\t1.2.3.4 5.6.7.8";
        let b = parse_banned_f2b("sshd", o);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].ip, "1.2.3.4");
        assert_eq!(b[1].jail, "sshd");
    }

    #[test]
    fn test_parse_iptables_banned() {
        let o = "Chain INPUT (policy ACCEPT)\ntarget     prot opt source               destination\n\
            DROP       all  --  10.0.0.5             0.0.0.0/0\nACCEPT     all  --  0.0.0.0/0            0.0.0.0/0";
        let b = parse_iptables_banned(o);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].ip, "10.0.0.5");
    }

    #[test]
    fn test_extract_jail_from_log() {
        let l = "2024-01-15 10:23:45,123 fail2ban.actions [123]: NOTICE  [sshd] Ban 1.2.3.4";
        assert_eq!(extract_jail_from_log(l), Some("sshd".to_string()));
    }
}
