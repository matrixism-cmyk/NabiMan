//! Renewing and issuing certificates: certbot in the background, one job per
//! domain at a time, its outcome kept for the page to ask after.
use super::*;
use crate::bounded_command::{deadline, OutputWithin};
use std::collections::HashMap;
use crate::lock_safe::LockExt;
use std::sync::{Mutex, OnceLock};

#[derive(Serialize, Clone)]
pub struct RenewStatus {
    pub domain: String,
    pub status: String, // "running", "success", "error"
    pub message: String,
    pub started_at: String,
}

fn renew_jobs() -> &'static Mutex<HashMap<String, RenewStatus>> {
    static JOBS: OnceLock<Mutex<HashMap<String, RenewStatus>>> = OnceLock::new();
    JOBS.get_or_init(Default::default)
}

#[derive(Deserialize)]
pub struct DomainRequest {
    pub domain: String,
}

fn valid(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && !domain.starts_with('-')
        && domain.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
}

/// Starts `certbot args` for `domain` unless a job for it is already running,
/// which is answered instead. The run is bounded by `deadline::CERTIFICATE`.
fn start(req: &actix_web::HttpRequest, action: &str, domain: String, args: Vec<String>, running: &str, started: &str) -> HttpResponse {
    if !valid(&domain) {
        return HttpResponse::Ok().json(ApiResponse::<String>::error("Invalid domain"));
    }
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    {
        let mut jobs = renew_jobs().lock_safe();
        if let Some(job) = jobs.get(&domain).filter(|j| j.status == "running") {
            return HttpResponse::Ok().json(ApiResponse::ok(job.clone()));
        }
        let job = RenewStatus { domain: domain.clone(), status: "running".into(), message: running.into(), started_at: now.clone() };
        jobs.insert(domain.clone(), job);
    }
    crate::audit_change::note(req, crate::audit_change::Change::new("certificate", &domain, action).field("job", None, Some("started")));
    let d = domain.clone();
    std::thread::spawn(move || {
        let output = std::process::Command::new("certbot").args(&args).output_within(deadline::CERTIFICATE);
        let (status, message) = match output {
            Ok(o) => {
                let msg = format!("{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
                    .trim()
                    .to_string();
                (if o.status.success() { "success" } else { "error" }, msg)
            }
            Err(e) => ("error", format!("certbot: {e}")),
        };
        if let Some(job) = renew_jobs().lock_safe().get_mut(&d) {
            job.status = status.into();
            job.message = message;
        }
    });
    HttpResponse::Ok().json(ApiResponse::ok(RenewStatus { domain, status: "running".into(), message: started.into(), started_at: now }))
}

pub(super) async fn renew_certificate(req: actix_web::HttpRequest, body: web::Json<DomainRequest>) -> HttpResponse {
    let args = ["renew", "--cert-name", &body.domain, "--force-renewal", "--non-interactive", "--no-random-sleep-on-renew"];
    start(&req, "renew", body.domain.clone(), args.map(String::from).to_vec(), "Renewing...", "Renewal started in background")
}

pub(super) async fn renew_status(path: web::Path<String>) -> HttpResponse {
    match renew_jobs().lock_safe().get(path.as_str()) {
        Some(job) => HttpResponse::Ok().json(ApiResponse::ok(job.clone())),
        None => HttpResponse::Ok().json(ApiResponse::<RenewStatus>::error("No renewal job found")),
    }
}

/// A new certificate needs the Let's Encrypt account address
/// (`NABIMAN_LETSENCRYPT_EMAIL`); without one the request is refused, saying so.
pub(super) async fn issue_certificate(req: actix_web::HttpRequest, body: web::Json<DomainRequest>) -> HttpResponse {
    let email = match crate::install_config::settings::letsencrypt_email() {
        Ok(email) => email,
        Err(why) => return HttpResponse::Ok().json(ApiResponse::<String>::error(&why)),
    };
    let args = [
        "--apache", "-d", &body.domain, "--non-interactive", "--agree-tos", "--no-eff-email", "--email", &email,
    ];
    start(&req, "issue", body.domain.clone(), args.map(String::from).to_vec(), "Issuing...", "Certificate issuance started")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn a_new_certificate_needs_the_lets_encrypt_address() {
        if std::env::var_os("NABIMAN_LETSENCRYPT_EMAIL").is_some() {
            return; // set where this runs: issuing would really start certbot
        }
        let res = issue_certificate(actix_web::test::TestRequest::default().to_http_request(), web::Json(DomainRequest { domain: "example.com".into() })).await;
        let body = actix_web::body::to_bytes(res.into_body()).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["success"], false);
        assert!(body["message"].as_str().unwrap().contains("NABIMAN_LETSENCRYPT_EMAIL"), "{body}");
        assert!(renew_jobs().lock_safe().get("example.com").is_none(), "nothing was started");
    }
}
