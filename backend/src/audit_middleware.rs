use crate::audit::{append_entry, AuditEntry, AuditLog};
use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::rc::Rc;

/// Capability IDs in share paths are bearer secrets, including on failed requests.
/// Preserve stable correlation without recording the value that grants access.
pub(crate) fn audit_path(path: &str) -> String {
    use sha2::{Digest, Sha256};
    let path = path.split('?').next().unwrap_or(path);
    for prefix in ["/api/share/", "/api/terminal/share-links/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            let (token, suffix) = rest
                .split_once('/')
                .map_or((rest, ""), |(token, suffix)| (token, suffix));
            let id = hex::encode(Sha256::digest(token.as_bytes()));
            let suffix = match (prefix, suffix) {
                (_, "") => "",
                ("/api/share/", "auth") => "/auth",
                ("/api/share/", "ws") => "/ws",
                _ => "/[unrecognized]",
            };
            return format!("{prefix}[cap:{}]{suffix}", &id[..32]);
        }
    }
    path.to_string()
}

// High-frequency endpoints to skip
const SKIP_PATHS: &[&str] = &[
    "/api/server/status",
    "/api/server/history",
    "/api/traffic/current",
    "/api/traffic/summary",
    "/api/containers/available",
    "/api/database/available",
    "/api/mail/available",
    "/api/dns/available",
];

pub struct AuditMiddleware {
    log: AuditLog,
    forwarding: crate::audit_forwarding::Forwarder,
}

impl AuditMiddleware {
    pub fn new(log: AuditLog, forwarding: crate::audit_forwarding::Forwarder) -> Self {
        Self { log, forwarding }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuditMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuditMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuditMiddlewareService {
            service: Rc::new(service),
            log: self.log.clone(),
            forwarding: self.forwarding.clone(),
        }))
    }
}

pub struct AuditMiddlewareService<S> {
    service: Rc<S>,
    log: AuditLog,
    forwarding: crate::audit_forwarding::Forwarder,
}

impl<S, B> Service<ServiceRequest> for AuditMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = audit_path(req.path());
        let method = req.method().to_string();

        // Skip non-API and high-frequency polling
        if !path.starts_with("/api/") {
            let svc = self.service.clone();
            return Box::pin(async move { svc.call(req).await });
        }

        let ip = req
            .peer_addr()
            .map(|a| a.ip().to_string())
            .unwrap_or_default();
        let skip_successful_poll = method == "GET" && SKIP_PATHS.contains(&path.as_str());
        let log = self.log.clone();
        let forwarding = self.forwarding.clone();
        let svc = self.service.clone();

        Box::pin(async move {
            let mut res = svc.call(req).await?;
            let status = res.status().as_u16();
            if skip_successful_poll && status < 400 && crate::auth::token_audit_actor(res.request()).is_none() {
                return Ok(res);
            }
            let user = crate::auth::token_audit_actor(res.request()).or_else(|| crate::auth::extract_username(res.request()))
                .unwrap_or_else(|| "unauthenticated".into());

            let persisted = append_entry(
                &log,
                AuditEntry {
                    timestamp: chrono::Utc::now()
                        .format("%Y-%m-%d %H:%M:%S UTC")
                        .to_string(),
                    user,
                    method,
                    path,
                    status,
                    ip,
                    hash: None,
                },
            );

            if let Some(entry) = &persisted {
                if !forwarding.enqueue(entry) {
                    res.headers_mut().insert(
                        actix_web::http::header::HeaderName::from_static(
                            "x-nabiman-audit-forwarding",
                        ),
                        actix_web::http::header::HeaderValue::from_static("not-queued"),
                    );
                }
            }
            if persisted.is_none() {
                res.headers_mut().insert(
                    actix_web::http::header::HeaderName::from_static("x-nabiman-audit"),
                    actix_web::http::header::HeaderValue::from_static("unavailable"),
                );
            }
            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::audit_path;
    #[test]
    fn public_and_owner_share_paths_use_same_nonsecret_correlation() {
        let token = "secret-share-capability";
        let public = audit_path(&format!("/api/share/{token}/auth?ticket=secret-proof"));
        let owner = audit_path(&format!("/api/terminal/share-links/{token}"));
        assert!(!public.contains(token));
        assert!(!owner.contains(token));
        assert!(!public.contains("secret-proof"));
        assert!(public.starts_with("/api/share/[cap:"));
        assert!(public.ends_with("]/auth"));
        assert_eq!(
            public.split("[cap:").nth(1).unwrap().split(']').next(),
            owner.split("[cap:").nth(1).unwrap().split(']').next()
        );
        assert_ne!(
            audit_path("/api/share/another"),
            audit_path(&format!("/api/share/{token}"))
        );
    }
    #[test]
    fn unknown_share_suffixes_and_nonshare_queries_do_not_leak_credentials() {
        assert!(!audit_path("/api/share/secret/unknown/another-secret").contains("another-secret"));
        assert_eq!(audit_path("/api/terminal?ticket=secret"), "/api/terminal");
        assert_eq!(
            audit_path("/api/users/immutable-id"),
            "/api/users/immutable-id"
        );
    }
}
