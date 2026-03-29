use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
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

    let flush = |certs: &mut Vec<SslCertificate>, d: &mut String, e: &mut String, p: &mut String| {
        if !d.is_empty() && !e.is_empty() {
            let (exp_str, days) = parse_expiry(e);
            let status = if days < 0 { "expired" } else if days < 14 { "warning" } else { "valid" };
            certs.push(SslCertificate {
                domain: d.clone(), issuer: "Let's Encrypt".into(),
                expiry: exp_str, days_left: days, path: p.clone(), status: status.into(),
            });
        }
        d.clear(); e.clear(); p.clear();
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
        raw[start+6..].split_whitespace().next()
            .and_then(|d| d.parse::<i64>().ok()).unwrap_or(0)
    } else if raw.contains("EXPIRED") { -1 } else { 0 };
    (date_part, days)
}

/// Fallback: read cert expiry directly from filesystem when certbot is locked
fn list_from_filesystem() -> Vec<SslCertificate> {
    let live_dir = std::path::Path::new("/etc/letsencrypt/live");
    if !live_dir.exists() { return Vec::new(); }
    let mut certs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(live_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "README" { continue; }
            let cert_path = entry.path().join("fullchain.pem");
            if !cert_path.exists() { continue; }
            // Use openssl to read expiry
            let expiry_info = std::process::Command::new("openssl")
                .args(["x509", "-enddate", "-noout", "-in"])
                .arg(&cert_path).output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            // Format: notAfter=Jun 21 01:53:31 2026 GMT
            let expiry_str = expiry_info.trim_start_matches("notAfter=").to_string();
            let days = calc_days_left(&expiry_str);
            let status = if days < 0 { "expired" } else if days < 14 { "warning" } else { "valid" };
            certs.push(SslCertificate {
                domain: name, issuer: "Let's Encrypt".into(),
                expiry: expiry_str, days_left: days,
                path: cert_path.to_string_lossy().to_string(), status: status.into(),
            });
        }
    }
    certs.sort_by(|a, b| a.domain.cmp(&b.domain));
    certs
}

fn calc_days_left(expiry: &str) -> i64 {
    // Parse "Jun 21 01:53:31 2026 GMT"
    chrono::NaiveDateTime::parse_from_str(
        expiry.trim_end_matches(" GMT"), "%b %d %H:%M:%S %Y"
    ).map(|dt| {
        let exp = dt.and_utc();
        (exp - chrono::Utc::now()).num_days()
    }).unwrap_or(0)
}

fn list_vhost_domains() -> Vec<String> {
    let output = std::process::Command::new("sh")
        .args(["-c", "grep -rh 'ServerName' /etc/apache2/sites-enabled/ 2>/dev/null || grep -rh 'server_name' /etc/nginx/sites-enabled/ 2>/dev/null || true"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    output.lines()
        .filter_map(|l| {
            let name = l.split_whitespace().nth(1).unwrap_or("").trim_end_matches(';').to_string();
            if name.is_empty() || name.contains('*') || name == "_"
                || name.contains("example") || name == "redirection" || name == "The" {
                None
            } else { Some(name) }
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter().collect()
}

async fn list_certificates() -> HttpResponse {
    // Try certbot first, fall back to filesystem if locked/fails
    let output = std::process::Command::new("certbot")
        .args(["certificates", "--non-interactive"]).output();

    let mut certs = match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout).to_string()
                + &String::from_utf8_lossy(&o.stderr);
            let parsed = parse_certbot_output(&text);
            if parsed.is_empty() { list_from_filesystem() } else { parsed }
        }
        _ => list_from_filesystem(),
    };
    // Add uncertified vhost domains
    let vhosts = list_vhost_domains();
    let certified: std::collections::HashSet<String> = certs.iter().map(|c| c.domain.clone()).collect();
    for domain in vhosts {
        if !certified.contains(&domain) {
            certs.push(SslCertificate {
                domain, issuer: "-".into(), expiry: "-".into(),
                days_left: 0, path: String::new(), status: "none".into(),
            });
        }
    }
    certs.sort_by(|a, b| a.domain.cmp(&b.domain));
    HttpResponse::Ok().json(ApiResponse::ok(certs))
}

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Serialize, Clone)]
pub struct RenewStatus {
    pub domain: String,
    pub status: String, // "running", "success", "error"
    pub message: String,
    pub started_at: String,
}

