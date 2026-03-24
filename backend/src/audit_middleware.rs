use actix_web::{dev::{ServiceRequest, ServiceResponse, Transform, Service}, Error};
use std::future::{Ready, ready, Future};
use std::pin::Pin;
use std::rc::Rc;
use crate::audit::{AuditLog, AuditEntry, append_entry};

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
}

impl AuditMiddleware {
    pub fn new(log: AuditLog) -> Self { Self { log } }
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
        }))
    }
}

pub struct AuditMiddlewareService<S> {
    service: Rc<S>,
    log: AuditLog,
}

impl<S, B> Service<ServiceRequest> for AuditMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_string();
        let method = req.method().to_string();

        // Skip non-API and high-frequency polling
        if !path.starts_with("/api/") || (method == "GET" && SKIP_PATHS.contains(&path.as_str())) {
            let svc = self.service.clone();
            return Box::pin(async move { svc.call(req).await });
        }

        let ip = req.peer_addr().map(|a| a.ip().to_string()).unwrap_or_default();
        let user = extract_user(&req);
        let log = self.log.clone();
        let svc = self.service.clone();

        Box::pin(async move {
            let res = svc.call(req).await?;
            let status = res.status().as_u16();

            append_entry(&log, AuditEntry {
                timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                user, method, path, status, ip,
            });

            Ok(res)
        })
    }
}

fn extract_user(req: &ServiceRequest) -> String {
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                if let Ok(data) = crate::auth::decode_claims_unverified(token) {
                    return data.claims.sub;
                }
            }
        }
    }
    "-".to_string()
}
