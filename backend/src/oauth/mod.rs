use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};

mod config;
mod oidc;

use config::{load_config, save_config, default_scopes, OAuthConfig, OAuthConfigMasked};
use oidc::{determine_role, exchange_code, fetch_userinfo, generate_state, url_encode};

#[derive(Serialize)]
struct AuthorizeResponse {
    pub url: String,
    pub state: String,
}

// --- Handlers ---

async fn get_config() -> HttpResponse {
    let cfg = load_config();
    let masked = OAuthConfigMasked::from(&cfg);
    HttpResponse::Ok().json(ApiResponse::ok(masked))
}

async fn save_oauth_config(body: web::Json<OAuthConfig>) -> HttpResponse {
    let mut cfg = body.into_inner();

    // If secret is masked, preserve existing secret
    if cfg.client_secret == "***" {
        let existing = load_config();
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

    save_config(&cfg);
    let masked = OAuthConfigMasked::from(&cfg);
    HttpResponse::Ok().json(ApiResponse::ok(masked))
}

// --- OAuth2 callback: exchange code for token, fetch userinfo, create session ---

#[derive(Deserialize)]
struct CallbackQuery {
    code: String,
    #[allow(dead_code)]
    state: Option<String>,
}

async fn callback(
    query: web::Query<CallbackQuery>,
    secret: web::Data<crate::auth::JwtSecret>,
    sessions: web::Data<crate::jwt_sessions::SessionStore>,
    user_store: web::Data<crate::users::UserStore>,
) -> HttpResponse {
    let cfg = load_config();
    if !cfg.enabled {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("OAuth is not enabled"));
    }

    // Step 1: Exchange authorization code for access token
    let token_response = match exchange_code(&cfg, &query.code) {
        Ok(t) => t,
        Err(e) => return HttpResponse::Ok().json(ApiResponse::<()>::error(&format!("Token exchange failed: {}", e))),
    };

    // Step 2: Fetch user info using the access token
    let user_info = if !cfg.userinfo_url.is_empty() {
        match fetch_userinfo(&cfg.userinfo_url, &token_response.access_token) {
            Ok(info) => info,
            Err(e) => return HttpResponse::Ok().json(ApiResponse::<()>::error(&format!("Userinfo fetch failed: {}", e))),
        }
    } else {
        serde_json::Map::new()
    };

    // Step 3: Determine username and role from userinfo
    let username = user_info.get("email")
        .or_else(|| user_info.get("preferred_username"))
        .or_else(|| user_info.get("sub"))
        .and_then(|v| v.as_str())
        .unwrap_or("oauth_user")
        .to_string();

    let role = determine_role(&cfg, &user_info);

    // Step 4: Auto-provision user if not exists
    crate::users::ensure_oauth_user(&user_store, &username, &role);

    // Step 5: Create session and JWT
    let sid = crate::jwt_sessions::generate_session_id();
    crate::jwt_sessions::register_session(&sessions, &sid, &username, "oauth");

    match crate::jwt_sessions::create_token_pair(&secret, &username, &role, &sid) {
        Ok((access, refresh)) => HttpResponse::Ok().json(ApiResponse::ok(
            serde_json::json!({
                "token": access,
                "refresh_token": refresh,
                "username": username,
                "role": role,
                "provider": cfg.provider_name,
            })
        )),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&e)),
    }
}

async fn authorize() -> HttpResponse {
    let cfg = load_config();

    if !cfg.enabled {
        return HttpResponse::Ok().json(ApiResponse::<()>::error("OAuth is not enabled"));
    }
    if cfg.authorize_url.is_empty() || cfg.client_id.is_empty() {
        return HttpResponse::Ok().json(
            ApiResponse::<()>::error("OAuth authorize URL or client ID not configured"),
        );
    }

    let state = generate_state();
    let scopes = if cfg.scopes.is_empty() { default_scopes() } else { cfg.scopes.clone() };

    let url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
        cfg.authorize_url,
        url_encode(&cfg.client_id),
        url_encode(&cfg.redirect_uri),
        url_encode(&scopes),
        url_encode(&state),
    );

    HttpResponse::Ok().json(ApiResponse::ok(AuthorizeResponse { url, state }))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/auth/oauth")
            .route("/config", web::get().to(get_config))
            .route("/config", web::post().to(save_oauth_config))
            .route("/authorize", web::get().to(authorize))
            .route("/callback", web::get().to(callback))
    );
}
