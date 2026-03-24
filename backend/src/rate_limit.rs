use actix_web::{dev::{ServiceRequest, ServiceResponse, Transform, Service}, Error, HttpResponse};
use actix_web::body::EitherBody;
use std::collections::HashMap;
use std::future::{Ready, ready, Future};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use crate::models::ApiResponse;

const MAX_ATTEMPTS: usize = 5;
const WINDOW_SECS: u64 = 300; // 5 minutes

type AttemptMap = Arc<Mutex<HashMap<String, Vec<Instant>>>>;

pub struct RateLimiter { attempts: AttemptMap }

impl RateLimiter {
    pub fn new() -> Self {
        Self { attempts: Arc::new(Mutex::new(HashMap::new())) }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = RateLimiterMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimiterMiddleware {
            service: Rc::new(service),
            attempts: self.attempts.clone(),
        }))
    }
}

pub struct RateLimiterMiddleware<S> {
    service: Rc<S>,
    attempts: AttemptMap,
}

impl<S, B> Service<ServiceRequest> for RateLimiterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Only rate-limit POST /api/auth/login
        if req.path() != "/api/auth/login" || req.method() != "POST" {
            let svc = self.service.clone();
            return Box::pin(async move {
                svc.call(req).await.map(|r| r.map_into_left_body())
            });
        }

        let ip = req.peer_addr()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|| "unknown".into());

        let now = Instant::now();
        let cutoff = now - std::time::Duration::from_secs(WINDOW_SECS);
        let mut map = self.attempts.lock().unwrap();

        let entry = map.entry(ip.clone()).or_default();
        entry.retain(|t| *t > cutoff);

        if entry.len() >= MAX_ATTEMPTS {
            let remaining = entry.first()
                .map(|t| WINDOW_SECS.saturating_sub(t.elapsed().as_secs()))
                .unwrap_or(WINDOW_SECS);
            drop(map);
            return Box::pin(async move {
                let resp = HttpResponse::TooManyRequests()
                    .json(ApiResponse::<()>::error(
                        &format!("Too many login attempts. Try again in {} seconds", remaining)
                    ));
                Ok(req.into_response(resp).map_into_right_body())
            });
        }

        entry.push(now);
        drop(map);

        let svc = self.service.clone();
        Box::pin(async move {
            svc.call(req).await.map(|r| r.map_into_left_body())
        })
    }
}
