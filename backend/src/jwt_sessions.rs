use crate::auth::JwtSecret;
use crate::models::ApiResponse;
use crate::users::UserStore;
use actix_web::{post, web, HttpResponse};
use serde::{Deserialize, Serialize};

const ACCESS_TTL: usize = 900; // 15 minutes
const REFRESH_TTL: usize = 604800; // 7 days

#[derive(Serialize, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub username: String,
    pub user_id: String,
    pub created_at: String,
    pub last_active: String,
    pub ip: String,
    #[serde(skip_serializing)]
    pub(crate) refresh_generation: u64,
    #[serde(skip_serializing)]
    pub(crate) mfa_verified: bool,
    /// When the person last did something (unix seconds), for signing out when idle.
    #[serde(skip_serializing)]
    pub(crate) last_activity: i64,
}

#[path = "session_store.rs"]
mod store;
#[path = "app_settings.rs"]
pub mod app_settings;
pub use store::SessionStore;

/// Sign-ins in memory only (tests and fixtures).
#[cfg_attr(not(test), allow(dead_code))]
pub fn new_session_store() -> SessionStore {
    SessionStore::in_memory()
}

/// The server's sign-ins, kept in the data directory across restarts.
pub fn open_session_store() -> SessionStore {
    let dir = std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into());
    SessionStore::open(std::path::PathBuf::from(dir).join("login_sessions.json"))
}

#[derive(Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: String,
    pub role: String,
    pub sid: String, // session id
    pub exp: usize,
    pub iat: usize,
    pub typ: String, // "refresh"
    pub generation: u64,
}

pub fn create_token_pair(
    secret: &str,
    username: &str,
    role: &str,
    session_id: &str,
) -> Result<(String, String), String> {
    let expires = chrono::Utc::now().timestamp() as usize + REFRESH_TTL;
    token_pair(secret, username, role, session_id, 0, expires)
}

fn token_pair(
    secret: &str,
    username: &str,
    role: &str,
    session_id: &str,
    generation: u64,
    refresh_expires: usize,
) -> Result<(String, String), String> {
    let now = chrono::Utc::now().timestamp() as usize;

    let access = crate::auth::Claims {
        sub: username.into(),
        role: role.into(),
        iat: now,
        exp: now + ACCESS_TTL,
        sid: session_id.into(),
    };
    let access_token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &access,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())?;

    let refresh = RefreshClaims {
        sub: username.into(),
        role: role.into(),
        sid: session_id.into(),
        iat: now,
        exp: refresh_expires,
        typ: "refresh".into(),
        generation,
    };
    let refresh_tok = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &refresh,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())?;

    Ok((access_token, refresh_tok))
}

pub fn generate_session_id() -> String {
    use rand::Rng;
    let id: u128 = rand::thread_rng().gen();
    format!("{:032x}", id)
}

pub fn register_session(store: &SessionStore, sid: &str, username: &str, user_id: &str, ip: &str) {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    store.lock().unwrap().insert(
        sid.to_string(),
        SessionInfo {
            session_id: sid.to_string(),
            username: username.to_string(),
            user_id: user_id.to_string(),
            created_at: now.clone(),
            last_active: now,
            ip: ip.to_string(),
            refresh_generation: 0,
            mfa_verified: false,
            last_activity: app_settings::now(),
        },
    );
}

/// Hold the account lock through registration: reset/delete/review either runs
/// before this check and rejects stale authentication, or afterwards and revokes it.
pub fn register_authenticated_session(
    store: &SessionStore,
    users: &UserStore,
    sid: &str,
    authenticated: &crate::models::NabimanUser,
    mfa_verified: bool,
    ip: &str,
) -> bool {
    let users = users.lock().unwrap();
    let Some(current) = users
        .iter()
        .find(|u| u.id == authenticated.id && u.username == authenticated.username)
    else {
        return false;
    };
    if !current.active()
        || current.password_hash != authenticated.password_hash
        || current.totp_enabled != authenticated.totp_enabled
        || current.totp_secret != authenticated.totp_secret
        || current.role != authenticated.role
        || current.share_authorization_version != authenticated.share_authorization_version
        || current.memberships != authenticated.memberships
        || current.scope_restricted != authenticated.scope_restricted
    {
        return false;
    }
    register_session(store, sid, &current.username, &current.id, ip);
    if let Some(session) = store.lock().unwrap().get_mut(sid) {
        session.mfa_verified = mfa_verified && current.totp_enabled;
    }
    true
}

pub fn revoke_user_sessions(store: &SessionStore, username: &str, keep_sid: Option<&str>) {
    store
        .lock()
        .unwrap()
        .retain(|sid, session| session.username != username || keep_sid == Some(sid.as_str()));
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[post("/api/auth/refresh")]
async fn refresh_token(
    body: web::Json<RefreshRequest>,
    secret: web::Data<JwtSecret>,
    sessions: web::Data<SessionStore>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    let claims = match jsonwebtoken::decode::<RefreshClaims>(
        &body.refresh_token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_ref().as_bytes()),
        &jsonwebtoken::Validation::default(),
    ) {
        Ok(d) => d.claims,
        Err(_) => {
            return HttpResponse::Unauthorized()
                .json(ApiResponse::<()>::error("Invalid refresh token"))
        }
    };

    if claims.typ != "refresh" {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Not a refresh token"));
    }

    // Account -> session is the same lock order used by review and login.
    // Keep both through consume+mint+commit so only one request can rotate.
    let users = users.lock().unwrap();
    let mut sessions = sessions.lock().unwrap();
    let Some(session) = sessions.get(&claims.sid) else {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Session revoked"));
    };
    let Some(user) = users.iter().find(|u| {
        u.active()
            && u.username == claims.sub
            && u.id == session.user_id
            && !u.id.is_empty()
            && session.username == claims.sub
    }) else {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Account unavailable"));
    };
    let now = chrono::Utc::now().timestamp().max(0) as usize;
    if claims.exp <= now || claims.iat > now || session.refresh_generation != claims.generation
        || app_settings::idle_expired(session.last_activity, now as i64)
    {
        sessions.remove(&claims.sid);
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error(
            "Login expired or refresh token reused; sign in again",
        ));
    }
    let Some(next) = claims.generation.checked_add(1) else {
        sessions.remove(&claims.sid);
        return HttpResponse::Unauthorized().finish();
    };
    match token_pair(
        &secret,
        &user.username,
        user.role.as_str(),
        &claims.sid,
        next,
        claims.exp,
    ) {
        Ok((token, refresh_token)) => {
            let session = sessions.get_mut(&claims.sid).unwrap();
            session.refresh_generation = next;
            session.last_active = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            HttpResponse::Ok()
                .insert_header(("Cache-Control", "no-store"))
                .json(ApiResponse::ok(
                    serde_json::json!({ "token": token, "refresh_token": refresh_token }),
                ))
        }
        Err(_) => HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error("Token issuance failed")),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    app_settings::config(cfg);
    cfg.service(refresh_token)
        .configure(crate::session_management::config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_change_can_keep_only_current_session() {
        let store = new_session_store();
        register_session(&store, "current", "alice", "alice", "127.0.0.1");
        register_session(&store, "other", "alice", "alice", "127.0.0.2");
        register_session(&store, "third", "bob", "bob", "127.0.0.3");
        revoke_user_sessions(&store, "alice", Some("current"));
        let sessions = store.lock().unwrap();
        assert!(sessions.contains_key("current"));
        assert!(!sessions.contains_key("other"));
        assert!(sessions.contains_key("third"));
    }
}

#[cfg(test)]
#[path = "refresh_rotation_tests.rs"]
mod rotation_tests;
