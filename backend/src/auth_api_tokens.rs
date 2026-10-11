use super::*;
use crate::api_keys::{ApiKeyStore, Rejection, TokenActor};
/// Reject mixed credentials rather than silently selecting a stronger identity.
pub(super) fn check(req: &ServiceRequest, users: &UserStore) -> Option<AuthDecision> {
    let bearer = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    let header = req.headers().get("X-API-Key");
    if header.is_none() && !bearer.is_some_and(|v| v.starts_with("nbm_")) {
        return None;
    }
    if (header.is_some() && req.headers().contains_key("Authorization"))
        || req.headers().contains_key("X-Auth-Token")
        || req.headers().get_all("X-API-Key").count() > 1
        || req.headers().get_all("Authorization").count() > 1
    {
        return Some(AuthDecision::Unauthorized);
    }
    let raw = header
        .and_then(|v| v.to_str().ok())
        .or(bearer)
        .unwrap_or("");
    let Some(store) = req.app_data::<web::Data<ApiKeyStore>>() else {
        return Some(AuthDecision::Unauthorized);
    };
    Some(
        match crate::api_keys::authenticate(raw, store, users, req.method().as_str(), req.path()) {
            Ok(actor) => {
                req.extensions_mut().insert(VerifiedPrincipal {
                    user_id: actor.user.id.clone(),
                    username: actor.user.username.clone(),
                    role: actor.user.role.clone(),
                    sid: format!("api-token:{}", actor.id),
                    expires: actor.expires,
                });
                req.extensions_mut().insert(actor);
                AuthDecision::Allow
            }
            Err(Rejection::Forbidden) => AuthDecision::Forbidden,
            Err(Rejection::Invalid | Rejection::Storage) => AuthDecision::Unauthorized,
        },
    )
}
pub(crate) fn audit_actor(req: &actix_web::HttpRequest) -> Option<String> {
    req.extensions()
        .get::<TokenActor>()
        .map(|a| format!("api-token:{};owner:{}", a.id, a.user.id))
}
