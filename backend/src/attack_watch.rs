//! Who is trying to get into a server: its SSH log of the last while, read
//! over the saved connection and counted - which addresses failed to log in,
//! as whom, how often and when; who did get in; and whom fail2ban holds off.
//! Read-only: one `sh` script that prints log lines; nothing on the server
//! changes.

use crate::lock_safe::LockExt;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Serialize;

use crate::models::ApiResponse;

const SCRIPT: &str = include_str!("attack_watch.sh");
const MAX_WINDOW_MIN: u32 = 24 * 60;
const FRESH: Duration = Duration::from_secs(20);
const TOP: usize = 30;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct Attacker {
    pub ip: String,
    pub count: u32,
    /// When it last tried (seconds since 1970; 0 when the log gave no time).
    pub last: u64,
    /// The accounts it tried, most tried first.
    pub users: Vec<String>,
    /// A private or loopback address: someone inside, most likely a mistyped password.
    pub internal: bool,
    /// fail2ban holds it off now.
    pub banned: bool,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct Login {
    pub ip: String,
    pub user: String,
    pub at: u64,
    /// `password`, `publickey`…
    pub method: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct Report {
    /// The log could be read (else `source` says why not).
    pub readable: bool,
    /// `journal`, `journal-sudo`, a log file's path, or `none`.
    pub source: String,
    pub window_min: u32,
    pub failed: u32,
    /// Failed tries by kind: `failed_password`, `invalid_user`, `preauth_closed`.
    pub kinds: HashMap<String, u32>,
    pub attackers: Vec<Attacker>,
    /// How many addresses failed in all (the list above is the top of them).
    pub sources: u32,
    /// Failed tries per minute, oldest first, when the log gave times.
    pub per_minute: Vec<u32>,
    pub accepted: Vec<Login>,
    /// Addresses fail2ban holds off, when it runs and may be asked.
    pub banned: Vec<String>,
    pub checked_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn is_ip(s: &str) -> bool {
    s.parse::<std::net::IpAddr>().is_ok()
}

fn internal(ip: &str) -> bool {
    match ip.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v)) => v.is_private() || v.is_loopback() || v.is_link_local() || (v.octets()[0] == 100 && (64..128).contains(&v.octets()[1])),
        Ok(std::net::IpAddr::V6(v)) => v.is_loopback() || (v.segments()[0] & 0xfe00) == 0xfc00,
        Err(_) => false,
    }
}

/// One sshd line: (kind, user, ip, method). Kinds: the three failures, or `accepted`.
fn parse_line(msg: &str) -> Option<(&'static str, String, String, String)> {
    let words: Vec<&str> = msg.split_whitespace().collect();
    let after = |key: &str| words.iter().position(|w| *w == key).and_then(|i| words.get(i + 1)).map(|s| s.to_string());
    let ip = after("from").filter(|s| is_ip(s));
    if let Some(i) = msg.find("Failed password for ") {
        let rest = &msg[i + "Failed password for ".len()..];
        let user = rest.strip_prefix("invalid user ").unwrap_or(rest).split_whitespace().next()?.to_string();
        return Some(("failed_password", user, ip?, String::new()));
    }
    if let Some(i) = msg.find("Invalid user ") {
        let user = msg[i + "Invalid user ".len()..].split_whitespace().next().unwrap_or("").to_string();
        return Some(("invalid_user", user, ip?, String::new()));
    }
    if let Some(i) = msg.find("Accepted ") {
        let rest: Vec<&str> = msg[i + "Accepted ".len()..].split_whitespace().collect();
        // "Accepted password for jcia from 1.2.3.4 port …"
        return Some(("accepted", rest.get(2)?.to_string(), ip?, rest.first()?.to_string()));
    }
    if let Some(i) = msg.find("authenticating user ") {
        // "Connection closed by authenticating user root 1.2.3.4 port 1 [preauth]"
        let rest: Vec<&str> = msg[i + "authenticating user ".len()..].split_whitespace().collect();
        let ip = rest.get(1).filter(|s| is_ip(s))?.to_string();
        return Some(("preauth_closed", rest.first()?.to_string(), ip, String::new()));
    }
    None
}

fn clip(s: &str, n: usize) -> String {
    s.chars().filter(|c| c.is_ascii_graphic()).take(n).collect()
}

