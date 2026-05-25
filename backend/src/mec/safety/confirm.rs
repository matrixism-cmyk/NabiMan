use crate::mec::services::ServiceError;
use actix_web::HttpRequest;

const HEADER: &str = "X-Confirm-Name";

/// Ensures the client sent `X-Confirm-Name: <expected>` header. This is a
/// lightweight guard against accidental curl/fetch deletions of critical
/// resources (tenants, NAT rules) in a live cluster.
///
/// Bypassed entirely when the env var NABIMAN_MEC_SKIP_CONFIRM=1 is set,
/// so dev / automation can opt out explicitly.
pub fn require_confirm_header(req: &HttpRequest, expected: &str) -> Result<(), ServiceError> {
    if std::env::var("NABIMAN_MEC_SKIP_CONFIRM")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
    {
        return Ok(());
    }
    let got = req
        .headers()
        .get(HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if got != expected {
        return Err(ServiceError::InvalidInput(format!(
            "파괴적 작업은 '{}: {}' 헤더로 확인이 필요합니다. 받은 값: '{}'",
            HEADER, expected, got
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;

    #[actix_web::test]
    async fn missing_header_rejected() {
        let req = test::TestRequest::default().to_http_request();
        assert!(require_confirm_header(&req, "x").is_err());
    }

    #[actix_web::test]
    async fn matching_header_ok() {
        let req = test::TestRequest::default()
            .insert_header(("X-Confirm-Name", "x"))
            .to_http_request();
        assert!(require_confirm_header(&req, "x").is_ok());
    }

    #[actix_web::test]
    async fn mismatch_rejected() {
        let req = test::TestRequest::default()
            .insert_header(("X-Confirm-Name", "wrong"))
            .to_http_request();
        assert!(require_confirm_header(&req, "x").is_err());
    }

    #[actix_web::test]
    async fn env_bypass() {
        std::env::set_var("NABIMAN_MEC_SKIP_CONFIRM", "1");
        let req = test::TestRequest::default().to_http_request();
        assert!(require_confirm_header(&req, "x").is_ok());
        std::env::remove_var("NABIMAN_MEC_SKIP_CONFIRM");
    }
}
