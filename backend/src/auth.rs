#[path = "auth_api_tokens.rs"]
mod api_tokens;
pub(crate) use api_tokens::audit_actor as token_audit_actor;
#[path = "auth_bootstrap.rs"]
mod bootstrap;
pub(crate) use bootstrap::validate_bootstrap_password;
pub use bootstrap::{new_jwt_secret, new_password_store};

use crate::jwt_sessions::SessionStore;
use crate::models::ApiResponse;
use crate::users::UserStore;
use actix_web::dev::ServiceRequest;
use actix_web::{post, web, HttpMessage, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type PasswordStore = Arc<std::sync::Mutex<String>>; // legacy, kept for migration
pub type JwtSecret = Arc<String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,  // username
    pub role: String, // admin/operator/viewer
    pub exp: usize,
    pub iat: usize,
    pub sid: String,
}

#[derive(Deserialize)]
struct AccessEnvelope {
    #[serde(flatten)]
    claims: Claims,
    #[serde(default)]
    typ: Option<String>,
}

fn verify_jwt(secret: &str, token: &str) -> Option<Claims> {
    jsonwebtoken::decode::<AccessEnvelope>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    )
    .ok()
    .and_then(|d| match d.claims.typ.as_deref() {
        None | Some("access") => Some(d.claims.claims),
        _ => None,
    })
}

