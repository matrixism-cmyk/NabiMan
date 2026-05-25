use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::Method;
use std::collections::HashMap;
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::mec::audit_logger::AuditLogger;

const PREFIX: &str = "/api/mec/v1/";
const THROTTLE: Duration = Duration::from_secs(30);
/// High-frequency / self-referential resources never recorded as "view" events
/// (these are auto-polled by the dashboard itself).
const SKIP_RESOURCES: &[&str] = &["dashboard", "health", "jobs", "settings"];

pub type ViewThrottle = Arc<Mutex<HashMap<String, Instant>>>;

pub fn new_view_throttle() -> ViewThrottle {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Records authenticated MEC GET requests as "view" audit entries so the
/// dashboard activity feed reflects ongoing read usage (not just mutations).
/// Per-(user, resource) throttling keeps interval polling from flooding the log.
pub struct MecAccessAudit {
    audit: Arc<AuditLogger>,
    throttle: ViewThrottle,
}

impl MecAccessAudit {
    pub fn new(audit: Arc<AuditLogger>, throttle: ViewThrottle) -> Self {
        Self { audit, throttle }
    }
}

impl<S, B> Transform<S, ServiceRequest> for MecAccessAudit
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Transform = MecAccessAuditMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(MecAccessAuditMiddleware {
            service: Rc::new(service),
            audit: self.audit.clone(),
            throttle: self.throttle.clone(),
        }))
    }
}

pub struct MecAccessAuditMiddleware<S> {
    service: Rc<S>,
    audit: Arc<AuditLogger>,
    throttle: ViewThrottle,
}

impl<S, B> Service<ServiceRequest> for MecAccessAuditMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if req.method() == Method::GET {
            record_view(&req, &self.audit, &self.throttle);
        }
        let svc = self.service.clone();
        Box::pin(async move { svc.call(req).await })
    }
}

fn record_view(req: &ServiceRequest, audit: &AuditLogger, throttle: &ViewThrottle) {
    let rest = match req.path().strip_prefix(PREFIX) {
        Some(r) => r,
        None => return,
    };
    let mut segs = rest.split('/').filter(|s| !s.is_empty());
    let resource = match segs.next() {
        Some(r) => r.to_string(),
        None => return,
    };
    if SKIP_RESOURCES.iter().any(|r| *r == resource) {
        return;
    }
    let resource_id = segs.next().unwrap_or("-").to_string();

    // Only log authenticated views (skip anonymous/expired probes).
    let user = match extract_user(req) {
        Some(u) => u,
        None => return,
    };

    // Throttle per (user, resource) so interval polls don't flood the feed.
    let key = format!("{user}|{resource}");
    {
        let mut map = throttle.lock().unwrap();
        let now = Instant::now();
        if let Some(prev) = map.get(&key) {
            if now.duration_since(*prev) < THROTTLE {
                return;
            }
        }
        map.insert(key, now);
    }

    let ip = req
        .connection_info()
        .realip_remote_addr()
        .map(|s| s.to_string());
    let ua = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    audit
        .begin(&user, "view", &resource, &resource_id)
        .with_source_ip(ip)
        .with_user_agent(ua)
        .success(None);
}

fn extract_user(req: &ServiceRequest) -> Option<String> {
    let auth = req.headers().get("Authorization")?.to_str().ok()?;
    let token = auth.strip_prefix("Bearer ")?;
    crate::auth::decode_claims_unverified(token)
        .ok()
        .map(|d| d.claims.sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_polling_resources() {
        assert!(SKIP_RESOURCES.iter().any(|r| *r == "dashboard"));
        assert!(!SKIP_RESOURCES.iter().any(|r| *r == "nodes"));
    }
}
