use crate::models::ApiResponse;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

mod config;
mod login_state;
mod oidc;

use config::{default_scopes, load_config, save_config, OAuthConfig, OAuthConfigMasked};
use oidc::{determine_role, exchange_code, fetch_userinfo, url_encode};

#[derive(Serialize)]
struct AuthorizeResponse {
    pub url: String,
    pub state: String,
}

// --- Handlers ---

async fn get_config() -> HttpResponse {
    let cfg = match load_config() {
        Ok(cfg) => cfg,
        Err(error) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error))
        }
    };
    let masked = OAuthConfigMasked::from(&cfg);
    HttpResponse::Ok().json(ApiResponse::ok(masked))
}

async fn save_oauth_config(body: web::Json<OAuthConfig>) -> HttpResponse {
    let mut cfg = body.into_inner();

    // If secret is masked, preserve existing secret
    if cfg.client_secret == "***" {
        let existing = match load_config() {
            Ok(existing) => existing,
            Err(error) => {
                return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error))
            }
        };
        cfg.client_secret = existing.client_secret;
    }

    if cfg.enabled {
        if cfg.client_id.is_empty() {
            return HttpResponse::Ok().json(ApiResponse::<()>::error("Client ID is required"));
        }
        if cfg.authorize_url.is_empty() {
            return HttpResponse::Ok().json(ApiResponse::<()>::error("Authorize URL is required"));
        }
        if cfg.token_url.is_empty() {
            return HttpResponse::Ok().json(ApiResponse::<()>::error("Token URL is required"));
        }
    }

    if let Err(error) = save_config(&cfg) {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error));
    }
    let masked = OAuthConfigMasked::from(&cfg);
    HttpResponse::Ok().json(ApiResponse::ok(masked))
}

// --- OAuth2 callback: exchange code for token, fetch userinfo, create session ---

#[derive(Deserialize)]
struct CallbackQuery {
    code: String,
    state: Option<String>,
}

async fn callback(
    request: HttpRequest,
    query: web::Query<CallbackQuery>,
    secret: web::Data<crate::auth::JwtSecret>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    user_store: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    // Older configurations point the provider at this API route. Send browser
    // navigation to the login screen, which completes the same-origin fetch.
    // Do not consume the state until that fetch verifies the browser cookie.
    if request
        .headers()
        .get("Accept")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/html"))
    {
        let location = format!(
            "/?code={}&state={}",
            url_encode(&query.code),
            url_encode(query.state.as_deref().unwrap_or(""))
        );
        return HttpResponse::SeeOther()
            .insert_header(("Location", location))
            .insert_header(("Cache-Control", "no-store"))
            .finish();
    }
    let browser = request.cookie(login_state::COOKIE);
    if !query
        .state
        .as_deref()
        .zip(browser.as_ref())
        .is_some_and(|(state, cookie)| login_state::consume(state, cookie.value()))
    {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "OAuth login expired or did not originate in this browser",
        ));
    }
    let cfg = match load_config() {
        Ok(cfg) => cfg,
        Err(error) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error))
        }
    };
    if !cfg.enabled {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("OAuth is not enabled"));
    }

    // Step 1: Exchange authorization code for access token
    let token_response = match exchange_code(&cfg, &query.code) {
        Ok(t) => t,
        Err(e) => {
            return HttpResponse::Ok().json(ApiResponse::<()>::error(&format!(
                "Token exchange failed: {}",
                e
            )))
        }
    };

    // Step 2: Fetch user info using the access token
    let user_info = if !cfg.userinfo_url.is_empty() {
        match fetch_userinfo(&cfg.userinfo_url, &token_response.access_token) {
            Ok(info) => info,
            Err(e) => {
                return HttpResponse::Ok().json(ApiResponse::<()>::error(&format!(
                    "Userinfo fetch failed: {}",
                    e
                )))
            }
        }
    } else {
        serde_json::Map::new()
    };

    // Step 3: Determine username and role from userinfo
    let username = match ["email", "preferred_username", "sub"]
        .iter()
        .filter_map(|claim| user_info.get(*claim).and_then(|value| value.as_str()))
        .find(|value| !value.trim().is_empty())
    {
        Some(username) => username.to_string(),
        None => {
            return HttpResponse::Unauthorized().json(ApiResponse::<()>::error(
                "OAuth provider did not return a user identity",
            ))
        }
    };

    let role = determine_role(&cfg, &user_info);

    // Step 4: Auto-provision user if not exists
    if let Err(error) = crate::users::ensure_oauth_user(&user_store, &username, &role) {
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error));
    }

    // Step 5: Create session and JWT
    let user = match crate::users::find_user_by_name(&user_store, &username) {
        Some(user) if user.active() => user,
        _ => {
            return HttpResponse::Forbidden().json(ApiResponse::<()>::error(
                "최고관리자의 가입 승인이 필요합니다",
            ))
        }
    };
    let role = user.role.as_str().to_string();
    let sid = crate::jwt_sessions::generate_session_id();
    if !crate::jwt_sessions::register_authenticated_session(
        &sessions,
        &user_store,
        &sid,
        &user,
        false,
        &crate::access_rules::client_ip(&request).map(|a| a.to_string()).unwrap_or_else(|| "oauth".into()),
    ) {
        return HttpResponse::Unauthorized().json(ApiResponse::<()>::error(
            "Account changed during authentication; sign in again",
        ));
    }

    match crate::jwt_sessions::create_token_pair(&secret, &username, &role, &sid) {
        Ok((access, refresh)) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "no-store"))
            .json(ApiResponse::ok(serde_json::json!({
                "token": access,
                "refresh_token": refresh,
                "username": username,
                "role": role,
                "provider": cfg.provider_name,
            }))),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

