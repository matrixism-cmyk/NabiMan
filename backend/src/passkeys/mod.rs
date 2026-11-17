//! Passkey (WebAuthn) sign-in: a passkey alone signs an account in, and the
//! password stays as the other way in.
//!
//! * Signing in names no account. `login/begin` hands out a challenge;
//!   `login/finish` checks the answer (origin, relying-party id, challenge,
//!   signature, user verification, signature counter: `webauthn-rs`), finds
//!   the account by the credential, applies every check a password sign-in
//!   applies to the account, and issues the same session and tokens. The
//!   authenticator verified the person (PIN, fingerprint, face), so no
//!   authenticator-app code is asked for afterwards.
//! * An account registers, renames and removes its own passkeys in its
//!   security settings. Registering and removing ask for the current
//!   password again, as the authenticator-app settings do.
//!
//! Which site the passkeys belong to comes from `NABIMAN_PUBLIC_ORIGIN`,
//! never from a request's `Host` header; without it passkeys are off (see
//! `site`). Passkeys are kept with the account in the user database;
//! challenges live in memory, briefly, and are used once (`challenges`).

mod challenges;
mod login;
mod manage;
mod site;
mod store;
#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

use actix_web::{web, HttpResponse};
use webauthn_rs::prelude::Webauthn;

use crate::models::ApiResponse;

/// Passkeys one account may have.
pub(crate) const MAX_PER_ACCOUNT: usize = 10;

/// The relying party of this install and its outstanding challenges.
pub(crate) struct Passkeys {
    /// `Err`: why passkeys are off here (shown to an administrator, logged).
    engine: Result<Webauthn, String>,
    challenges: challenges::Challenges,
}

impl Passkeys {
    /// From `NABIMAN_PUBLIC_ORIGIN` (`origins`) and the product name.
    pub(crate) fn from_setting(origins: Option<&str>, product: &str) -> Self {
        Self { engine: site::engine(origins, product), challenges: challenges::Challenges::default() }
    }

    /// As this process was started.
    pub(crate) fn from_env() -> Self {
        use crate::install_config::values::{env, text_in};
        let product = text_in(&env, "NABIMAN_PRODUCT_NAME").unwrap_or_else(|| "NabiMan".into());
        let passkeys = Self::from_setting(text_in(&env, site::SETTING).as_deref(), &product);
        match &passkeys.engine {
            Ok(_) => log_info!("passkeys", "passkey sign-in is on", origins = text_in(&env, site::SETTING).unwrap_or_default()),
            Err(why) => log_info!("passkeys", "passkey sign-in is off", reason = why),
        }
        passkeys
    }

    fn engine(&self) -> Result<&Webauthn, HttpResponse> {
        self.engine.as_ref().map_err(|_| unavailable())
    }

    fn off_reason(&self) -> Option<&str> {
        self.engine.as_ref().err().map(String::as_str)
    }
}

/// Passkeys are not set up on this install (503, so the page can say so).
fn unavailable() -> HttpResponse {
    HttpResponse::ServiceUnavailable()
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::<()>::error("Passkeys are not configured on this server"))
}

/// One answer for every failed passkey sign-in, whatever failed.
fn invalid() -> HttpResponse {
    HttpResponse::Unauthorized().json(ApiResponse::<()>::error("Invalid credentials"))
}

fn bad(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::<()>::error(message))
}

/// Whether the rate limiter counts attempts on this path (POST).
pub(crate) fn rate_limited(path: &str) -> bool {
    matches!(
        path,
        "/api/auth/passkeys/login/begin"
            | "/api/auth/passkeys/login/finish"
            | "/api/auth/passkeys/register/begin"
            | "/api/auth/passkeys/register/finish"
            | "/api/auth/passkeys/delete"
    )
}

/// The two steps of signing in, reached without a session.
pub(crate) fn public_route(path: &str) -> bool {
    matches!(path, "/api/auth/passkeys/login/begin" | "/api/auth/passkeys/login/finish")
}

/// What every signed-in account may call: its own passkeys.
pub(crate) fn own_route(method: &str, path: &str) -> bool {
    (method == "GET" && path == "/api/auth/passkeys")
        || (method == "POST"
            && matches!(
                path,
                "/api/auth/passkeys/register/begin"
                    | "/api/auth/passkeys/register/finish"
                    | "/api/auth/passkeys/rename"
                    | "/api/auth/passkeys/delete"
            ))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/auth/passkeys")
            .route("", web::get().to(manage::list))
            .route("/login/begin", web::post().to(login::begin))
            .route("/login/finish", web::post().to(login::finish))
            .route("/register/begin", web::post().to(manage::register_begin))
            .route("/register/finish", web::post().to(manage::register_finish))
            .route("/rename", web::post().to(manage::rename))
            .route("/delete", web::post().to(manage::delete)),
    );
}
