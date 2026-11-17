//! An account's own passkeys: list, register, rename, remove. Registering
//! and removing ask for the current password again, as the
//! authenticator-app settings do; an account without a local password
//! (signed in through LDAP or single sign-on) has no passkeys here.

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use webauthn_rs::prelude::RegisterPublicKeyCredential;

use super::challenges::Pending;
use super::{bad, store, unavailable, Passkeys, MAX_PER_ACCOUNT};
use crate::audit_change::{note, Change};
use crate::lock_safe::LockExt;
use crate::models::ApiResponse;
use crate::users::UserStore;

fn actor_id(req: &HttpRequest) -> String {
    crate::auth::verified_user_id(req).unwrap_or_default()
}

fn not_saved() -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Passkey could not be saved"))
}

fn local_password(hash: &str) -> bool {
    hash.starts_with("$2")
}

pub(super) async fn list(req: HttpRequest, passkeys: web::Data<Passkeys>, users: web::Data<UserStore>) -> HttpResponse {
    let id = actor_id(&req);
    let users = users.lock_safe();
    let Some(user) = users.iter().find(|u| u.id == id && u.active()) else {
        return HttpResponse::Unauthorized().finish();
    };
    let rows: Vec<_> = user
        .mfa
        .passkeys
        .iter()
        .map(|p| serde_json::json!({ "id": p.id, "name": p.name, "created_at": p.created_at, "last_used": p.last_used }))
        .collect();
    // Why it is off is for whoever can fix it.
    let reason = passkeys.off_reason().filter(|_| user.super_admin());
    HttpResponse::Ok().insert_header(("Cache-Control", "no-store")).json(ApiResponse::ok(serde_json::json!({
        "enabled": passkeys.off_reason().is_none(),
        "reason": reason,
        "local_password_available": local_password(&user.password_hash),
        "limit": MAX_PER_ACCOUNT,
        "passkeys": rows,
    })))
}

#[derive(Deserialize)]
pub(super) struct Begin {
    password: String,
}

pub(super) async fn register_begin(
    req: HttpRequest,
    body: web::Json<Begin>,
    passkeys: web::Data<Passkeys>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    let engine = match passkeys.engine() {
        Ok(engine) => engine,
        Err(off) => return off,
    };
    let id = actor_id(&req);
    // Checked off the workers and outside the users lock; `store::add`
    // confirms the hash is still the same when the passkey is saved.
    let Some(hash) = crate::password_check::verified_hash(&users, &id, &body.password).await.filter(|h| local_password(h)) else {
        return bad("Invalid password");
    };
    let Some(user) = users.lock_safe().iter().find(|u| u.id == id && u.active() && u.password_hash == hash).cloned() else {
        return bad("Invalid password");
    };
    if user.mfa.passkeys.len() >= MAX_PER_ACCOUNT {
        return bad("Too many passkeys");
    }
    let known = user.mfa.passkeys.iter().map(|p| p.credential.cred_id().clone()).collect();
    let started = engine.start_passkey_registration(store::handle(&user.id), &user.username, &user.username, Some(known));
    let Ok((options, state)) = started else {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Could not start passkey registration"));
    };
    let Ok(mut options) = serde_json::to_value(&options.public_key) else {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Could not start passkey registration"));
    };
    // Signing in names no account, so the authenticator must keep the
    // passkey itself (a discoverable credential). The library asks for user
    // verification and no attestation already.
    options["authenticatorSelection"]["residentKey"] = "required".into();
    options["authenticatorSelection"]["requireResidentKey"] = true.into();
    let Some(challenge) = passkeys.challenges.put(Pending::Register { user_id: user.id, password_hash: hash, state }) else {
        return unavailable();
    };
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(serde_json::json!({ "challenge_id": challenge, "publicKey": options })))
}

#[derive(Deserialize)]
pub(super) struct Finish {
    challenge_id: String,
    name: String,
    credential: RegisterPublicKeyCredential,
}

pub(super) async fn register_finish(
    req: HttpRequest,
    body: web::Json<Finish>,
    passkeys: web::Data<Passkeys>,
    users: web::Data<UserStore>,
) -> HttpResponse {
    let engine = match passkeys.engine() {
        Ok(engine) => engine,
        Err(off) => return off,
    };
    let Some(name) = store::name(&body.name) else {
        return bad("Passkey name must be 1 to 64 characters");
    };
    let id = actor_id(&req);
    let (password_hash, state) = match passkeys.challenges.take(&body.challenge_id) {
        Some(Pending::Register { user_id, password_hash, state }) if user_id == id => (password_hash, state),
        _ => return bad("Passkey registration expired; start again"),
    };
    let credential = match engine.finish_passkey_registration(&body.credential, &state) {
        Ok(credential) => credential,
        Err(error) => {
            log_warn!("passkeys", "passkey registration refused", user_id = id, reason = error);
            return bad("Passkey could not be verified");
        }
    };
    let passkey = store::stored(name, credential);
    let change = Change::new("passkey", &passkey.id, "create").field("name", None, Some(&passkey.name));
    match store::add(&users, &id, &password_hash, passkey) {
        Ok(()) => {
            note(&req, change);
            HttpResponse::Ok().json(ApiResponse::ok("Passkey registered"))
        }
        Err(e) if e == "This passkey is already registered" || e == "Account changed; try again" || e == "Too many passkeys" => bad(&e),
        Err(_) => not_saved(),
    }
}

#[derive(Deserialize)]
pub(super) struct Rename {
    id: String,
    name: String,
}

pub(super) async fn rename(req: HttpRequest, body: web::Json<Rename>, users: web::Data<UserStore>) -> HttpResponse {
    let Some(name) = store::name(&body.name) else {
        return bad("Passkey name must be 1 to 64 characters");
    };
    let result = crate::users::mutate_user(&users, &actor_id(&req), |user| {
        if !user.active() {
            return Err("Passkey not found".into());
        }
        let passkey = user.mfa.passkeys.iter_mut().find(|p| p.id == body.id).ok_or("Passkey not found")?;
        Ok(std::mem::replace(&mut passkey.name, name.clone()))
    });
    match result {
        Ok(old) => {
            note(&req, Change::new("passkey", &body.id, "update").field("name", Some(&old), Some(&name)));
            HttpResponse::Ok().json(ApiResponse::ok("Passkey renamed"))
        }
        Err(e) if e == "Passkey not found" || e == "User not found" => bad("Passkey not found"),
        Err(_) => not_saved(),
    }
}

#[derive(Deserialize)]
pub(super) struct Delete {
    id: String,
    password: String,
}

pub(super) async fn delete(req: HttpRequest, body: web::Json<Delete>, users: web::Data<UserStore>) -> HttpResponse {
    let id = actor_id(&req);
    let verified = crate::password_check::verified_hash(&users, &id, &body.password).await;
    let result = crate::users::mutate_user(&users, &id, |user| {
        if !user.active() || verified.as_deref() != Some(user.password_hash.as_str()) {
            return Err("Invalid password".into());
        }
        let index = user.mfa.passkeys.iter().position(|p| p.id == body.id).ok_or("Passkey not found")?;
        Ok(user.mfa.passkeys.remove(index).name)
    });
    match result {
        Ok(name) => {
            note(&req, Change::new("passkey", &body.id, "delete").field("name", Some(&name), None));
            HttpResponse::Ok().json(ApiResponse::ok("Passkey deleted"))
        }
        Err(e) if e == "Invalid password" || e == "Passkey not found" => bad(&e),
        Err(e) if e == "User not found" => bad("Invalid password"),
        Err(_) => not_saved(),
    }
}
