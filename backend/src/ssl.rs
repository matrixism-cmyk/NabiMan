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

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Certificate Name:") {
            domain = trimmed.trim_start_matches("Certificate Name:").trim().to_string();
        } else if trimmed.starts_with("Expiry Date:") {
            expiry = trimmed.trim_start_matches("Expiry Date:").trim().to_string();
        } else if trimmed.starts_with("Certificate Path:") {
            path = trimmed.trim_start_matches("Certificate Path:").trim().to_string();
        } else if trimmed.starts_with("Key Type:") || trimmed.starts_with("Serial") {
            // skip
        } else if trimmed.starts_with("- - -") || (trimmed.is_empty() && !domain.is_empty()) {
            if !domain.is_empty() && !expiry.is_empty() {
                let (exp_str, days) = parse_expiry(&expiry);
                let status = if days < 0 { "expired" }
                    else if days < 14 { "warning" }
                    else { "valid" };
                certs.push(SslCertificate {
                    domain: domain.clone(),
                    issuer: "Let's Encrypt".to_string(),
                    expiry: exp_str,
                    days_left: days,
                    path: path.clone(),
                    status: status.to_string(),
                });
                domain.clear(); expiry.clear(); path.clear();
            }
        }
    }
    if !domain.is_empty() && !expiry.is_empty() {
        let (exp_str, days) = parse_expiry(&expiry);
        let status = if days < 0 { "expired" } else if days < 14 { "warning" } else { "valid" };
        certs.push(SslCertificate {
            domain, issuer: "Let's Encrypt".to_string(), expiry: exp_str,
            days_left: days, path, status: status.to_string(),
        });
    }
    certs
}

fn parse_expiry(raw: &str) -> (String, i64) {
    // Format: "2026-06-21 07:47:16+00:00 (VALID: 89 days)"
    let date_part = raw.split('(').next().unwrap_or(raw).trim().to_string();
    let days = if let Some(start) = raw.find("VALID:") {
        raw[start+6..].split_whitespace().next()
            .and_then(|d| d.parse::<i64>().ok()).unwrap_or(0)
    } else if raw.contains("EXPIRED") { -1 } else { 0 };
    (date_part, days)
}

async fn list_certificates() -> HttpResponse {
    let output = std::process::Command::new("certbot")
        .args(["certificates"]).output();

    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).to_string()
                + &String::from_utf8_lossy(&o.stderr);
            let certs = parse_certbot_output(&text);
            HttpResponse::Ok().json(ApiResponse::ok(certs))
        }
        Err(_) => HttpResponse::Ok().json(
            ApiResponse::<Vec<SslCertificate>>::error("certbot not installed")
        ),
    }
}

#[derive(Deserialize)]
pub struct RenewRequest {
    pub domain: String,
}

async fn renew_certificate(body: web::Json<RenewRequest>) -> HttpResponse {
    let domain = &body.domain;
    if domain.is_empty() || domain.len() > 253
        || !domain.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-') {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid domain"));
    }

    let output = std::process::Command::new("certbot")
        .args(["renew", "--cert-name", domain, "--force-renewal"])
        .output();

    match output {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            let err = String::from_utf8_lossy(&o.stderr).to_string();
            let combined = format!("{}\n{}", out, err).trim().to_string();
            if o.status.success() {
                HttpResponse::Ok().json(ApiResponse::ok(combined))
            } else {
                HttpResponse::Ok().json(ApiResponse::<String>::error(&combined))
            }
        }
        Err(e) => HttpResponse::Ok().json(ApiResponse::<String>::error(&e.to_string())),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/ssl")
            .route("/certificates", web::get().to(list_certificates))
            .route("/renew", web::post().to(renew_certificate))
    );
}
