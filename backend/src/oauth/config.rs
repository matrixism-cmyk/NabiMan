use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()))
}

fn oauth_config_path() -> PathBuf {
    data_dir().join("oauth_config.json")
}

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

pub fn default_scopes() -> String {
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
pub struct OAuthConfigMasked {
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

pub fn load_config() -> OAuthConfig {
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

pub fn save_config(cfg: &OAuthConfig) {
    let path = oauth_config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::to_string_pretty(cfg).unwrap_or_default());
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
}
