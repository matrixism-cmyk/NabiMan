use actix_web::{web, HttpResponse};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()))
}

fn oauth_config_path() -> PathBuf {
    data_dir().join("oauth_config.json")
}

// --- Models ---

#[derive(Serialize, Deserialize, Clone)]
pub struct OAuthConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider_name: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub authorize_url: String,
    #[serde(default)]
    pub token_url: String,
    #[serde(default)]
    pub userinfo_url: String,
    #[serde(default = "default_scopes")]
    pub scopes: String,
    #[serde(default)]
    pub redirect_uri: String,
    #[serde(default = "default_role_claim")]
    pub role_claim: String,
    #[serde(default = "default_admin_value")]
    pub admin_value: String,
    #[serde(default = "default_operator_value")]
    pub operator_value: String,
}

fn default_scopes() -> String {
    "openid profile email".to_string()
}

fn default_role_claim() -> String {
    "role".to_string()
}

fn default_admin_value() -> String {
    "admin".to_string()
}

fn default_operator_value() -> String {
    "operator".to_string()
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider_name: String::new(),
            client_id: String::new(),
            client_secret: String::new(),
            authorize_url: String::new(),
            token_url: String::new(),
            userinfo_url: String::new(),
            scopes: default_scopes(),
            redirect_uri: String::new(),
            role_claim: default_role_claim(),
            admin_value: default_admin_value(),
            operator_value: default_operator_value(),
        }
    }
}

#[derive(Serialize)]
struct OAuthConfigMasked {
    pub enabled: bool,
    pub provider_name: String,
    pub client_id: String,
    pub client_secret: String,
    pub authorize_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    pub scopes: String,
    pub redirect_uri: String,
    pub role_claim: String,
    pub admin_value: String,
    pub operator_value: String,
}

impl From<&OAuthConfig> for OAuthConfigMasked {
    fn from(cfg: &OAuthConfig) -> Self {
        Self {
            enabled: cfg.enabled,
            provider_name: cfg.provider_name.clone(),
            client_id: cfg.client_id.clone(),
            client_secret: if cfg.client_secret.is_empty() { String::new() } else { "***".into() },
            authorize_url: cfg.authorize_url.clone(),
            token_url: cfg.token_url.clone(),
            userinfo_url: cfg.userinfo_url.clone(),
            scopes: cfg.scopes.clone(),
            redirect_uri: cfg.redirect_uri.clone(),
            role_claim: cfg.role_claim.clone(),
            admin_value: cfg.admin_value.clone(),
            operator_value: cfg.operator_value.clone(),
        }
    }
}

#[derive(Serialize)]
struct AuthorizeResponse {
    pub url: String,
    pub state: String,
}

// --- Storage ---

fn load_config() -> OAuthConfig {
    let path = oauth_config_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        OAuthConfig::default()
    }
}

fn save_config(cfg: &OAuthConfig) {
    let path = oauth_config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::to_string_pretty(cfg).unwrap_or_default());
}

fn generate_state() -> String {
    use rand::Rng;
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn url_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 2);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
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

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

fn exchange_code(cfg: &OAuthConfig, code: &str) -> Result<TokenResponse, String> {
    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}",
        url_encode(code),
        url_encode(&cfg.redirect_uri),
        url_encode(&cfg.client_id),
        url_encode(&cfg.client_secret),
    );

    let output = std::process::Command::new("curl")
        .args([
            "-s", "-X", "POST",
            "-H", "Content-Type: application/x-www-form-urlencoded",
            "-H", "Accept: application/json",
            "-d", &body,
            "--max-time", "10",
            &cfg.token_url,
        ])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err("Token endpoint returned error".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .map_err(|e| format!("Invalid token response: {} — {}", e, &stdout[..stdout.len().min(200)]))
}

fn fetch_userinfo(url: &str, access_token: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let auth_header = format!("Authorization: Bearer {}", access_token);
    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-H", &auth_header,
            "-H", "Accept: application/json",
            "--max-time", "10",
            url,
        ])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let val: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Invalid userinfo: {}", e))?;

    val.as_object().cloned()
        .ok_or_else(|| "Userinfo is not a JSON object".into())
}

fn determine_role(cfg: &OAuthConfig, info: &serde_json::Map<String, serde_json::Value>) -> String {
    // Check role claim in userinfo
    if let Some(claim_val) = info.get(&cfg.role_claim) {
        let check = |v: &str| -> Option<String> {
            if v == cfg.admin_value { Some("admin".into()) }
            else if v == cfg.operator_value { Some("operator".into()) }
            else { None }
        };

        if let Some(s) = claim_val.as_str() {
            if let Some(r) = check(s) { return r; }
        }
        // Also check if it's an array of roles/groups
        if let Some(arr) = claim_val.as_array() {
            for item in arr {
                if let Some(s) = item.as_str() {
                    if let Some(r) = check(s) { return r; }
                }
            }
        }
    }

    // Check groups claim as fallback
    if let Some(groups) = info.get("groups").and_then(|v| v.as_array()) {
        for g in groups {
            if let Some(s) = g.as_str() {
                if s == cfg.admin_value { return "admin".into(); }
                if s == cfg.operator_value { return "operator".into(); }
            }
        }
    }

    "viewer".into()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = OAuthConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.scopes, "openid profile email");
        assert_eq!(cfg.role_claim, "role");
    }

    #[test]
    fn test_masked_config_hides_secret() {
        let cfg = OAuthConfig {
            client_secret: "my_super_secret_value".into(),
            ..OAuthConfig::default()
        };
        let masked = OAuthConfigMasked::from(&cfg);
        assert_eq!(masked.client_secret, "***");
    }

    #[test]
    fn test_masked_config_empty_secret() {
        let cfg = OAuthConfig::default();
        let masked = OAuthConfigMasked::from(&cfg);
        assert_eq!(masked.client_secret, "");
    }

    #[test]
    fn test_url_encode_simple() {
        assert_eq!(url_encode("hello"), "hello");
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a=b&c=d"), "a%3Db%26c%3Dd");
    }

    #[test]
    fn test_generate_state_length() {
        let state = generate_state();
        assert_eq!(state.len(), 32);
        assert!(state.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_url_encode_special_chars() {
        assert_eq!(url_encode("openid profile email"), "openid%20profile%20email");
        assert_eq!(url_encode("https://example.com/callback"), "https%3A%2F%2Fexample.com%2Fcallback");
    }

    #[test]
    fn test_determine_role_admin_string() {
        let cfg = OAuthConfig { admin_value: "admin".into(), operator_value: "operator".into(), role_claim: "role".into(), ..OAuthConfig::default() };
        let mut info = serde_json::Map::new();
        info.insert("role".into(), serde_json::json!("admin"));
        assert_eq!(determine_role(&cfg, &info), "admin");
    }

    #[test]
    fn test_determine_role_array() {
        let cfg = OAuthConfig { admin_value: "admins".into(), operator_value: "ops".into(), role_claim: "groups".into(), ..OAuthConfig::default() };
        let mut info = serde_json::Map::new();
        info.insert("groups".into(), serde_json::json!(["users", "ops"]));
        assert_eq!(determine_role(&cfg, &info), "operator");
    }

    #[test]
    fn test_determine_role_default_viewer() {
        let cfg = OAuthConfig::default();
        let info = serde_json::Map::new();
        assert_eq!(determine_role(&cfg, &info), "viewer");
    }
}
