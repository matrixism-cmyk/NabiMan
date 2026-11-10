//! Headers every response carries: no framing by other sites, no sniffing,
//! a Content-Security-Policy fitted to the built SPA, and HSTS when the
//! request came over HTTPS.
//!
//! The policy (`policy`) allows what the SPA actually does, and nothing else:
//! - scripts only from this origin (no inline script in index.html); plus
//!   `'wasm-unsafe-eval'` for WebAssembly (the terminal's image addon decodes
//!   sixel with it, pdf.js its image codecs) and `blob:` for the dictation
//!   AudioWorklet, which is loaded from a blob URL;
//! - styles from this origin and inline (xterm.js and React insert `<style>`
//!   elements; the recovery page has an inline `<style>`);
//! - images, audio and video also as `data:`/`blob:` (2FA QR code, SFTP
//!   previews, speech playback); fonts also as `data:`;
//! - connections to this origin, its WebSocket (`ws:`/`wss:` on the same
//!   host, spelled out for browsers whose `'self'` does not cover them) and
//!   `blob:`; workers from this origin or `blob:` (pdf.js);
//! - frames from this origin, and from the feedback panel's origin when the
//!   install has a feedback widget (`branding`, `NABIMAN_FEEDBACK_ORIGIN`;
//!   none on a new install), plus its launcher's origin in `script-src` when
//!   that is not the bundled /fixhub-admin.js;
//! - `frame-ancestors 'self'`, `object-src 'none'`, `base-uri 'self'`,
//!   `form-action 'self'`.
//!
//! `NABIMAN_CSP=off` sends no policy; any other value replaces it verbatim.
//! `NABIMAN_HSTS=true` always sends HSTS, `false` never; unset, it is sent
//! when a trusted proxy (`NABIMAN_TRUSTED_PROXIES`) says the request came
//! over HTTPS (`X-Forwarded-Proto: https`).
use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error,
};
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::rc::Rc;

const HSTS: &str = "max-age=31536000; includeSubDomains";

/// A Host header fit to be written into the policy.
fn plain_host(host: &str) -> Option<&str> {
    (!host.is_empty() && host.len() <= 255 && host.chars().all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c)))
        .then_some(host)
}

/// The Content-Security-Policy for a request to `host`, or `None` when
/// switched off (`setting` is `NABIMAN_CSP`); `widget` is the feedback widget.
pub(crate) fn policy(setting: Option<&str>, host: Option<&str>, widget: Option<&crate::branding::FeedbackWidget>) -> Option<String> {
    match setting.map(str::trim) {
        Some(off) if off.eq_ignore_ascii_case("off") => return None,
        Some(custom) if !custom.is_empty() => return Some(custom.to_string()),
        _ => {}
    }
    let sockets = host.and_then(plain_host).map(|h| format!(" ws://{h} wss://{h}")).unwrap_or_default();
    let (frame, script) = crate::branding::csp_sources(widget);
    let frames = frame.map(|o| format!(" {o}")).unwrap_or_default();
    let scripts = script.map(|o| format!(" {o}")).unwrap_or_default();
    Some(format!(
        "default-src 'self'; script-src 'self'{scripts} 'wasm-unsafe-eval' blob:; style-src 'self' 'unsafe-inline'; \
         img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' data: blob:; \
         connect-src 'self'{sockets} blob:; worker-src 'self' blob:; frame-src 'self'{frames}; \
         frame-ancestors 'self'; object-src 'none'; base-uri 'self'; form-action 'self'"
    ))
}

/// Whether to send HSTS (`setting` is `NABIMAN_HSTS`).
pub(crate) fn hsts(setting: Option<&str>, over_https: bool) -> bool {
    match setting.map(str::trim) {
        Some("true") => true,
        Some("false") => false,
        _ => over_https,
    }
}

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