type RenewJobs = Arc<Mutex<HashMap<String, RenewStatus>>>;

fn renew_jobs() -> &'static RenewJobs {
    use std::sync::OnceLock;
    static JOBS: OnceLock<RenewJobs> = OnceLock::new();
    JOBS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

#[derive(Deserialize)]
pub struct RenewRequest { pub domain: String }

async fn renew_certificate(body: web::Json<RenewRequest>) -> HttpResponse {
    let domain = body.domain.clone();
    if domain.is_empty() || domain.len() > 253
        || !domain.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid domain"));
    }
    let jobs = renew_jobs();
    // Check if already running
    if let Some(job) = jobs.lock().unwrap().get(&domain) {
        if job.status == "running" {
            return HttpResponse::Ok().json(ApiResponse::ok(job.clone()));
        }
    }
    // Start background renewal
    jobs.lock().unwrap().insert(domain.clone(), RenewStatus {
        domain: domain.clone(), status: "running".into(), message: "Renewing...".into(),
        started_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    });
    let jobs2 = jobs.clone();
    let d = domain.clone();
    std::thread::spawn(move || {
        let output = std::process::Command::new("timeout")
            .args(["120", "certbot", "renew", "--cert-name", &d,
                   "--force-renewal", "--non-interactive", "--no-random-sleep-on-renew"])
            .output();
        let (status, message) = match output {
            Ok(o) => {
                let msg = format!("{}\n{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)).trim().to_string();
                if o.status.success() { ("success".into(), msg) }
                else { ("error".into(), msg) }
            }
            Err(e) => ("error".into(), e.to_string()),
        };
        if let Ok(mut map) = jobs2.lock() {
            if let Some(job) = map.get_mut(&d) {
                job.status = status;
                job.message = message;
            }
        }
    });
    HttpResponse::Ok().json(ApiResponse::ok(RenewStatus {
        domain, status: "running".into(), message: "Renewal started in background".into(),
        started_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    }))
}

async fn renew_status(path: web::Path<String>) -> HttpResponse {
    let domain = path.into_inner();
    let jobs = renew_jobs();
    match jobs.lock().unwrap().get(&domain) {
        Some(job) => HttpResponse::Ok().json(ApiResponse::ok(job.clone())),
        None => HttpResponse::Ok().json(ApiResponse::<RenewStatus>::error("No renewal job found")),
    }
}

#[derive(Deserialize)]
pub struct IssueRequest { pub domain: String }

async fn issue_certificate(body: web::Json<IssueRequest>) -> HttpResponse {
    let domain = body.domain.clone();
    if domain.is_empty() || domain.len() > 253
        || !domain.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid domain"));
    }
    let jobs = renew_jobs();
    if let Some(job) = jobs.lock().unwrap().get(&domain) {
        if job.status == "running" {
            return HttpResponse::Ok().json(ApiResponse::ok(job.clone()));
        }
    }
    jobs.lock().unwrap().insert(domain.clone(), RenewStatus {
        domain: domain.clone(), status: "running".into(), message: "Issuing...".into(),
        started_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    });
    let jobs2 = jobs.clone();
    let d = domain.clone();
    std::thread::spawn(move || {
        let output = std::process::Command::new("timeout")
            .args(["120", "certbot", "--apache", "-d", &d,
                   "--non-interactive", "--agree-tos", "--no-eff-email",
                   "--email", "webmaster@xos.kr"])
            .output();
        let (status, message) = match output {
            Ok(o) => {
                let msg = format!("{}\n{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)).trim().to_string();
                if o.status.success() { ("success".into(), msg) } else { ("error".into(), msg) }
            }
            Err(e) => ("error".into(), e.to_string()),
        };
        if let Ok(mut map) = jobs2.lock() {
            if let Some(job) = map.get_mut(&d) {
                job.status = status;
                job.message = message;
            }
        }
    });
    HttpResponse::Ok().json(ApiResponse::ok(RenewStatus {
        domain, status: "running".into(), message: "Certificate issuance started".into(),
        started_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    }))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/ssl")
            .route("/certificates", web::get().to(list_certificates))
            .route("/renew", web::post().to(renew_certificate))
            .route("/renew/{domain}", web::get().to(renew_status))
            .route("/issue", web::post().to(issue_certificate))
    );
}
