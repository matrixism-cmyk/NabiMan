use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{http::Method, HttpResponse};
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::rc::Rc;

const PREFIX: &str = "/api/mec/";
const READ_ONLY_ALLOW_PATH_SUFFIXES: &[&str] = &[
    "/firewall/sync",         // 읽기 전용이지만 POST (running-config 재동기화)
    "/jobs/",                 // SSE stream은 GET이므로 자동 통과
];

pub fn is_read_only() -> bool {
    std::env::var("NABIMAN_MEC_READ_ONLY")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes"))
        .unwrap_or(false)
}

fn is_mutation(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn path_allowed(path: &str) -> bool {
    READ_ONLY_ALLOW_PATH_SUFFIXES
        .iter()
        .any(|suffix| path.ends_with(suffix))
}

pub struct MecReadOnly;

impl<S, B> Transform<S, ServiceRequest> for MecReadOnly
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Transform = MecReadOnlyMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(MecReadOnlyMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct MecReadOnlyMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for MecReadOnlyMiddleware<S>
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
        let blocked = is_read_only()
            && path.starts_with(PREFIX)
            && is_mutation(&method)
            && !path_allowed(&path);

        if blocked {
            return Box::pin(async move {
                let body = crate::models::mec::MecResponse::<serde_json::Value>::error(
                    crate::models::mec::MecError::new(
                        "MEC_READ_ONLY",
                        "NabiMan is running in MEC read-only mode. Set NABIMAN_MEC_READ_ONLY=false to allow mutations.",
                    ),
                );
                let response = HttpResponse::Forbidden().json(body);
                Ok(req.into_response(response).map_into_right_body())
            });
        }
        let svc = self.service.clone();
        Box::pin(async move { svc.call(req).await.map(|r| r.map_into_left_body()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_detected() {
        assert!(is_mutation(&Method::POST));
        assert!(is_mutation(&Method::DELETE));
        assert!(!is_mutation(&Method::GET));
    }

    #[test]
    fn allowed_suffix_bypasses_block() {
        assert!(path_allowed("/api/mec/v1/firewall/sync"));
        assert!(!path_allowed("/api/mec/v1/tenants/x"));
    }
}
