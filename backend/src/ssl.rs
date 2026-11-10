#[path = "ssl_jobs.rs"]
mod jobs;
use jobs::{issue_certificate, renew_certificate, renew_status};

use crate::bounded_command::{deadline, OutputWithin};
use crate::models::ApiResponse;
use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
pub struct SslCertificate {
    pub domain: String,
    pub issuer: String,
    pub expiry: String,
    pub days_left: i64,
    pub path: String,
    pub status: String,
}

fn parse_certbot_output(output: &str) -> Vec<SslCertificate> {
    let mut certs = Vec::new();
    let mut domain = String::new();
    let mut expiry = String::new();
    let mut path = String::new();

    let flush =
        |certs: &mut Vec<SslCertificate>, d: &mut String, e: &mut String, p: &mut String| {
            if !d.is_empty() && !e.is_empty() {
                let (exp_str, days) = parse_expiry(e);
                let status = if days < 0 {
                    "expired"
                } else if days < 14 {
                    "warning"
                } else {
                    "valid"
                };
                certs.push(SslCertificate {
                    domain: d.clone(),
                    issuer: "Let's Encrypt".into(),
                    expiry: exp_str,
                    days_left: days,
                    path: p.clone(),
                    status: status.into(),
                });
            }
            d.clear();
            e.clear();
            p.clear();
        };

    for line in output.lines() {
        let t = line.trim();
        if t.starts_with("Certificate Name:") {
            flush(&mut certs, &mut domain, &mut expiry, &mut path);
            domain = t.trim_start_matches("Certificate Name:").trim().into();
        } else if t.starts_with("Expiry Date:") {
            expiry = t.trim_start_matches("Expiry Date:").trim().into();
        } else if t.starts_with("Certificate Path:") {
            path = t.trim_start_matches("Certificate Path:").trim().into();
        }
    }
    flush(&mut certs, &mut domain, &mut expiry, &mut path);
    certs
}

fn parse_expiry(raw: &str) -> (String, i64) {
    let date_part = raw.split('(').next().unwrap_or(raw).trim().to_string();
    let days = if let Some(start) = raw.find("VALID:") {
        raw[start + 6..]
            .split_whitespace()
            .next()
            .and_then(|d| d.parse::<i64>().ok())
            .unwrap_or(0)
    } else if raw.contains("EXPIRED") {
        -1
    } else {
        0
    };
    (date_part, days)
}

/// Fallback: read cert expiry directly from filesystem when certbot is locked
fn list_from_filesystem() -> Vec<SslCertificate> {
    let live_dir = std::path::Path::new("/etc/letsencrypt/live");
    if !live_dir.exists() {
        return Vec::new();
    }
    let mut certs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(live_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "README" {
                continue;
            }
            let cert_path = entry.path().join("fullchain.pem");
            if !cert_path.exists() {
                continue;
            }
            // Use openssl to read expiry
            let expiry_info = std::process::Command::new("openssl")
                .args(["x509", "-enddate", "-noout", "-in"])
                .arg(&cert_path)
                .output_within(deadline::QUICK)
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            // Format: notAfter=Jun 21 01:53:31 2026 GMT
            let expiry_str = expiry_info.trim_start_matches("notAfter=").to_string();
            let days = calc_days_left(&expiry_str);
            let status = if days < 0 {
                "expired"
            } else if days < 14 {
                "warning"
            } else {
                "valid"
            };
            certs.push(SslCertificate {
                domain: name,
                issuer: "Let's Encrypt".into(),
                expiry: expiry_str,
                days_left: days,
                path: cert_path.to_string_lossy().to_string(),
                status: status.into(),
            });
        }
    }
    certs.sort_by(|a, b| a.domain.cmp(&b.domain));
    certs
}

fn calc_days_left(expiry: &str) -> i64 {
    // Parse "Jun 21 01:53:31 2026 GMT"
    chrono::NaiveDateTime::parse_from_str(expiry.trim_end_matches(" GMT"), "%b %d %H:%M:%S %Y")
        .map(|dt| {
            let exp = dt.and_utc();
            (exp - chrono::Utc::now()).num_days()
        })
        .unwrap_or(0)
}

fn list_vhost_domains() -> Vec<String> {
    let output = std::process::Command::new("sh")
        .args(["-c", "grep -rh 'ServerName' /etc/apache2/sites-enabled/ 2>/dev/null || grep -rh 'server_name' /etc/nginx/sites-enabled/ 2>/dev/null || true"])
        .output_within(deadline::QUICK)
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    output
        .lines()
        .filter_map(|l| {
            let name = l
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .trim_end_matches(';')
                .to_string();
            if name.is_empty()
                || name.contains('*')
                || name == "_"
                || name.contains("example")
                || name == "redirection"
                || name == "The"
            {
                None
            } else {
                Some(name)
            }
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

async fn list_certificates() -> HttpResponse {
    // certbot, openssl per certificate, grep: on the blocking pool.
    match web::block(collect_certificates).await {
        Ok(certs) => HttpResponse::Ok().json(ApiResponse::ok(certs)),
        Err(_) => HttpResponse::Ok().json(ApiResponse::<()>::error("Listing certificates stopped unexpectedly")),
    }
}

fn collect_certificates() -> Vec<SslCertificate> {
    // Try certbot first, fall back to filesystem if locked/fails
    let output = std::process::Command::new("certbot")
        .args(["certificates", "--non-interactive"])
        .output_within(deadline::SERVICE);

    let mut certs = match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout).to_string()
                + &String::from_utf8_lossy(&o.stderr);
            let parsed = parse_certbot_output(&text);
            if parsed.is_empty() {
                list_from_filesystem()
            } else {
                parsed
            }
        }
        _ => list_from_filesystem(),
    };
    // Add uncertified vhost domains
    let vhosts = list_vhost_domains();
    let certified: std::collections::HashSet<String> =
        certs.iter().map(|c| c.domain.clone()).collect();
    for domain in vhosts {
        if !certified.contains(&domain) {
            certs.push(SslCertificate {
                domain,
                issuer: "-".into(),
                expiry: "-".into(),
                days_left: 0,
                path: String::new(),
                status: "none".into(),
            });
        }
    }
    certs.sort_by(|a, b| a.domain.cmp(&b.domain));
    certs
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/ssl")
            .route("/certificates", web::get().to(list_certificates))
            .route("/renew", web::post().to(renew_certificate))
            .route("/renew/{domain}", web::get().to(renew_status))
            .route("/issue", web::post().to(issue_certificate)),
    );
}
