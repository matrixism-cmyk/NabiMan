use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::Method;
use actix_web::HttpResponse;
use std::collections::HashMap;
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PREFIX: &str = "/api/mec/";
const WINDOW: Duration = Duration::from_secs(60);
const MAX_MUTATIONS_PER_WINDOW: usize = 30;
const MAX_DELETES_PER_WINDOW: usize = 10;

type Bucket = Arc<Mutex<HashMap<String, Vec<(Instant, bool)>>>>;

pub struct MecRateLimit {
    bucket: Bucket,
}

impl Default for MecRateLimit {
    fn default() -> Self {
        Self::new()
    }
}

impl MecRateLimit {
    pub fn new() -> Self {
        Self {
            bucket: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for MecRateLimit
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Transform = MecRateLimitMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(MecRateLimitMiddleware {
            service: Rc::new(service),
            bucket: self.bucket.clone(),
        }))
    }
}

pub struct MecRateLimitMiddleware<S> {
    service: Rc<S>,
    bucket: Bucket,
}

impl<S, B> Service<ServiceRequest> for MecRateLimitMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_string();
        let method = req.method().clone();
        if !path.starts_with(PREFIX) || !is_mutation(&method) {
            let svc = self.service.clone();
            return Box::pin(async move { svc.call(req).await.map(|r| r.map_into_left_body()) });
        }

        let user_key = user_key(&req);
        let now = Instant::now();
        let is_delete = method == Method::DELETE;
        let (mutation_count, delete_count) = tally_and_record(&self.bucket, &user_key, now, is_delete);

        if mutation_count > MAX_MUTATIONS_PER_WINDOW
            || (is_delete && delete_count > MAX_DELETES_PER_WINDOW)
        {
            return Box::pin(async move {
                let body = crate::models::mec::MecResponse::<serde_json::Value>::error(
                    crate::models::mec::MecError::new(
                        "MEC_RATE_LIMIT",
                        &format!(
                            "Too many mutations in the last minute ({}/{}, deletes {}/{}). Slow down to protect live tenants.",
                            mutation_count, MAX_MUTATIONS_PER_WINDOW, delete_count, MAX_DELETES_PER_WINDOW
                        ),
                    ),
                );
                let response = HttpResponse::TooManyRequests()
                    .insert_header(("Retry-After", "60"))
                    .json(body);
                Ok(req.into_response(response).map_into_right_body())
            });
        }

        let svc = self.service.clone();
        Box::pin(async move { svc.call(req).await.map(|r| r.map_into_left_body()) })
    }
}

fn is_mutation(method: &Method) -> bool {
    matches!(*method, Method::POST | Method::PUT | Method::PATCH | Method::DELETE)
}

fn user_key(req: &ServiceRequest) -> String {
    let user = req
        .headers()
        .get("X-User")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("")
        .to_string();
    format!("{}|{}", user, ip)
}

fn tally_and_record(
    bucket: &Bucket,
    key: &str,
    now: Instant,
    is_delete: bool,
) -> (usize, usize) {
    let mut map = bucket.lock().unwrap();
    let entries = map.entry(key.to_string()).or_default();
    entries.retain(|(t, _)| now.duration_since(*t) < WINDOW);
    entries.push((now, is_delete));
    let mutations = entries.len();
    let deletes = entries.iter().filter(|(_, d)| *d).count();
    (mutations, deletes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_within_window() {
        let b: Bucket = Arc::new(Mutex::new(HashMap::new()));
        let now = Instant::now();
        for _ in 0..5 {
            let _ = tally_and_record(&b, "u|1", now, false);
        }
        let (m, d) = tally_and_record(&b, "u|1", now, true);
        assert_eq!(m, 6);
        assert_eq!(d, 1);
    }

    #[test]
    fn expires_after_window() {
        let b: Bucket = Arc::new(Mutex::new(HashMap::new()));
        let past = Instant::now() - Duration::from_secs(120);
        tally_and_record(&b, "u", past, false);
        let (m, _) = tally_and_record(&b, "u", Instant::now(), false);
        assert_eq!(m, 1);
    }

    #[test]
    fn method_classifier() {
        assert!(is_mutation(&Method::DELETE));
        assert!(is_mutation(&Method::POST));
        assert!(!is_mutation(&Method::GET));
    }
}
