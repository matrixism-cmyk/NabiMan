//! The gate in front of every handler: which path is judged, and how a
//! refusal is answered.
//!
//! actix routes on its own re-quoted copy of the path (`%74` becomes `t`;
//! `%25`, `%2F` and `%2B` stay encoded), while `req.path()` is the raw URI.
//! A permission rule that read the raw path could deny `/api/terminal` yet
//! wave through `/api/%74erminal`, which the router sends to the same
//! handler. So authorization reads the router's copy, and an API path that
//! has more than one spelling is refused outright: every other reader of
//! `req.path()` (rate limits, MEC guards, audit, stream grants) then sees the
//! same path the router matched.
use crate::auth::AuthDecision;
use actix_web::{dev::ServiceRequest, HttpResponse};

/// The path the router will match, if the request may be judged on it.
/// `Err` for an API request spelled in a non-canonical way.
pub(crate) fn routed_path(req: &ServiceRequest) -> Result<&str, ()> {
    let routed = req.match_info().as_str();
    if api_bound(routed) && !canonical(req.path()) {
        return Err(());
    }
    Ok(routed)
}

/// Whether the router could take this path to an API handler: its first
/// real segment is `api`, in any case and behind any empty or dot segments.
fn api_bound(path: &str) -> bool {
    path.split('/')
        .find(|s| !matches!(*s, "" | "." | ".."))
        .is_some_and(|s| s.eq_ignore_ascii_case("api"))
}

/// One spelling only: lower-case `/api` prefix, no empty or dot segments
/// (a single trailing `/` is kept), no malformed escapes, and no escapes of
/// characters that never need one (RFC 3986 unreserved) or that would hand
/// a handler a path separator.
fn canonical(raw: &str) -> bool {
    if raw != "/api" && !raw.starts_with("/api/") {
        return false;
    }
    let body = raw.strip_suffix('/').unwrap_or(raw);
    if body.split('/').skip(1).any(|s| matches!(s, "" | "." | "..")) {
        return false;
    }
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok());
            let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) else {
                return false;
            };
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b'\\') {
                return false;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    true
}

/// The answer for a refused request; `None` lets it through.
pub(crate) fn denial(decision: AuthDecision) -> Option<HttpResponse> {
    use crate::models::ApiResponse;
    match decision {
        AuthDecision::Allow => None,
        AuthDecision::BadRequest => Some(
            HttpResponse::BadRequest().json(ApiResponse::<()>::error("Non-canonical request path")),
        ),
        AuthDecision::Unauthorized => {
            Some(HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Unauthorized")))
        }
        AuthDecision::MfaRequired => Some(
            HttpResponse::Forbidden()
                .insert_header(("X-Nabiman-MFA-Required", "1"))
                .json(ApiResponse::<()>::error(
                    "소속 보안 정책에 따라 2단계 인증 등록 후 다시 로그인해야 합니다",
                )),
        ),
        AuthDecision::Forbidden => {
            Some(HttpResponse::Forbidden().json(ApiResponse::<()>::error("Forbidden")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_spelling_per_api_path() {
        for ok in ["/api", "/api/", "/api/terminal", "/api/users/", "/api/x%20y", "/api/%EA%B0%80", "/api/a%25b", "/api/a%2Bb", "/api/a%3Ab"] {
            assert!(canonical(ok), "{ok}");
        }
        for bad in [
            "/api/%74erminal", "/%61pi/x", "/api/a%2Fb", "/api/a%2fb", "/api/a%5Cb", "/api/a%7Eb", "/api/a%2Eb",
            "/api//x", "//api/x", "/api/./x", "/api/a/../x", "/api/x/.", "/API/x", "/api/x%", "/api/x%G1", "/api/x%4",
        ] {
            assert!(!canonical(bad), "{bad}");
        }
    }

    #[test]
    fn only_paths_the_router_could_send_to_the_api_are_judged() {
        for api in ["/api", "/api/x", "//api/x", "/./api/x", "/../API/x"] {
            assert!(api_bound(api), "{api}");
        }
        for other in ["/", "/index.html", "/apiary", "/static/api/x", "/assets/app.js"] {
            assert!(!api_bound(other), "{other}");
        }
    }
}
