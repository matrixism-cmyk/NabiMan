use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
pub struct DnsStatus {
    pub service: DnsServiceInfo,
    pub health: DnsHealthCheck,
    pub records: DnsRecordStats,
    pub users: DnsUserStats,
    pub metrics: Option<DnsMetrics>,
    pub recent_changes: Vec<DnsRecentChange>,
}

#[derive(Serialize)]
pub struct DnsServiceInfo {
    pub running: bool,
    pub pid: String,
    pub memory: String,
    pub uptime: String,
}

#[derive(Serialize)]
pub struct DnsHealthCheck {
    pub ok: bool,
    pub response: String,
    pub response_ms: u64,
}

#[derive(Serialize)]
pub struct DnsRecordStats {
    pub total: u64,
    pub active: u64,
    pub domains: u64,
}

#[derive(Serialize)]
pub struct DnsUserStats {
    pub total: u64,
    pub active: u64,
    pub pending: u64,
    pub disabled: u64,
}

#[derive(Serialize)]
pub struct DnsMetrics {
    pub queries_total: String,
    pub cache_hits: String,
    pub cache_misses: String,
    pub cache_hit_ratio: String,
    pub responses_nxdomain: String,
    pub rate_limited: String,
    pub acl_denied: String,
}

#[derive(Serialize)]
pub struct DnsRecentChange {
    pub name: String,
    pub rtype: String,
    pub updated_at: String,
}

fn service_info() -> DnsServiceInfo {
    let running = std::process::Command::new("systemctl")
        .args(["is-active", "nabidns-auth"])
        .output().map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false);

    let show = std::process::Command::new("systemctl")
        .args(["show", "nabidns-auth", "-p", "MainPID,MemoryCurrent,ActiveEnterTimestamp"])
        .output().map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut pid = String::new();
    let mut memory = String::new();
    let mut uptime = String::new();
    for line in show.lines() {
        if let Some(v) = line.strip_prefix("MainPID=") { pid = v.to_string(); }
        else if let Some(v) = line.strip_prefix("MemoryCurrent=") {
            let bytes: u64 = v.parse().unwrap_or(0);
            memory = format_bytes(bytes);
        }
        else if let Some(v) = line.strip_prefix("ActiveEnterTimestamp=") {
            uptime = v.trim().to_string();
        }
    }
    DnsServiceInfo { running, pid, memory, uptime }
}

fn format_bytes(b: u64) -> String {
    if b < 1024 { return format!("{} B", b); }
    if b < 1048576 { return format!("{:.1} KB", b as f64 / 1024.0); }
    if b < 1073741824 { return format!("{:.1} MB", b as f64 / 1048576.0); }
    format!("{:.1} GB", b as f64 / 1073741824.0)
}

fn health_check() -> DnsHealthCheck {
    let start = Instant::now();
    let output = std::process::Command::new("dig")
        .args(["@127.0.0.1", "xos.kr", "A", "+short", "+time=3"])
        .output();
    let elapsed = start.elapsed().as_millis() as u64;

    match output {
        Ok(o) => {
            let resp = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let ok = resp.contains("115.68.193.237");
            DnsHealthCheck { ok, response: resp, response_ms: elapsed }
        }
        Err(e) => DnsHealthCheck { ok: false, response: e.to_string(), response_ms: elapsed },
    }
}

fn mysql_query(sql: &str) -> String {
    std::process::Command::new("mysql")
        .args(["-u", "nabidns", "-pNabi48dc79a162b27a27aec2eb28", "-h", "127.0.0.1", "nabidns", "-BNe", sql])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn record_stats() -> DnsRecordStats {
    let total: u64 = mysql_query("SELECT COUNT(*) FROM dns_records").parse().unwrap_or(0);
    let active: u64 = mysql_query("SELECT COUNT(*) FROM dns_records WHERE active=1").parse().unwrap_or(0);
    let domains: u64 = mysql_query("SELECT COUNT(DISTINCT name) FROM dns_records").parse().unwrap_or(0);
    DnsRecordStats { total, active, domains }
}

fn user_stats() -> DnsUserStats {
    let total: u64 = mysql_query("SELECT COUNT(*) FROM nabidns_users").parse().unwrap_or(0);
    let active: u64 = mysql_query("SELECT COUNT(*) FROM nabidns_users WHERE status='active'").parse().unwrap_or(0);
    let pending: u64 = mysql_query("SELECT COUNT(*) FROM nabidns_users WHERE status='pending'").parse().unwrap_or(0);
    let disabled: u64 = mysql_query("SELECT COUNT(*) FROM nabidns_users WHERE status='disabled'").parse().unwrap_or(0);
    DnsUserStats { total, active, pending, disabled }
}

fn read_metrics() -> Option<DnsMetrics> {
    let sock = "/run/nabidns/metrics.sock";
    if !std::path::Path::new(sock).exists() { return None; }
    let output = std::process::Command::new("socat")
        .args(["-", &format!("UNIX-CONNECT:{}", sock)])
        .output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut m = DnsMetrics {
        queries_total: "0".into(), cache_hits: "0".into(), cache_misses: "0".into(),
        cache_hit_ratio: "0".into(), responses_nxdomain: "0".into(),
        rate_limited: "0".into(), acl_denied: "0".into(),
    };
    for line in text.lines() {
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 { continue; }
        let (k, v) = (parts[0].trim(), parts[1].trim().to_string());
        match k {
            "queries_total" => m.queries_total = v,
            "cache_hits" => m.cache_hits = v,
            "cache_misses" => m.cache_misses = v,
            "cache_hit_ratio" => m.cache_hit_ratio = v,
            "responses_nxdomain" => m.responses_nxdomain = v,
            "rate_limited" => m.rate_limited = v,
            "acl_denied" => m.acl_denied = v,
            _ => {}
        }
    }
    Some(m)
}

fn recent_changes() -> Vec<DnsRecentChange> {
    let output = mysql_query(
        "SELECT name, rtype, updated_at FROM dns_records ORDER BY updated_at DESC LIMIT 10"
    );
    output.lines().filter_map(|line| {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let rtype_str = match parts[1] {
                "1" => "A", "2" => "NS", "5" => "CNAME", "6" => "SOA",
                "15" => "MX", "16" => "TXT", "28" => "AAAA", "33" => "SRV",
                other => other,
            };
            Some(DnsRecentChange {
                name: parts[0].to_string(),
                rtype: rtype_str.to_string(),
                updated_at: parts[2].to_string(),
            })
        } else { None }
    }).collect()
}

async fn dns_status() -> HttpResponse {
    let status = DnsStatus {
        service: service_info(),
        health: health_check(),
        records: record_stats(),
        users: user_stats(),
        metrics: read_metrics(),
        recent_changes: recent_changes(),
    };
    HttpResponse::Ok().json(ApiResponse::ok(status))
}

async fn dns_available() -> HttpResponse {
    let running = std::process::Command::new("systemctl")
        .args(["is-active", "nabidns-auth"])
        .output().map(|o| o.status.success()).unwrap_or(false);
    HttpResponse::Ok().json(ApiResponse::ok(running))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/dns")
            .route("/available", web::get().to(dns_available))
            .route("/status", web::get().to(dns_status))
    );
}
