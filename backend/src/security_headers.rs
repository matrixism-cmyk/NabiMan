use actix_web::{dev::{ServiceRequest, ServiceResponse, Transform, Service}, Error};
use std::future::{Ready, ready, Future};
use std::pin::Pin;
use std::rc::Rc;

pub struct SecurityHeaders;

impl<S, B> Transform<S, ServiceRequest> for SecurityHeaders
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = SecurityHeadersMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SecurityHeadersMiddleware { service: Rc::new(service) }))
    }
}

pub struct SecurityHeadersMiddleware<S> { service: Rc<S> }

impl<S, B> Service<ServiceRequest> for SecurityHeadersMiddleware<S>
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
        let svc = self.service.clone();
        Box::pin(async move {
            let mut res = svc.call(req).await?;
            let headers = res.headers_mut();
            headers.insert(
                actix_web::http::header::X_FRAME_OPTIONS,
                "DENY".parse().unwrap(),
            );
            headers.insert(
                actix_web::http::header::X_CONTENT_TYPE_OPTIONS,
                "nosniff".parse().unwrap(),
            );
            headers.insert(
                "X-XSS-Protection".parse().unwrap(),
                "1; mode=block".parse().unwrap(),
            );
            headers.insert(
                "Referrer-Policy".parse().unwrap(),
                "strict-origin-when-cross-origin".parse().unwrap(),
            );
            headers.insert(
                "Permissions-Policy".parse().unwrap(),
                "camera=(), microphone=(), geolocation=()".parse().unwrap(),
            );
            // HSTS only if behind TLS proxy (controlled by env)
            if std::env::var("NABIMAN_HSTS").unwrap_or_default() == "true" {
                headers.insert(
                    actix_web::http::header::STRICT_TRANSPORT_SECURITY,
                    "max-age=31536000; includeSubDomains".parse().unwrap(),
                );
            }
            Ok(res)
        })
    }
}
