//! Passkeys end to end against the real handlers, the sign-in gate and the
//! rate limiter, with the library's software authenticator standing in for
//! a browser and its device.

mod manage;
mod sign_in;
mod storage;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::http::StatusCode;
use actix_web::{test, web};
use serde_json::{json, Value};
use webauthn_authenticator_rs::softpasskey::SoftPasskey;
use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse, Url};

use super::Passkeys;
use crate::audit_change::Change;
use crate::jwt_sessions::SessionStore;
use crate::lock_safe::LockExt;
use crate::models::NabimanUser;
use crate::users::UserStore;

pub(super) const ORIGIN: &str = "https://man.example.com";
pub(super) const PASSWORD: &str = "correct horse battery";

/// A browser with one device in it. The software authenticator keeps no
/// discoverable credentials, so the "browser" remembers which account each
/// credential was made for, picks the credential, and reports the account's
/// user handle — which a real authenticator does by itself.
pub(super) struct Browser {
    device: SoftPasskey,
    pub(super) origin: Url,
    handles: HashMap<String, Value>,
}

impl Browser {
    pub(super) fn at(origin: &str) -> Self {
        Self { device: SoftPasskey::new(true), origin: Url::parse(origin).unwrap(), handles: HashMap::new() }
    }

    /// `navigator.credentials.create`: the credential as the page posts it.
    pub(super) fn create(&mut self, options: &Value) -> Value {
        let selection = &options["authenticatorSelection"];
        assert_eq!(selection["residentKey"], "required", "a discoverable credential");
        assert_eq!(selection["requireResidentKey"], true);
        assert_eq!(selection["userVerification"], "required");
        assert_eq!(options["attestation"], "none");
        let mut asked = options.clone();
        asked["authenticatorSelection"]["requireResidentKey"] = false.into();
        asked["authenticatorSelection"]["residentKey"] = "discouraged".into();
        let request: CreationChallengeResponse = serde_json::from_value(json!({ "publicKey": asked })).unwrap();
        let made = serde_json::to_value(self.device.do_registration(self.origin.clone(), request).unwrap()).unwrap();
        self.handles.insert(made["id"].as_str().unwrap().to_string(), options["user"]["id"].clone());
        made
    }

    /// `navigator.credentials.get` for the credential `id` (base64url).
    pub(super) fn get(&mut self, options: &Value, id: &str) -> Value {
        assert_eq!(options["userVerification"], "required");
        assert!(options["allowCredentials"].as_array().is_none_or(|a| a.is_empty()), "names no account");
        let mut asked = options.clone();
        asked["allowCredentials"] = json!([{ "type": "public-key", "id": id }]);
        let request: RequestChallengeResponse = serde_json::from_value(json!({ "publicKey": asked })).unwrap();
        let mut answer = serde_json::to_value(self.device.do_authentication(self.origin.clone(), request).unwrap()).unwrap();
        answer["response"]["userHandle"] = self.handles[id].clone();
        answer
    }
}

pub(super) fn user(id: &str, name: &str, role: &str) -> NabimanUser {
    serde_json::from_value(json!({
        "id": id, "username": name, "password_hash": bcrypt::hash(PASSWORD, 4).unwrap(),
        "role": role, "created_at": "now", "status": "active", "share_authorization_version": id
    }))
    .unwrap()
}

/// What the audit middleware would record: path, status, and the noted change.
pub(super) type Recorded = Arc<Mutex<Vec<(String, u16, Option<Change>)>>>;

pub(super) struct World {
    pub users: UserStore,
    pub sessions: SessionStore,
    pub passkeys: web::Data<Passkeys>,
    pub recorded: Recorded,
    _storage: Box<dyn std::any::Any>,
}

impl World {
    pub(super) fn with(users: Vec<NabimanUser>) -> Self {
        Self::configured(users, Some(ORIGIN))
    }

    pub(super) fn configured(users: Vec<NabimanUser>, origin: Option<&str>) -> Self {
        Self {
            _storage: Box::new(crate::users::isolated_test_storage()),
            users: Arc::new(Mutex::new(users)),
            sessions: crate::jwt_sessions::new_session_store(),
            passkeys: web::Data::new(Passkeys::from_setting(origin, "NabiMan")),
            recorded: Recorded::default(),
        }
    }

    pub(super) fn update(&self, name: &str, change: impl FnOnce(&mut NabimanUser)) {
        change(self.users.lock_safe().iter_mut().find(|u| u.username == name).unwrap());
    }