/// Counts what the script printed (see attack_watch.sh for its lines).
pub(crate) fn digest(out: &str, window_min: u32) -> Report {
    let mut r = Report { window_min, ..Default::default() };
    let mut now = 0u64;
    struct Tally { count: u32, last: u64, users: HashMap<String, u32> }
    let mut by_ip: HashMap<String, Tally> = HashMap::new();
    let mut times: Vec<u64> = Vec::new();
    for line in out.lines() {
        if let Some(v) = line.strip_prefix("SRC ") {
            r.source = clip(v, 80);
            continue;
        }
        if let Some(v) = line.strip_prefix("CAN ") {
            r.readable = v.trim() == "yes";
            continue;
        }
        if let Some(v) = line.strip_prefix("NOW ") {
            now = v.trim().parse().unwrap_or(0);
            continue;
        }
        if let Some(v) = line.strip_prefix("BAN ") {
            r.banned = v.split_whitespace().filter(|s| is_ip(s)).map(str::to_string).take(500).collect();
            continue;
        }
        // "1699999999.123456 host sshd[1]: message" from the journal; a plain log line otherwise.
        let at = line.split_whitespace().next().and_then(|w| w.split('.').next()).and_then(|w| w.parse::<u64>().ok()).filter(|t| *t > 1_000_000_000).unwrap_or(0);
        let Some((kind, user, ip, method)) = parse_line(line) else { continue };
        if kind == "accepted" {
            if r.accepted.len() < 50 {
                r.accepted.push(Login { ip, user: clip(&user, 32), at, method: clip(&method, 20) });
            }
            continue;
        }
        r.failed += 1;
        *r.kinds.entry(kind.to_string()).or_default() += 1;
        if at > 0 {
            times.push(at);
        }
        let t = by_ip.entry(ip).or_insert(Tally { count: 0, last: 0, users: HashMap::new() });
        t.count += 1;
        t.last = t.last.max(at);
        if t.users.len() < 200 {
            *t.users.entry(clip(&user, 32)).or_default() += 1;
        }
    }
    r.checked_at = now;
    r.sources = by_ip.len() as u32;
    let mut all: Vec<Attacker> = by_ip
        .into_iter()
        .map(|(ip, t)| {
            let mut users: Vec<(String, u32)> = t.users.into_iter().filter(|(u, _)| !u.is_empty()).collect();
            users.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            Attacker { internal: internal(&ip), banned: r.banned.contains(&ip), ip, count: t.count, last: t.last, users: users.into_iter().take(6).map(|u| u.0).collect() }
        })
        .collect();
    all.sort_by(|a, b| b.count.cmp(&a.count).then(b.last.cmp(&a.last)).then(a.ip.cmp(&b.ip)));
    all.truncate(TOP);
    r.attackers = all;
    if now > 0 && !times.is_empty() {
        let start = now.saturating_sub(u64::from(window_min) * 60);
        let mut per = vec![0u32; window_min.min(180) as usize];
        let span = (u64::from(window_min) * 60 / per.len() as u64).max(1);
        for t in times {
            if t >= start {
                let i = (((t - start) / span) as usize).min(per.len() - 1);
                per[i] += 1;
            }
        }
        r.per_minute = per;
    }
    r.accepted.reverse();
    r
}

type Cache = Mutex<Option<HashMap<String, (Instant, Report)>>>;
static CACHE: Cache = Mutex::new(None);

fn run(mut cmd: Command, script_on_stdin: bool, window_min: u32) -> Report {
    cmd.stdin(if script_on_stdin { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::null());
    let out = cmd.spawn().and_then(|mut child| {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(SCRIPT.as_bytes());
        }
        child.wait_with_output()
    });
    match out {
        Ok(o) if !o.stdout.is_empty() => digest(&String::from_utf8_lossy(&o.stdout), window_min),
        Ok(_) => Report { window_min, source: "none".into(), error: Some("연결하지 못했거나 응답이 없습니다 / no answer from the server".into()), ..Default::default() },
        Err(e) => Report { window_min, source: "none".into(), error: Some(e.to_string()), ..Default::default() },
    }
}

fn look(id: &str, window_min: u32) -> Result<Report, String> {
    let key = format!("{id}:{window_min}");
    if let Some((at, r)) = CACHE.lock_safe().as_ref().and_then(|c| c.get(&key)) {
        if at.elapsed() < FRESH {
            return Ok(r.clone());
        }
    }
    let report = if id == "local" {
        let mut cmd = Command::new("sh");
        cmd.args(["-s", "--", &window_min.to_string()]);
        run(cmd, true, window_min)
    } else {
        let servers = crate::remote_servers::load_servers()?;
        let srv = servers.iter().find(|s| s.id == id && !s.member_owned()).ok_or("Server not found")?;
        if !matches!(srv.terminal.as_str(), "ssh" | "") {
            return Err("SSH로 연결하는 서버만 볼 수 있습니다 / only servers reached over SSH".into());
        }
        let (cmd, _pw) = crate::remote_probe::ssh_command(srv, 6, &format!("sh -s -- {window_min}"))?;
        run(cmd, true, window_min)
    };
    let mut guard = CACHE.lock_safe();
    let cache = guard.get_or_insert_with(HashMap::new);
    cache.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(600));
    cache.insert(key, (Instant::now(), report.clone()));
    Ok(report)
}

