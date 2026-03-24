use actix_web::{web, post, HttpResponse, HttpRequest};
use serde::Deserialize;
use crate::models::ApiResponse;
use crate::users::UserStore;

fn generate_secret() -> String {
    use rand::Rng;
    let bytes: Vec<u8> = (0..20).map(|_| rand::thread_rng().gen()).collect();
    base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &bytes)
}

fn make_totp(secret: &str) -> Option<totp_rs::TOTP> {
    let decoded = base32::decode(base32::Alphabet::Rfc4648 { padding: false }, secret)?;
    totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1, 6, 1, 30, decoded,
        Some("NabiMan".into()), String::new(),
    ).ok()
}

fn verify_code(secret: &str, code: &str) -> bool {
    make_totp(secret).map(|t| t.check_current(code).unwrap_or(false)).unwrap_or(false)
}

fn make_qr_uri(secret: &str, username: &str) -> String {
    make_totp(secret).map(|mut t| { t.account_name = username.to_string(); t.get_url() }).unwrap_or_default()
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

#[post("/api/auth/2fa/setup")]
async fn setup_2fa(req: HttpRequest, user_store: web::Data<UserStore>) -> HttpResponse {
    let username = extract_username(&req);
    let secret = generate_secret();
    let uri = make_qr_uri(&secret, &username);

    // Store secret temporarily (not enabled yet)
    let mut users = user_store.lock().unwrap();
    if let Some(u) = users.iter_mut().find(|u| u.username == username) {
        u.totp_secret = Some(secret.clone());
    }

    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "secret": secret,
        "uri": uri,
    })))
}

#[derive(Deserialize)]
pub struct VerifySetupRequest { pub code: String }

#[post("/api/auth/2fa/verify-setup")]
async fn verify_setup(
    req: HttpRequest, body: web::Json<VerifySetupRequest>, user_store: web::Data<UserStore>,
) -> HttpResponse {
    let username = extract_username(&req);
    let mut users = user_store.lock().unwrap();
    let user = match users.iter_mut().find(|u| u.username == username) {
        Some(u) => u,
        None => return HttpResponse::Ok().json(ApiResponse::<()>::error("User not found")),
    };
    let secret = match &user.totp_secret {
        Some(s) => s.clone(),
        None => return HttpResponse::Ok().json(ApiResponse::<()>::error("2FA not set up")),
    };
    if verify_code(&secret, &body.code) {
        user.totp_enabled = true;
        HttpResponse::Ok().json(ApiResponse::ok("2FA enabled"))
    } else {
        HttpResponse::Ok().json(ApiResponse::<()>::error("Invalid code"))
    }
}

#[derive(Deserialize)]
pub struct DisableRequest { pub password: String }

#[post("/api/auth/2fa/disable")]
async fn disable_2fa(
    req: HttpRequest, body: web::Json<DisableRequest>, user_store: web::Data<UserStore>,
) -> HttpResponse {
    let username = extract_username(&req);
    let mut users = user_store.lock().unwrap();
    let user = match users.iter_mut().find(|u| u.username == username) {
        Some(u) => u,
        None => return HttpResponse::Ok().json(ApiResponse::<()>::error("User not found")),
    };
    if !bcrypt::verify(&body.password, &user.password_hash).unwrap_or(false) {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Invalid password"));
    }
    user.totp_enabled = false;
    user.totp_secret = None;
    HttpResponse::Ok().json(ApiResponse::ok("2FA disabled"))
}

#[derive(Deserialize)]
pub struct VerifyLoginRequest { pub username: String, pub code: String }

#[post("/api/auth/2fa/verify")]
async fn verify_login(
    body: web::Json<VerifyLoginRequest>, user_store: web::Data<UserStore>,
    secret: web::Data<crate::auth::JwtSecret>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    req: HttpRequest,
) -> HttpResponse {
    let user = match crate::users::find_user_by_name(&user_store, &body.username) {
        Some(u) => u,
        None => return HttpResponse::Ok().json(ApiResponse::<()>::error("User not found")),
    };
    let totp_secret = match &user.totp_secret {
        Some(s) => s.clone(),
        None => return HttpResponse::Ok().json(ApiResponse::<()>::error("2FA not configured")),
    };
    if !verify_code(&totp_secret, &body.code) {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("Invalid 2FA code"));
    }
    let sid = crate::jwt_sessions::generate_session_id();
    let ip = req.peer_addr().map(|a| a.ip().to_string()).unwrap_or_default();
    crate::jwt_sessions::register_session(&sessions, &sid, &user.username, &ip);
    match crate::jwt_sessions::create_token_pair(&secret, &user.username, user.role.as_str(), &sid) {
        Ok((access, refresh)) => HttpResponse::Ok().json(ApiResponse::ok(
            serde_json::json!({ "token": access, "refresh_token": refresh, "username": user.username, "role": user.role })
        )),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(setup_2fa).service(verify_setup).service(disable_2fa).service(verify_login);
}
