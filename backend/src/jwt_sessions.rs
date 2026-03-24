use actix_web::{web, post, get, HttpResponse, HttpRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::models::ApiResponse;
use crate::auth::JwtSecret;

const ACCESS_TTL: usize = 900;     // 15 minutes
const REFRESH_TTL: usize = 604800; // 7 days

#[derive(Serialize, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub username: String,
    pub created_at: String,
    pub last_active: String,
    pub ip: String,
}

pub type SessionStore = Arc<Mutex<HashMap<String, SessionInfo>>>;

pub fn new_session_store() -> SessionStore {
    Arc::new(Mutex::new(HashMap::new()))
}

#[derive(Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: String,
    pub role: String,
    pub sid: String, // session id
    pub exp: usize,
    pub iat: usize,
    pub typ: String, // "refresh"
}

pub fn create_token_pair(secret: &str, username: &str, role: &str, session_id: &str)
    -> Result<(String, String), String>
{
    let now = chrono::Utc::now().timestamp() as usize;

    let access = crate::auth::Claims {
        sub: username.into(), role: role.into(), iat: now, exp: now + ACCESS_TTL,
    };
    let access_token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &access,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    ).map_err(|e| e.to_string())?;

    let refresh = RefreshClaims {
        sub: username.into(), role: role.into(), sid: session_id.into(),
        iat: now, exp: now + REFRESH_TTL, typ: "refresh".into(),
    };
    let refresh_tok = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &refresh,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    ).map_err(|e| e.to_string())?;

    Ok((access_token, refresh_tok))
}

pub fn generate_session_id() -> String {
    use rand::Rng;
    let id: u128 = rand::thread_rng().gen();
    format!("{:032x}", id)
}

pub fn register_session(store: &SessionStore, sid: &str, username: &str, ip: &str) {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    store.lock().unwrap().insert(sid.to_string(), SessionInfo {
        session_id: sid.to_string(),
        username: username.to_string(),
        created_at: now.clone(),
        last_active: now,
        ip: ip.to_string(),
    });
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
) -> HttpResponse {
    let claims = match jsonwebtoken::decode::<RefreshClaims>(
        &body.refresh_token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_ref().as_bytes()),
        &jsonwebtoken::Validation::default(),
    ) {
        Ok(d) => d.claims,
        Err(_) => return HttpResponse::Unauthorized()
            .json(ApiResponse::<()>::error("Invalid refresh token")),
    };

    if claims.typ != "refresh" {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Not a refresh token"));
    }

    // Check session still exists
    let exists = sessions.lock().unwrap().contains_key(&claims.sid);
    if !exists {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Session revoked"));
    }

    // Update last_active
    if let Some(s) = sessions.lock().unwrap().get_mut(&claims.sid) {
        s.last_active = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    }

    // Issue new access token only
    let now = chrono::Utc::now().timestamp() as usize;
    let access = crate::auth::Claims {
        sub: claims.sub, role: claims.role, iat: now, exp: now + ACCESS_TTL,
    };
    match jsonwebtoken::encode(
        &jsonwebtoken::Header::default(), &access,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_ref().as_bytes()),
    ) {
        Ok(token) => HttpResponse::Ok().json(ApiResponse::ok(
            serde_json::json!({ "token": token })
        )),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e.to_string())),
    }
}

#[get("/api/auth/sessions")]
async fn list_sessions(sessions: web::Data<SessionStore>) -> HttpResponse {
    let map = sessions.lock().unwrap();
    let list: Vec<SessionInfo> = map.values().cloned().collect();
    HttpResponse::Ok().json(ApiResponse::ok(list))
}

#[derive(Deserialize)]
pub struct RevokeRequest {
    pub session_id: String,
}

#[post("/api/auth/sessions/revoke")]
async fn revoke_session(body: web::Json<RevokeRequest>, sessions: web::Data<SessionStore>) -> HttpResponse {
    let removed = sessions.lock().unwrap().remove(&body.session_id).is_some();
    if removed {
        HttpResponse::Ok().json(ApiResponse::ok("Session revoked"))
    } else {
        HttpResponse::Ok().json(ApiResponse::<()>::error("Session not found"))
    }
}

#[post("/api/auth/sessions/revoke-all")]
async fn revoke_all(req: HttpRequest, sessions: web::Data<SessionStore>) -> HttpResponse {
    let username = extract_username(&req);
    let mut map = sessions.lock().unwrap();
    map.retain(|_, s| s.username != username);
    HttpResponse::Ok().json(ApiResponse::ok("All sessions revoked"))
}

fn extract_username(req: &HttpRequest) -> String {
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                if let Ok(data) = crate::auth::decode_claims_unverified(token) {
                    return data.claims.sub;
                }
            }
        }
    }
    String::new()
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(refresh_token)
       .service(list_sessions)
       .service(revoke_session)
       .service(revoke_all);
}
