//! Bootstrap secrets are returned only to the creating request, never persisted.
use super::services::{ServiceError, ServiceResult};
use rand::RngCore;
use serde::Serialize;

#[derive(Serialize)]
pub struct InitialCredentials {
    pub username: String,
    pub password: String,
}
pub fn generate_password() -> ServiceResult<String> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut entropy = [0u8; 48];
    rand::rngs::OsRng
        .try_fill_bytes(&mut entropy)
        .map_err(|_| ServiceError::Internal("Secure random source unavailable".into()))?;
    Ok(entropy
        .iter()
        .map(|b| ALPHABET[(b & 63) as usize] as char)
        .collect())
}
pub fn validate_password(password: &str) -> ServiceResult<()> {
    if password.chars().count() < 15
        || password.len() > 128
        || password.chars().any(char::is_control)
    {
        return Err(ServiceError::InvalidInput("Password must have at least 15 characters, at most 128 UTF-8 bytes, and no control characters".into()));
    }
    if [
        "passwordpassword",
        "123456789012345",
        "abcdefghijklmnop",
        "qwertyqwertyqwerty",
    ]
    .contains(&password.to_ascii_lowercase().as_str())
    {
        return Err(ServiceError::InvalidInput(
            "Choose a less predictable password".into(),
        ));
    }
    Ok(())
}
/// Credential APIs may echo request bodies in provider errors. Never propagate
/// those messages to jobs, audits, profiles or responses.
pub fn safe_error(error: ServiceError) -> ServiceError {
    match error {
        ServiceError::Forbidden(_) => {
            ServiceError::Forbidden("Rancher credential authorization denied".into())
        }
        ServiceError::NotFound(_) => ServiceError::NotFound("Rancher credential target".into()),
        ServiceError::Conflict(_) => {
            ServiceError::Conflict("Rancher rejected credential operation".into())
        }
        ServiceError::Unauthorized(_) => {
            ServiceError::Unauthorized("Rancher credential authorization failed".into())
        }
        ServiceError::InvalidInput(_) => {
            ServiceError::InvalidInput("Rancher rejected credential input".into())
        }
        ServiceError::Unavailable(_) => {
            ServiceError::Unavailable("Rancher credential service unavailable".into())
        }
        ServiceError::Internal(_) => {
            ServiceError::Internal("Rancher credential operation failed".into())
        }
        ServiceError::Upstream { .. } => ServiceError::Upstream {
            source: "rancher".into(),
            message: "Credential operation failed; verify Rancher availability and policy".into(),
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secure_bootstrap_secrets_are_unique_and_accepted() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..128 {
            let p = generate_password().unwrap();
            assert_eq!(p.len(), 48);
            validate_password(&p).unwrap();
            assert!(seen.insert(p));
        }
    }
    #[test]
    fn rejects_weak_or_unbounded_credential_inputs() {
        for p in [
            "changeme!".to_string(),
            "passwordpassword".into(),
            "123456789012345".into(),
            "x".repeat(129),
            "valid-long-pass\n".into(),
        ] {
            assert!(validate_password(&p).is_err());
        }
        validate_password("a lengthy personal passphrase").unwrap();
    }
    #[test]
    fn credential_error_messages_never_retain_provider_echoes() {
        let error = safe_error(ServiceError::Upstream {
            source: "rancher".into(),
            message: "password=SECRET".into(),
        });
        assert!(!error.to_string().contains("SECRET"));
        assert_eq!(error.http_status(), 502);
    }

    #[actix_web::test]
    async fn rancher_wire_requests_force_change_and_provider_echoes_are_redacted() {
        use crate::mec::services::{
            rancher_real::{RancherReal, RancherRealConfig},
            rancher_service::RancherService,
        };
        use actix_web::{web, App, HttpResponse};
        let server = actix_test::start(|| {
            App::new().route("/v3/users",web::post().to(|body:web::Json<serde_json::Value>|async move {
            if body["username"]=="echo-error" {return HttpResponse::BadRequest().json(body.into_inner());}
            assert_eq!(body["mustChangePassword"],true);
            assert_eq!(body["password"].as_str().unwrap().len(),48);
            HttpResponse::Created().json(serde_json::json!({"id":"u-wire","username":"wire-user","enabled":true,"mustChangePassword":true}))
        }))
        });
        let client = RancherReal::new(RancherRealConfig {
            base_url: server.url(""),
            token: "isolated-fixture-token".into(),
            cluster_id: "c-local".into(),
            insecure_tls: false,
        })
        .unwrap();
        let password = generate_password().unwrap();
        let user = client.create_user("wire-user", &password).await.unwrap();
        assert!(user.must_change_password);
        let error = client
            .create_user("echo-error", &password)
            .await
            .unwrap_err();
        assert!(!error.to_string().contains(&password));
        assert_eq!(error.http_status(), 502);
    }
}
