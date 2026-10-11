use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("NABIMAN_DATA_DIR").unwrap_or_else(|_| "/var/lib/nabiman".into()))
}

fn ldap_config_path() -> PathBuf {
    data_dir().join("ldap_config.json")
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LdapConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub bind_dn: String,
    #[serde(default)]
    pub bind_password: String,
    #[serde(default)]
    pub search_base: String,
    #[serde(default = "default_user_filter")]
    pub user_filter: String,
    #[serde(default)]
    pub tls: bool,
    #[serde(default)]
    pub admin_group: String,
    #[serde(default)]
    pub operator_group: String,
}

fn default_user_filter() -> String {
    "(uid={username})".to_string()
}

impl Default for LdapConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: "ldap://localhost:389".to_string(),
            bind_dn: String::new(),
            bind_password: String::new(),
            search_base: "dc=example,dc=com".to_string(),
            user_filter: default_user_filter(),
            tls: false,
            admin_group: "cn=admins,ou=groups,dc=example,dc=com".to_string(),
            operator_group: "cn=operators,ou=groups,dc=example,dc=com".to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct LdapConfigMasked {
    pub enabled: bool,
    pub server_url: String,
    pub bind_dn: String,
    pub bind_password: String,
    pub search_base: String,
    pub user_filter: String,
    pub tls: bool,
    pub admin_group: String,
    pub operator_group: String,
}

impl From<&LdapConfig> for LdapConfigMasked {
    fn from(cfg: &LdapConfig) -> Self {
        Self {
            enabled: cfg.enabled,
            server_url: cfg.server_url.clone(),
            bind_dn: cfg.bind_dn.clone(),
            bind_password: if cfg.bind_password.is_empty() {
                String::new()
            } else {
                "***".into()
            },
            search_base: cfg.search_base.clone(),
            user_filter: cfg.user_filter.clone(),
            tls: cfg.tls,
            admin_group: cfg.admin_group.clone(),
            operator_group: cfg.operator_group.clone(),
        }
    }
}

pub fn load_config() -> Result<LdapConfig, String> {
    let path = ldap_config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            serde_json::from_str(&content).map_err(|e| format!("LDAP config is invalid: {e}"))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LdapConfig::default()),
        Err(error) => Err(format!("Cannot read LDAP config: {error}")),
    }
}

pub fn save_config(cfg: &LdapConfig) -> Result<(), String> {
    let path = ldap_config_path();
    let json = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
    crate::private_storage::write(&path, &json).map_err(|e| format!("Cannot save LDAP config: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_masked_config_hides_password() {
        let cfg = LdapConfig {
            bind_password: "supersecret".into(),
            ..LdapConfig::default()
        };
        let masked = LdapConfigMasked::from(&cfg);
        assert_eq!(masked.bind_password, "***");
    }

    #[test]
    fn test_default_config_values() {
        let cfg = LdapConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.server_url, "ldap://localhost:389");
        assert_eq!(cfg.user_filter, "(uid={username})");
    }
}