pub fn decode_claims_unverified(token: &str) -> Result<jsonwebtoken::TokenData<Claims>, String> {
    let mut validation = jsonwebtoken::Validation::default();
    validation.insecure_disable_signature_validation();
    jsonwebtoken::decode::<Claims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(b""),
        &validation,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
pub fn check_auth_token(secret: &str, token: &str, sessions: &SessionStore) -> bool {
    verify_jwt(secret, token).is_some_and(|claims| session_valid(&claims, sessions))
}

fn session_valid(claims: &Claims, sessions: &SessionStore) -> bool {
    sessions
        .lock()
        .unwrap()
        .get(&claims.sid)
        .is_some_and(|session| session.username == claims.sub)
}

// --- Handlers ---
#[post("/api/auth/login")]
pub async fn login(
    req: actix_web::HttpRequest,
    body: web::Json<crate::models::LoginRequest>,
    user_store: web::Data<UserStore>,
    secret: web::Data<JwtSecret>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let username = body.username.as_deref().unwrap_or("admin");
    let user = match crate::users::find_user_by_name(&user_store, username) {
        Some(u) => u,
        None => {
            return HttpResponse::Unauthorized()
                .json(ApiResponse::<()>::error("Invalid credentials"))
        }
    };
    match bcrypt::verify(&body.password, &user.password_hash) {
        Ok(true) => {
            if !user.active() {
                return HttpResponse::Forbidden().json(ApiResponse::<()>::error(
                    "가입 승인 대기, 반려 또는 이용 정지 상태입니다. 최고관리자에게 문의하세요.",
                ));
            }
            // Check if 2FA is enabled
            if user.totp_enabled {
                return HttpResponse::Ok().json(ApiResponse::ok(
                    serde_json::json!({ "requires_2fa": true, "username": user.username }),
                ));
            }
            crate::users::update_last_login(&user_store, username);
            let sid = crate::jwt_sessions::generate_session_id();
            let ip = req
                .peer_addr()
                .map(|a| a.ip().to_string())
                .unwrap_or_default();
            if !crate::jwt_sessions::register_authenticated_session(
                &sessions,
                &user_store,
                &sid,
                &user,
                false,
                &ip,
            ) {
                return HttpResponse::Unauthorized().json(ApiResponse::<()>::error(
                    "Account changed during authentication; sign in again",
                ));
            }
            match crate::jwt_sessions::create_token_pair(&secret, &user.username, user.role.as_str(), &sid) {
                Ok((access, refresh)) => HttpResponse::Ok().json(ApiResponse::ok(
                    serde_json::json!({ "token": access, "refresh_token": refresh, "username": user.username, "role": user.role })
                )),
                Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
            }
        }
        _ => HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Invalid credentials")),
    }
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[post("/api/auth/change-password")]
pub async fn change_password(
    req: actix_web::HttpRequest,
    body: web::Json<ChangePasswordRequest>,
    user_store: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let username = match extract_username(&req) {
        Some(username) => username,
        None => return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Unauthorized")),
    };
    let user = match crate::users::find_user_by_name(&user_store, &username) {
        Some(u) if Some(u.id.as_str()) == verified_user_id(&req).as_deref() => u,
        _ => return HttpResponse::Ok().json(ApiResponse::<()>::error("User not found")),
    };
    if !bcrypt::verify(&body.current_password, &user.password_hash).unwrap_or(false) {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Current password is incorrect"));
    }
    if body.new_password.len() < 8 {
        return HttpResponse::Ok().json(ApiResponse::<()>::error(
            "Password must be at least 8 characters",
        ));
    }
    let new_hash = match bcrypt::hash(&body.new_password, 10) {
        Ok(h) => h,
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<()>::error(&e.to_string())),
    };
    match crate::users::update_password(&user_store, &user.id, &user.password_hash, new_hash) {
        Ok(()) => {
            let current_sid = http_token(&req)
                .and_then(|token| decode_claims_unverified(token).ok())
                .map(|data| data.claims.sid);
            crate::jwt_sessions::revoke_user_sessions(&sessions, &username, current_sid.as_deref());
            HttpResponse::Ok().json(ApiResponse::ok("Password changed"))
        }
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

#[post("/api/auth/logout")]
pub async fn logout(
    req: actix_web::HttpRequest,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    if let Some(token) = http_token(&req) {
        if let Ok(data) = decode_claims_unverified(token) {
            sessions.lock().unwrap().remove(&data.claims.sid);
        }
    }
    HttpResponse::Ok().json(ApiResponse::ok("Logged out"))
}

fn http_token(req: &actix_web::HttpRequest) -> Option<&str> {
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| {
            req.headers()
                .get("X-Auth-Token")
                .and_then(|v| v.to_str().ok())
        })
}

/// Set only after signature, session and active-account checks. Permission
/// decisions remain in middleware; denied requests may retain a verified actor.
#[derive(Clone)]
struct VerifiedPrincipal {
    user_id: String,
    username: String,
    role: crate::models::UserRole,
    sid: String,
    expires: usize,
}
pub(crate) fn verified_user_id(req: &actix_web::HttpRequest) -> Option<String> {
    req.extensions()
        .get::<VerifiedPrincipal>()
        .map(|p| p.user_id.clone())
}

pub(crate) fn verified_claims(req: &actix_web::HttpRequest) -> Option<Claims> {
    req.extensions().get::<VerifiedPrincipal>().map(|p| Claims {
        sub: p.username.clone(),
        role: p.role.as_str().into(),
        sid: p.sid.clone(),
        exp: p.expires,
        iat: 0,
    })
}

pub fn extract_username(req: &actix_web::HttpRequest) -> Option<String> {
    req.extensions()
        .get::<VerifiedPrincipal>()
        .map(|p| p.username.clone())
}

#[cfg(test)]
pub(crate) fn verified_role(req: &actix_web::HttpRequest) -> Option<crate::models::UserRole> {
    req.extensions()
        .get::<VerifiedPrincipal>()
        .map(|p| p.role.clone())
}

// --- Middleware auth check (JWT + RBAC) ---
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuthDecision {
    Allow,
    Unauthorized,
    Forbidden,
    MfaRequired,
}

pub fn check_auth(
    req: &ServiceRequest,
    secret: &JwtSecret,
    sessions: &SessionStore,
    users: &UserStore,
) -> AuthDecision {
    req.extensions_mut().remove::<VerifiedPrincipal>();
    let path = req.path();
    // Share links carry their own secret and are checked in terminal::share.
    if path == "/api/auth/login"
        || path == "/api/auth/register"
        || path == "/api/auth/refresh"
        || path == "/api/auth/2fa/verify"
        || path == "/api/auth/oauth/callback"
        || path == "/api/auth/ldap/login"
        || (req.method() == actix_web::http::Method::GET
            && matches!(
                path,
                "/api/auth/oauth/authorize"
                    | "/api/auth/oauth/provider"
                    | "/api/auth/ldap/provider"
            ))
        || path.starts_with("/api/share/")
        || !path.starts_with("/api/")
    {
        return AuthDecision::Allow;
    }

    if let Some(decision) = api_tokens::check(req, users) { return decision; }
    // The terminal and its dictation are WebSockets: a browser cannot put the
    // account token in a header there, so a one-use ticket stands in.
    let ticketed = path == "/api/terminal" || path == "/api/terminal/dictation";
    let claims = if ticketed
        && req
            .query_string()
            .split('&')
            .any(|p| p.starts_with("ticket="))
    {
        let Some(store) = req.app_data::<web::Data<crate::terminal_tickets::TicketStore>>() else {
            return AuthDecision::Unauthorized;
        };
        match crate::terminal_tickets::redeem(req.request(), store) {
            Some(c) => c,
            None => return AuthDecision::Unauthorized,
        }
    } else if let Some(admitted) = crate::terminal::sftp::links::redeem(
        req.method().as_str(),
        path,
        req.query_string(),
    ) {
        // A file dragged onto the desktop is fetched by the browser itself,
        // without the Authorization header; a single-use link stands in.
        match admitted {
            Some(c) => c,
            None => return AuthDecision::Unauthorized,
        }
    } else {
        // Account JWTs must never be accepted from a terminal URL.
        if ticketed
            && req
                .query_string()
                .split('&')
                .any(|p| p.starts_with("token="))
        {
            return AuthDecision::Unauthorized;
        }
        let Some(token) = extract_token(req) else {
            return AuthDecision::Unauthorized;
        };
        match verify_jwt(secret, &token) {
            Some(c) => c,
            None => return AuthDecision::Unauthorized,
        }
    };
    if !session_valid(&claims, sessions) {
        return AuthDecision::Unauthorized;
    }
    let user = match crate::users::find_user_by_name(users, &claims.sub) {
        Some(user)
            if user.active()
                && sessions
                    .lock()
                    .unwrap()
                    .get(&claims.sid)
                    .is_some_and(|s| !s.user_id.is_empty() && s.user_id == user.id) =>
        {
            user
        }
        _ => return AuthDecision::Unauthorized,
    };
    req.extensions_mut().insert(VerifiedPrincipal {
        user_id: user.id.clone(),
        username: user.username.clone(),
        role: user.role.clone(),
        sid: claims.sid.clone(),
        expires: claims.exp,
    });
    let verified = sessions
        .lock()
        .unwrap()
        .get(&claims.sid)
        .is_some_and(|s| s.mfa_verified);
    if !crate::mfa_policy::permitted(&user, verified, req.method().as_str(), path) {
        return AuthDecision::MfaRequired;
    }
    if crate::scope_access::permission(&user, req.method().as_str(), path) {
        AuthDecision::Allow
    } else {
        AuthDecision::Forbidden
    }
}

fn extract_token(req: &ServiceRequest) -> Option<String> {
    // Authorization: Bearer
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(tok) = val.strip_prefix("Bearer ") {
                return Some(tok.trim().to_string());
            }
        }
    }
    // Legacy X-Auth-Token
    req.headers()
        .get("X-Auth-Token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(login).service(logout).service(change_password);
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
