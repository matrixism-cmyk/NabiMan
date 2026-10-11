use crate::models::{ApiResponse, NabimanUser};
use crate::users::UserStore;
use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sha2::{Digest, Sha256};
#[path = "mfa_settings.rs"]
mod settings;

fn generate_secret() -> String {
    use rand::Rng;
    let bytes: Vec<u8> = (0..20).map(|_| rand::thread_rng().gen()).collect();
    base32::encode(base32::Alphabet::Rfc4648 { padding: false }, &bytes)
}
fn make_totp(secret: &str) -> Option<totp_rs::TOTP> {
    let decoded = base32::decode(base32::Alphabet::Rfc4648 { padding: false }, secret)?;
    totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        decoded,
        Some("NabiMan".into()),
        String::new(),
    )
    .ok()
}
fn recovery_hash(code: &str) -> String {
    let normalized = code.trim().replace('-', "").to_ascii_lowercase();
    hex::encode(Sha256::digest(format!("nabiman-mfa-recovery:{normalized}")))
}
fn recovery_codes() -> (Vec<String>, Vec<String>) {
    use rand::RngCore;
    let codes: Vec<_> = (0..10)
        .map(|_| {
            let mut bytes = [0u8; 16];
            rand::thread_rng().fill_bytes(&mut bytes);
            let value = hex::encode(bytes);
            format!(
                "{}-{}-{}-{}",
                &value[..8],
                &value[8..16],
                &value[16..24],
                &value[24..]
            )
        })
        .collect();
    let hashes = codes.iter().map(|code| recovery_hash(code)).collect();
    (codes, hashes)
}
fn consume_totp(user: &mut NabimanUser, code: &str, now: u64) -> bool {
    if code.len() != 6 || !code.bytes().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let Some(mut totp) = user.totp_secret.as_deref().and_then(make_totp) else {
        return false;
    };
    totp.skew = 0;
    let step = now / 30;
    for candidate in [step, step.saturating_sub(1), step + 1] {
        if user
            .mfa
            .last_used_step
            .is_some_and(|used| candidate <= used)
        {
            continue;
        }
        if totp.check(code, candidate * 30) {
            user.mfa.last_used_step = Some(candidate);
            return true;
        }
    }
    false
}
fn consume_factor(user: &mut NabimanUser, code: &str, now: u64) -> bool {
    if consume_totp(user, code, now) {
        return true;
    }
    let digest = recovery_hash(code);
    if let Some(index) = user
        .mfa
        .recovery_hashes
        .iter()
        .position(|hash| *hash == digest)
    {
        user.mfa.recovery_hashes.remove(index);
        return true;
    }
    false
}
#[derive(Deserialize)]
pub struct VerifyLoginRequest {
    pub username: String,
    pub password: String,
    pub code: String,
}
#[post("/api/auth/2fa/verify")]
async fn verify_login(
    body: web::Json<VerifyLoginRequest>,
    user_store: web::Data<UserStore>,
    secret: web::Data<crate::auth::JwtSecret>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    req: HttpRequest,
) -> HttpResponse {
    let invalid =
        || HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Invalid credentials"));
    let Some(snapshot) = crate::users::find_user_by_name(&user_store, &body.username) else {
        return invalid();
    };
    if !snapshot.active()
        || !snapshot.totp_enabled
        || !bcrypt::verify(&body.password, &snapshot.password_hash).unwrap_or(false)
    {
        return invalid();
    }
    // Consume the factor durably before any session or JWT is issued.
    let user = match crate::users::mutate_user(&user_store, &snapshot.id, |user| {
        if !user.active()
            || !user.totp_enabled
            || user.password_hash != snapshot.password_hash
            || user.totp_secret != snapshot.totp_secret
            || !consume_factor(
                user,
                &body.code,
                chrono::Utc::now().timestamp().max(0) as u64,
            )
        {
            return Err("Invalid credentials".into());
        }
        Ok(user.clone())
    }) {
        Ok(user) => user,
        Err(error) if error == "Invalid credentials" => return invalid(),
        Err(_) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Authentication state could not be saved",
            ))
        }
    };
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
        true,
        &ip,
    ) {
        return invalid();
    }
    match crate::jwt_sessions::create_token_pair(&secret,&user.username,user.role.as_str(),&sid) {
        Ok((access,refresh))=>HttpResponse::Ok().insert_header(("Cache-Control","no-store"))
            .json(ApiResponse::ok(serde_json::json!({"token":access,"refresh_token":refresh,"username":user.username,"role":user.role}))),
        Err(_)=>HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Token issuance failed")),
    }
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(verify_login).configure(settings::config);
}
#[cfg(test)]
#[path = "totp_tests.rs"]
mod tests;
