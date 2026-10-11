use super::*;
use crate::jwt_sessions::SessionStore;
use actix_web::get;
fn actor_id(req: &HttpRequest) -> String {
    crate::auth::verified_user_id(req).unwrap_or_default()
}
fn bad(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::<()>::error(message))
}
fn saved_error() -> HttpResponse {
    HttpResponse::InternalServerError()
        .json(ApiResponse::<()>::error("MFA state could not be saved"))
}
fn revoke_all(sessions: &SessionStore, id: &str) {
    sessions.lock().unwrap().retain(|_, s| s.user_id != id);
}
#[derive(Deserialize)]
struct Password {
    password: String,
}
#[derive(Deserialize)]
struct Factor {
    password: String,
    code: String,
}
#[derive(Deserialize)]
struct Confirm {
    code: String,
    recovery_saved: bool,
}
#[get("/api/auth/2fa/status")]
async fn status(
    req: HttpRequest,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let users = users.lock().unwrap();
    let id = actor_id(&req);
    let Some(user) = users.iter().find(|u| u.id == id && u.active()) else {
        return HttpResponse::Unauthorized().finish();
    };
    let required = match crate::mfa_policy::required(user) {
        Ok(required) => required,
        Err(_) => {
            return HttpResponse::ServiceUnavailable()
                .json(ApiResponse::<()>::error("MFA policy unavailable"))
        }
    };
    let verified = crate::auth::verified_claims(&req).is_some_and(|c| {
        sessions
            .lock()
            .unwrap()
            .get(&c.sid)
            .is_some_and(|s| s.user_id == user.id && s.mfa_verified)
    });
    HttpResponse::Ok().insert_header(("Cache-Control","no-store")).json(ApiResponse::ok(serde_json::json!({
        "required":required,"verified":verified,"blocked":required && !(user.totp_enabled && verified),
        "username":user.username,"enabled":user.totp_enabled,"recovery_codes_remaining":user.mfa.recovery_hashes.len(),
        "local_password_available":user.password_hash.starts_with("$2"),
        "pending":!user.totp_enabled && user.mfa.pending_expires_at.is_some_and(|expiry|expiry>chrono::Utc::now().timestamp())
    })))
}
#[post("/api/auth/2fa/setup")]
pub(super) async fn setup_2fa(
    req: HttpRequest,
    body: web::Json<Password>,
    user_store: web::Data<UserStore>,
) -> HttpResponse {
    let id = actor_id(&req);
    let secret = generate_secret();
    let (codes, hashes) = recovery_codes();
    let expires = chrono::Utc::now().timestamp() + 600;
    let result = crate::users::mutate_user(&user_store, &id, |user| {
        if !user.active() || !bcrypt::verify(&body.password, &user.password_hash).unwrap_or(false) {
            return Err("Invalid password".into());
        }
        if user.totp_enabled {
            return Err("MFA is already enabled".into());
        }
        let mut totp = make_totp(&secret).ok_or("Could not generate authenticator")?;
        totp.account_name = user.username.clone();
        let qr = totp
            .get_qr_base64()
            .map_err(|_| "Could not generate QR code")?;
        user.totp_secret = Some(secret.clone());
        user.mfa = crate::models::MfaState {
            recovery_hashes: hashes,
            last_used_step: None,
            pending_expires_at: Some(expires),
            revision: uuid::Uuid::new_v4().to_string(),
        };
        Ok(qr)
    });
    match result {
        Ok(qr)=>HttpResponse::Ok().insert_header(("Cache-Control","no-store")).json(ApiResponse::ok(serde_json::json!({"secret":secret,"qr_png_base64":qr,"recovery_codes":codes,"expires_at":expires}))),
        Err(e) if e=="Invalid password" || e=="MFA is already enabled"=>bad(&e),
        Err(_)=>saved_error(),
    }
}
#[post("/api/auth/2fa/verify-setup")]
async fn verify_setup(
    req: HttpRequest,
    body: web::Json<Confirm>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    if !body.recovery_saved {
        return bad("Save recovery codes before enabling MFA");
    }
    let id = actor_id(&req);
    let result = crate::users::mutate_user(&users, &id, |user| {
        let now = chrono::Utc::now().timestamp();
        if !user.active()
            || user.totp_enabled
            || !user
                .mfa
                .pending_expires_at
                .is_some_and(|expiry| expiry > now)
            || !consume_totp(user, &body.code, now.max(0) as u64)
        {
            return Err("Invalid code or expired enrollment".into());
        }
        user.totp_enabled = true;
        user.mfa.pending_expires_at = None;
        Ok(())
    });
    match result {
        Ok(()) => {
            revoke_all(&sessions, &id);
            HttpResponse::Ok().json(ApiResponse::ok("MFA enabled; sign in again"))
        }
        Err(e) if e == "Invalid code or expired enrollment" => bad(&e),
        Err(_) => saved_error(),
    }
}
#[post("/api/auth/2fa/disable")]
async fn disable_2fa(
    req: HttpRequest,
    body: web::Json<Factor>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let id = actor_id(&req);
    let result = crate::users::mutate_user(&users, &id, |user| {
        if !user.active()
            || !user.totp_enabled
            || !bcrypt::verify(&body.password, &user.password_hash).unwrap_or(false)
            || !consume_factor(
                user,
                &body.code,
                chrono::Utc::now().timestamp().max(0) as u64,
            )
        {
            return Err("Invalid credentials".into());
        }
        if crate::mfa_policy::required(user)? {
            return Err("소속 보안 정책에 따라 2단계 인증을 해제할 수 없습니다".into());
        }
        user.totp_enabled = false;
        user.totp_secret = None;
        user.mfa = Default::default();
        Ok(())
    });
    match result {
        Ok(()) => {
            revoke_all(&sessions, &id);
            HttpResponse::Ok().json(ApiResponse::ok("MFA disabled; sign in again"))
        }
        Err(e)
            if e == "Invalid credentials"
                || e == "소속 보안 정책에 따라 2단계 인증을 해제할 수 없습니다" =>
        {
            bad(&e)
        }
        Err(_) => saved_error(),
    }
}
#[post("/api/auth/2fa/recovery-codes")]
async fn regenerate(
    req: HttpRequest,
    body: web::Json<Factor>,
    users: web::Data<UserStore>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let id = actor_id(&req);
    let (codes, hashes) = recovery_codes();
    let result = crate::users::mutate_user(&users, &id, |user| {
        if !user.active()
            || !user.totp_enabled
            || !bcrypt::verify(&body.password, &user.password_hash).unwrap_or(false)
            || !consume_factor(
                user,
                &body.code,
                chrono::Utc::now().timestamp().max(0) as u64,
            )
        {
            return Err("Invalid credentials".into());
        }
        user.mfa.recovery_hashes = hashes;
        user.mfa.revision = uuid::Uuid::new_v4().to_string();
        Ok(())
    });
    match result {
        Ok(()) => {
            let current = crate::auth::verified_claims(&req).map(|c| c.sid);
            sessions
                .lock()
                .unwrap()
                .retain(|sid, s| s.user_id != id || current.as_deref() == Some(sid.as_str()));
            HttpResponse::Ok()
                .insert_header(("Cache-Control", "no-store"))
                .json(ApiResponse::ok(serde_json::json!({"recovery_codes":codes})))
        }
        Err(e) if e == "Invalid credentials" => bad(&e),
        Err(_) => saved_error(),
    }
}
pub(super) fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(status)
        .service(setup_2fa)
        .service(verify_setup)
        .service(disable_2fa)
        .service(regenerate);
}
