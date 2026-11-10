//! One `info` line per request (target `http`): method, the route pattern
//! that served it (`/api/users/{id}`, never the raw path with its ids, and
//! never the query string or body), status, duration and the signed-in
//! account's id when there is one. Probes log at `debug`.

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::rc::Rc;
use std::time::Instant;

/// The route label for a request: its pattern, or a fixed word.
pub(crate) fn route_label(pattern: Option<String>, path: &str) -> String {
    match pattern.filter(|p| !p.is_empty() && p != "/") {
        Some(p) => p,
        None if path.starts_with("/api/") || path == "/api" => "<unmatched>".into(),
        None => "<static>".into(),
    }
}

fn probe(route: &str) -> bool {
    matches!(route, "/healthz" | "/readyz" | "/metrics")
}

pub(crate) struct RequestLog;

impl<S, B> Transform<S, ServiceRequest> for RequestLog
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RequestLogService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestLogService { service: Rc::new(service) }))
    }
}

pub(crate) struct RequestLogService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RequestLogService<S>
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
        let started = Instant::now();
        let method = req.method().to_string();
        let path = req.path().to_string();
        let svc = self.service.clone();
        Box::pin(async move {
            let result = svc.call(req).await;
            let (status, route, user) = match &result {
                Ok(res) => (
                    res.status().as_u16(),
                    route_label(res.request().match_pattern(), &path),
                    crate::auth::verified_user_id(res.request()),
                ),
                Err(e) => (e.as_response_error().status_code().as_u16(), route_label(None, &path), None),
            };
            let ms = started.elapsed().as_millis();
            let user = user.unwrap_or_else(|| "-".into());
            if probe(&route) {
                log_debug!("http", "request", method = method, route = route, status = status, ms = ms, user = user);
            } else {
                log_info!("http", "request", method = method, route = route, status = status, ms = ms, user = user);
            }
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test as atest, web, App, HttpResponse};

    #[test]
    fn routes_are_labelled_by_pattern_or_a_fixed_word() {
        assert_eq!(route_label(Some("/api/users/{id}".into()), "/api/users/42"), "/api/users/{id}");
        assert_eq!(route_label(None, "/api/nothing/here"), "<unmatched>");
        assert_eq!(route_label(Some("".into()), "/assets/app.js"), "<static>");
        assert_eq!(route_label(None, "/index.html?token=x"), "<static>");
    }

    #[actix_web::test]
    async fn the_pattern_not_the_path_is_what_a_request_is_known_by() {
        let app = atest::init_service(App::new().wrap(RequestLog).route(
            "/api/things/{id}",
            web::get().to(|req: actix_web::HttpRequest| async move {
                HttpResponse::Ok().body(route_label(req.match_pattern(), req.path()))
            }),
        ))
        .await;
        let res = atest::call_service(&app, atest::TestRequest::get().uri("/api/things/secret-id?token=abc").to_request()).await;
        assert_eq!(res.status(), 200);
        assert_eq!(atest::read_body(res).await, "/api/things/{id}");
    }
}