    pub(super) fn account(&self, name: &str) -> NabimanUser {
        crate::users::find_user_by_name(&self.users, name).unwrap()
    }
}

/// The App as `main` builds it, as far as signing in goes.
macro_rules! serve {
    ($world:expr) => {{
        let secret: crate::auth::JwtSecret = std::sync::Arc::new("test-secret".to_string());
        let recorded = $world.recorded.clone();
        actix_web::test::init_service(
            actix_web::App::new()
                .wrap(crate::rate_limit::RateLimiter::new())
                .wrap(crate::AuthCheck::new(secret.clone(), $world.sessions.clone(), $world.users.clone()))
                .wrap_fn(move |req, service| {
                    use actix_web::dev::Service as _;
                    let (recorded, path) = (recorded.clone(), req.path().to_string());
                    let answered = service.call(req);
                    async move {
                        let res = answered.await?;
                        let change = (res.status().as_u16() < 400).then(|| crate::audit_change::noted(res.request())).flatten();
                        recorded.lock().unwrap().push((path, res.status().as_u16(), change));
                        Ok(res)
                    }
                })
                .app_data(actix_web::web::Data::new(secret))
                .app_data(actix_web::web::Data::new($world.users.clone()))
                .app_data(actix_web::web::Data::new($world.sessions.clone()))
                .app_data($world.passkeys.clone())
                .configure(crate::passkeys::config)
                .configure(crate::auth::config)
                .configure(crate::totp::config),
        )
        .await
    }};
}
pub(super) use serve;

pub(super) async fn call<S, B>(app: &S, method: &str, path: &str, token: Option<&str>, body: Value) -> (StatusCode, Value)
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    call_from(app, method, path, token, body, None).await
}

pub(super) async fn call_from<S, B>(app: &S, method: &str, path: &str, token: Option<&str>, body: Value, peer: Option<&str>) -> (StatusCode, Value)
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let mut request = match method {
        "GET" => test::TestRequest::get(),
        _ => test::TestRequest::post().set_json(body),
    }
    .uri(path);
    if let Some(token) = token {
        request = request.insert_header(("Authorization", format!("Bearer {token}")));
    }
    if let Some(peer) = peer {
        request = request.peer_addr(peer.parse().unwrap());
    }
    let response = test::call_service(app, request.to_request()).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// Signs in with the password and gives the access token.
pub(super) async fn password_token<S, B>(app: &S, name: &str) -> String
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let (status, body) = call(app, "POST", "/api/auth/login", None, json!({ "username": name, "password": PASSWORD })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["data"]["token"].as_str().expect("a token").to_string()
}

/// Registers a passkey called `name` for the signed-in account; its credential id.
pub(super) async fn register<S, B>(app: &S, browser: &mut Browser, token: &str, name: &str) -> String
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let (status, begun) = call(app, "POST", "/api/auth/passkeys/register/begin", Some(token), json!({ "password": PASSWORD })).await;
    assert_eq!(status, StatusCode::OK, "{begun}");
    let credential = browser.create(&begun["data"]["publicKey"]);
    let id = credential["id"].as_str().unwrap().to_string();
    let body = json!({ "challenge_id": begun["data"]["challenge_id"], "name": name, "credential": credential });
    let (status, done) = call(app, "POST", "/api/auth/passkeys/register/finish", Some(token), body).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    id
}

/// A challenge and the browser's answer to it, ready to post.
pub(super) async fn answer<S, B>(app: &S, browser: &mut Browser, id: &str) -> Value
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let (status, begun) = call(app, "POST", "/api/auth/passkeys/login/begin", None, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{begun}");
    json!({ "challenge_id": begun["data"]["challenge_id"], "credential": browser.get(&begun["data"]["publicKey"], id) })
}

/// A whole passkey sign-in: the status and the answer's body.
pub(super) async fn sign_in<S, B>(app: &S, browser: &mut Browser, id: &str) -> (StatusCode, Value)
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let body = answer(app, browser, id).await;
    call(app, "POST", "/api/auth/passkeys/login/finish", None, body).await
}

/// The same, with the answer coming from `peer`.
pub(super) async fn sign_in_from<S, B>(app: &S, browser: &mut Browser, id: &str, peer: &str) -> (StatusCode, Value)
where
    S: Service<actix_http::Request, Response = ServiceResponse<B>, Error = actix_web::Error>,
    B: MessageBody,
{
    let body = answer(app, browser, id).await;
    call_from(app, "POST", "/api/auth/passkeys/login/finish", None, body, Some(peer)).await
}
