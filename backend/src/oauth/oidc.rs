use serde::Deserialize;
use super::config::OAuthConfig;

pub fn generate_state() -> String {
    use rand::Rng;
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

pub fn url_encode(s: &str) -> String {
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

#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
}

pub fn exchange_code(cfg: &OAuthConfig, code: &str) -> Result<TokenResponse, String> {
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

pub fn fetch_userinfo(url: &str, access_token: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
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

pub fn determine_role(cfg: &OAuthConfig, info: &serde_json::Map<String, serde_json::Value>) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

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