async fn authorize(request: HttpRequest) -> HttpResponse {
    let cfg = match load_config() {
        Ok(cfg) => cfg,
        Err(error) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&error))
        }
    };

    if !cfg.enabled {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("OAuth is not enabled"));
    }
    if cfg.authorize_url.is_empty() || cfg.client_id.is_empty() {
        return HttpResponse::Ok().json(ApiResponse::<()>::error(
            "OAuth authorize URL or client ID not configured",
        ));
    }

    let (state, browser) = match login_state::issue() {
        Ok(pair) => pair,
        Err(error) => {
            return HttpResponse::ServiceUnavailable().json(ApiResponse::<()>::error(error))
        }
    };
    let scopes = if cfg.scopes.is_empty() {
        default_scopes()
    } else {
        cfg.scopes.clone()
    };

    let url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
        cfg.authorize_url,
        url_encode(&cfg.client_id),
        url_encode(&cfg.redirect_uri),
        url_encode(&scopes),
        url_encode(&state),
    );

    let cookie = actix_web::cookie::Cookie::build(login_state::COOKIE, browser)
        .path("/api/auth/oauth")
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        .secure(request.connection_info().scheme() == "https")
        .max_age(actix_web::cookie::time::Duration::minutes(10))
        .finish();
    HttpResponse::Ok()
        .cookie(cookie)
        .insert_header(("Cache-Control", "no-store"))
        .json(ApiResponse::ok(AuthorizeResponse { url, state }))
}

async fn provider() -> HttpResponse {
    match load_config() {
        Ok(cfg) => HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
            "enabled": cfg.enabled, "provider_name": cfg.provider_name,
        }))),
        Err(_) => HttpResponse::ServiceUnavailable()
            .json(ApiResponse::<()>::error("OAuth configuration unavailable")),
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/auth/oauth")
            .route("/config", web::get().to(get_config))
            .route("/config", web::post().to(save_oauth_config))
            .route("/provider", web::get().to(provider))
            .route("/authorize", web::get().to(authorize))
            .route("/callback", web::get().to(callback)),
    );
}
