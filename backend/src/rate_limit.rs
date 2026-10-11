//! Process-wide bounded admission for password, MFA and registration entry points.
mod state;
#[cfg(test)]
mod tests;
use crate::models::ApiResponse;
use actix_web::{
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    web, Error, HttpMessage, HttpResponse,
};
use futures::StreamExt;
use state::{Permit, Shared};
use std::{
    future::{ready, Future, Ready},
    pin::Pin,
    rc::Rc,
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct RateLimiter {
    state: Shared,
}
impl RateLimiter {
    pub fn new() -> Self {
        Self {
            state: Shared::default(),
        }
    }
}
impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = Middleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, ()>>;
    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(Middleware {
            service: Rc::new(service),
            state: self.state.clone(),
        }))
    }
}
pub struct Middleware<S> {
    service: Rc<S>,
    state: Shared,
}
fn limited(seconds: u64) -> HttpResponse {
    let seconds = seconds.max(1);
    HttpResponse::TooManyRequests()
        .insert_header(("Retry-After", seconds.to_string()))
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::<()>::error(&format!(
            "Too many authentication attempts. Retry after {seconds} seconds."
        )))
}
fn protected(path: &str) -> bool {
    matches!(
        path,
        "/api/auth/register"
            | "/api/auth/login"
            | "/api/auth/ldap/login"
            | "/api/auth/change-password"
            | "/api/auth/2fa/verify"
            | "/api/auth/2fa/setup"
            | "/api/auth/2fa/verify-setup"
            | "/api/auth/2fa/disable"
            | "/api/auth/2fa/recovery-codes"
    )
}
impl<S, B> Service<ServiceRequest> for Middleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;
    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }
    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        if !protected(req.path()) || req.method() != "POST" {
            return Box::pin(
                async move { service.call(req).await.map(|r| r.map_into_left_body()) },
            );
        }
        // Never trust client-controlled Forwarded / X-Forwarded-For headers.
        let peer = req
            .peer_addr()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|| "unknown".into());
        let permit = Permit::begin(&self.state, &peer, Instant::now());
        Box::pin(async move {
            let mut permit = match permit {
                Ok(p) => p,
                Err(seconds) => {
                    return Ok(req.into_response(limited(seconds)).map_into_right_body())
                }
            };
            let mut payload = req.take_payload();
            let read = tokio::time::timeout(Duration::from_secs(3), async {
                let mut body = web::BytesMut::new();
                while let Some(chunk) = payload.next().await {
                    let chunk = chunk.map_err(|_| ())?;
                    if body.len() + chunk.len() > 32768 {
                        return Err(());
                    }
                    body.extend_from_slice(&chunk);
                }
                Ok::<_, ()>(body.freeze())
            })
            .await;
            let bytes =
                match read {
                    Ok(Ok(b)) => b,
                    _ => return Ok(req
                        .into_response(
                            HttpResponse::BadRequest()
                                .insert_header(("Cache-Control", "no-store"))
                                .json(ApiResponse::<()>::error(
                                    "Authentication request is too large, incomplete or unreadable",
                                )),
                        )
                        .map_into_right_body()),
                };
            let signup = req.path() == "/api/auth/register";
            let category;
            let identity;
            if signup {
                category = "registration-source";
                identity = peer;
            } else if let Some(id) = crate::auth::verified_user_id(req.request()) {
                category = "settings-user";
                identity = id;
            } else {
                category = "login-account";
                // Ambiguous duplicate fields are rejected by the same typed deserializer as handlers.
                #[derive(serde::Deserialize)]
                struct Identity {
                    username: Option<String>,
                }
                let parsed = serde_json::from_slice::<Identity>(&bytes);
                let name =
                    match parsed {
                        Ok(v) => v.username.unwrap_or_else(|| {
                            if req.path() == "/api/auth/login" {
                                "admin".into()
                            } else {
                                String::new()
                            }
                        }),
                        Err(_) => {
                            return Ok(req
                                .into_response(HttpResponse::BadRequest().json(
                                    ApiResponse::<()>::error("Invalid authentication request"),
                                ))
                                .map_into_right_body())
                        }
                    };
                if name.len() > 256 {
                    return Ok(req
                        .into_response(HttpResponse::BadRequest().finish())
                        .map_into_right_body());
                }
                identity = name.to_lowercase();
            }
            if let Err(seconds) = permit.account(category, &identity, Instant::now()) {
                return Ok(req.into_response(limited(seconds)).map_into_right_body());
            }
            req.set_payload(bytes.into());
            let result = service.call(req).await;
            if !signup && result.as_ref().is_ok_and(|r| r.status().is_success()) {
                permit.success();
            }
            result.map(|r| r.map_into_left_body())
        })
    }
}
