//! Signing in with a passkey, without naming an account first.

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use webauthn_rs::prelude::PublicKeyCredential;

use super::challenges::Pending;
use super::{invalid, store, unavailable, Passkeys};
use crate::auth::JwtSecret;
use crate::jwt_sessions::SessionStore;
use crate::models::{AccountStatus, ApiResponse};
use crate::users::UserStore;

/// A challenge for whichever passkey of this site the person picks.
pub(super) async fn begin(passkeys: web::Data<Passkeys>) -> HttpResponse {
    let engine = match passkeys.engine() {
        Ok(engine) => engine,
        Err(off) => return off,
    };
    let Ok((options, state)) = engine.start_discoverable_authentication() else {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Could not start passkey sign-in"));
    };
    let Some(id) = passkeys.challenges.put(Pending::Login(state)) else {
        return unavailable();
    };
    // Only the request options: how the browser asks (a prompt, not form
    // autofill) is the page's choice.
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(serde_json::json!({ "challenge_id": id, "publicKey": options.public_key })))
}

#[derive(Deserialize)]
pub(super) struct Answer {
    challenge_id: String,
    credential: PublicKeyCredential,
}

/// The answer to a challenge: a full sign-in when it holds.
pub(super) async fn finish(
    req: HttpRequest,
    body: web::Json<Answer>,
    passkeys: web::Data<Passkeys>,
    users: web::Data<UserStore>,
    secret: web::Data<JwtSecret>,
    sessions: web::Data<SessionStore>,
) -> HttpResponse {
    let engine = match passkeys.engine() {
        Ok(engine) => engine,
        Err(off) => return off,
    };
    // Taken before anything is checked: one answer per challenge.
    let Some(Pending::Login(state)) = passkeys.challenges.take(&body.challenge_id) else {
        return invalid();
    };
    let credential = &body.credential;
    // The rate limiter counts failures by the id as written; it must be the
    // one way of writing the credential the answer is really for.
    if credential.id != store::browser_id(credential.get_credential_id()) {
        return invalid();
    }
    let Some((account, passkey)) = store::owner(&users, credential.get_credential_id()) else {
        return invalid();
    };
    // The authenticator says whose passkey it used; it must be this account's.
    if credential.get_user_unique_id() != Some(store::handle(&account.id).as_bytes().as_slice()) {
        return invalid();
    }
    let result = match engine.finish_discoverable_authentication_v2(credential, state, &passkey.credential) {
        Ok(result) if result.user_verified() => result,
        Ok(_) => return invalid(),
        Err(error) => {
            log_warn!("passkeys", "passkey sign-in refused", passkey = passkey.id, user = account.username, reason = error);
            return invalid();
        }
    };
    // From here on, what a password sign-in does once the password matched.
    if !account.active() {
        let why = if account.status == AccountStatus::Active && account.access.expired() {
            "계정 이용 기간이 끝났습니다. 관리자에게 문의하세요."
        } else {
            "가입 승인 대기, 반려 또는 이용 정지 상태입니다. 최고관리자에게 문의하세요."
        };
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error(why));
    }
    let address = crate::access_rules::client_ip(&req);
    if let Err(why) = crate::access_rules::admits(&account, address) {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error(&why));
    }
    // The counter and the use are saved before any session or token exists.
    let user = match store::used(&users, &account.id, &result) {
        Ok(user) => user,
        Err(error) if error == "Invalid credentials" => return invalid(),
        Err(_) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Authentication state could not be saved"))
        }
    };
    let sid = crate::jwt_sessions::generate_session_id();
    let ip = address.map(|a| a.to_string()).unwrap_or_default();
    // The authenticator verified the person: the second factor is done.
    if !crate::jwt_sessions::register_authenticated_session(&sessions, &users, &sid, &user, true, &ip) {
        return invalid();
    }
    match crate::jwt_sessions::create_token_pair(&secret, &user.username, user.role.as_str(), &sid) {
        Ok((access, refresh)) => {
            let change = crate::audit_change::Change::new("passkey", &passkey.id, "sign-in")
                .field("username", None, Some(&user.username))
                .field("name", None, Some(&passkey.name));
            crate::audit_change::note(&req, change);
            HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(
                serde_json::json!({ "token": access, "refresh_token": refresh, "username": user.username, "role": user.role }),
            ))
        }
        Err(_) => {
            sessions.lock_safe().remove(&sid);
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Token issuance failed"))
        }
    }
}