pub struct SecurityHeadersMiddleware<S> {
    service: Rc<S>,
}

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
        let host = req.headers().get("host").and_then(|h| h.to_str().ok()).map(str::to_owned);
        let csp = policy(std::env::var("NABIMAN_CSP").ok().as_deref(), host.as_deref(), crate::branding::feedback());
        // NabiMan speaks plain HTTP itself; HTTPS ends at the proxy, whose word
        // is taken only when it is trusted (actix's connection_info believes anyone).
        let https = crate::trusted_proxy::forwarded_https(
            req.peer_addr().map(|a| a.ip()),
            req.headers(),
            crate::trusted_proxy::trusted(),
        );
        let send_hsts = hsts(std::env::var("NABIMAN_HSTS").ok().as_deref(), https);
        let svc = self.service.clone();
        Box::pin(async move {
            let mut res = svc.call(req).await?;
            let headers = res.headers_mut();
            let mut set = |name: &'static str, value: &str| {
                if let Ok(value) = HeaderValue::from_str(value) {
                    headers.insert(HeaderName::from_static(name), value);
                }
            };
            // Same rule as frame-ancestors 'self', for browsers without CSP.
            set("x-frame-options", "SAMEORIGIN");
            set("x-content-type-options", "nosniff");
            set("x-xss-protection", "1; mode=block");
            set("referrer-policy", "strict-origin-when-cross-origin");
            set("permissions-policy", "camera=(), microphone=(self), geolocation=()");
            if let Some(csp) = &csp {
                set("content-security-policy", csp);
            }
            if send_hsts {
                set("strict-transport-security", HSTS);
            }
            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test as awtest, web, App, HttpResponse};

    #[test]
    fn the_policy_covers_what_the_spa_needs_and_closes_the_rest() {
        let p = policy(None, Some("nabiman.example:8443"), None).unwrap();
        for part in [
            "default-src 'self'",
            "script-src 'self' 'wasm-unsafe-eval' blob:",
            "connect-src 'self' ws://nabiman.example:8443 wss://nabiman.example:8443 blob:",
            "frame-src 'self';",
            "frame-ancestors 'self'",
            "object-src 'none'",
            "base-uri 'self'",
            "img-src 'self' data: blob:",
        ] {
            assert!(p.contains(part), "{part} in {p}");
        }
        assert!(!p.contains("'unsafe-eval'") && !p.contains("script-src 'self' 'unsafe-inline'"));
        // A Host header cannot write directives into the policy.
        let p = policy(None, Some("evil; script-src *"), None).unwrap();
        assert!(p.contains("connect-src 'self' blob:") && !p.contains("evil"));
        assert_eq!(policy(Some("off"), None, None), None);
        assert_eq!(policy(Some("default-src 'none'"), None, None).as_deref(), Some("default-src 'none'"));
    }

    #[test]
    fn a_feedback_widget_adds_only_its_own_origins() {
        use crate::branding::FeedbackWidget;
        let bundled = FeedbackWidget { script_url: "/fixhub-admin.js".into(), origin: "https://feedback.example".into(), site: "s".into() };
        let p = policy(None, Some("n.example"), Some(&bundled)).unwrap();
        assert!(p.contains("frame-src 'self' https://feedback.example;"), "{p}");
        assert!(p.contains("script-src 'self' 'wasm-unsafe-eval' blob:"), "{p}");
        let hosted = FeedbackWidget { script_url: "https://cdn.example/w.js".into(), ..bundled };
        let p = policy(None, Some("n.example"), Some(&hosted)).unwrap();
        assert!(p.contains("script-src 'self' https://cdn.example 'wasm-unsafe-eval' blob:"), "{p}");
        assert!(!p.contains("fix.bebo.kr"));
    }

    #[test]
    fn hsts_follows_https_unless_set() {
        assert!(hsts(None, true) && !hsts(None, false));
        assert!(hsts(Some("true"), false) && !hsts(Some("false"), true));
    }

    async fn headers(peer: &str, proto: Option<&str>) -> actix_web::http::header::HeaderMap {
        let app = awtest::init_service(App::new().wrap(SecurityHeaders).route("/", web::get().to(HttpResponse::Ok))).await;
        let mut req = awtest::TestRequest::get().uri("/").peer_addr(peer.parse().unwrap()).insert_header(("Host", "n.example"));
        if let Some(proto) = proto {
            req = req.insert_header(("X-Forwarded-Proto", proto));
        }
        awtest::call_service(&app, req.to_request()).await.headers().clone()
    }

    #[actix_web::test]
    async fn hsts_when_the_local_proxy_saw_https() {
        if std::env::var_os("NABIMAN_HSTS").is_some() || std::env::var_os("NABIMAN_CSP").is_some() {
            return; // the environment overrides what is tested here
        }
        let h = headers("127.0.0.1:4000", Some("https")).await;
        assert_eq!(h.get("strict-transport-security").unwrap(), HSTS);
        assert!(h.get("content-security-policy").unwrap().to_str().unwrap().contains("wss://n.example"));
        assert_eq!(h.get("x-frame-options").unwrap(), "SAMEORIGIN");
        assert!(headers("127.0.0.1:4000", Some("http")).await.get("strict-transport-security").is_none());
        assert!(headers("203.0.113.9:4000", Some("https")).await.get("strict-transport-security").is_none(), "only from a trusted proxy");
    }
}