#[derive(serde::Deserialize)]
pub(crate) struct Query {
    /// Minutes to look back (default 60).
    window: Option<u32>,
}

/// `GET /api/remote-servers/{id}/attacks`: SSH login attempts against a saved
/// server (`local` for this machine). Administrators only.
pub(crate) async fn attacks(path: web::Path<String>, query: web::Query<Query>, req: HttpRequest, users: web::Data<crate::users::UserStore>) -> HttpResponse {
    let user_id = crate::auth::verified_user_id(&req).unwrap_or_default();
    let member = users.lock_safe().iter().find(|u| u.id == user_id).is_none_or(crate::member_servers::is_member);
    if user_id.is_empty() || member {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error("Administrators only"));
    }
    let id = path.into_inner();
    let window = query.window.unwrap_or(60).clamp(5, MAX_WINDOW_MIN);
    match web::block(move || look(&id, window)).await {
        Ok(Ok(report)) => HttpResponse::Ok().json(ApiResponse::ok(report)),
        Ok(Err(e)) => HttpResponse::Ok().json(ApiResponse::<()>::error(&e)),
        Err(_) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error("could not look")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_is_counted_by_address_account_and_minute() {
        let now = 1_800_000_000u64;
        let out = format!(
            "SRC journal\nCAN yes\nNOW {now}\n\
             {a}.1 h sshd[1]: Failed password for root from 192.236.156.181 port 5 ssh2\n\
             {a}.2 h sshd[1]: Failed password for invalid user admin from 192.236.156.181 port 6 ssh2\n\
             {b}.0 h sshd[2]: Invalid user test from 109.160.32.162 port 22\n\
             {b}.5 h sshd[2]: Connection closed by authenticating user root 109.160.32.162 port 1 [preauth]\n\
             {b}.9 h sshd[3]: Failed password for jcia from 172.20.26.200 port 9 ssh2\n\
             {c}.0 h sshd[4]: Accepted password for jcia from 172.20.26.200 port 9 ssh2\n\
             {c}.1 h sshd[5]: Failed password for root from not-an-ip port 1\n\
             BAN 192.236.156.181 junk\n",
            a = now - 3000, b = now - 60, c = now - 10
        );
        let r = digest(&out, 60);
        assert!(r.readable);
        assert_eq!((r.failed, r.sources, r.source.as_str()), (5, 3, "journal"));
        assert_eq!(r.kinds["failed_password"], 3);
        assert_eq!(r.kinds["invalid_user"], 1);
        assert_eq!(r.kinds["preauth_closed"], 1);
        let top = &r.attackers[0];
        assert_eq!((top.ip.as_str(), top.count, top.banned, top.internal), ("109.160.32.162", 2, false, false));
        let first = r.attackers.iter().find(|a| a.ip == "192.236.156.181").unwrap();
        assert_eq!((first.count, first.banned, first.users.clone()), (2, true, vec!["admin".to_string(), "root".to_string()]));
        assert!(r.attackers.iter().find(|a| a.ip == "172.20.26.200").unwrap().internal);
        assert_eq!(r.accepted, vec![Login { ip: "172.20.26.200".into(), user: "jcia".into(), at: now - 10, method: "password".into() }]);
        assert_eq!(r.per_minute.len(), 60);
        assert_eq!(r.per_minute.iter().sum::<u32>(), 5);
        assert_eq!(r.per_minute[59] + r.per_minute[58], 3);
    }

    #[test]
    fn an_unreadable_log_says_so() {
        let r = digest("SRC none\nCAN no\nNOW 1800000000\nBAN \n", 60);
        assert!(!r.readable);
        assert_eq!((r.failed, r.source.as_str()), (0, "none"));
    }

    #[test]
    fn the_script_runs_here_and_answers_in_its_own_format() {
        let mut cmd = Command::new("sh");
        cmd.args(["-s", "--", "5"]);
        let r = run(cmd, true, 5);
        assert!(r.checked_at > 1_700_000_000, "{r:?}");
        assert!(!r.source.is_empty());
    }
}
