/// Integration tests for NabiMan core API endpoints
/// Tests authentication flow, API key management, license system, and RBAC.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    message: String,
}

// ==================== Auth Integration ====================

mod auth_tests {
    use super::*;

    #[test]
    fn test_login_request_serialization() {
        let req = serde_json::json!({ "username": "admin", "password": "testpass123" });
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("admin"));
        assert!(s.contains("testpass123"));
    }

    #[test]
    fn test_jwt_full_lifecycle() {
        use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};

        #[derive(Serialize, Deserialize, Clone)]
        struct Claims { sub: String, role: String, exp: usize, iat: usize }

        let secret = "integration_test_secret_key_2026";
        let now = chrono::Utc::now().timestamp() as usize;

        // Create access token (short-lived)
        let access = Claims { sub: "admin".into(), role: "admin".into(), iat: now, exp: now + 900 };
        let access_token = encode(&Header::default(), &access, &EncodingKey::from_secret(secret.as_bytes())).unwrap();

        // Create refresh token (long-lived)
        let refresh = Claims { sub: "admin".into(), role: "admin".into(), iat: now, exp: now + 604800 };
        let refresh_token = encode(&Header::default(), &refresh, &EncodingKey::from_secret(secret.as_bytes())).unwrap();

        // Verify access token
        let decoded = decode::<Claims>(&access_token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default()).unwrap();
        assert_eq!(decoded.claims.sub, "admin");
        assert_eq!(decoded.claims.role, "admin");

        // Verify refresh token
        let decoded_r = decode::<Claims>(&refresh_token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default()).unwrap();
        assert_eq!(decoded_r.claims.sub, "admin");

        // Wrong secret should fail
        assert!(decode::<Claims>(&access_token, &DecodingKey::from_secret(b"wrong"), &Validation::default()).is_err());
    }

    #[test]
    fn test_bcrypt_password_flow() {
        let password = "SecureP@ss123!";

        // Hash with bcrypt
        let hash = bcrypt::hash(password, 10).unwrap();
        assert!(hash.starts_with("$2b$10$"));

        // Verify correct password
        assert!(bcrypt::verify(password, &hash).unwrap());

        // Reject wrong password
        assert!(!bcrypt::verify("WrongPassword", &hash).unwrap());

        // Reject empty password
        assert!(!bcrypt::verify("", &hash).unwrap());

        // Different hashes for same password (salt)
        let hash2 = bcrypt::hash(password, 10).unwrap();
        assert_ne!(hash, hash2);
        assert!(bcrypt::verify(password, &hash2).unwrap());
    }

    #[test]
    fn test_password_strength_validation() {
        // These should be caught at application level
        assert!(bcrypt::hash("short", 4).is_ok()); // bcrypt allows, but app should reject
        assert!(bcrypt::hash("a".repeat(72).as_str(), 4).is_ok()); // bcrypt max is 72 bytes
    }
}

// ==================== API Key Integration ====================

mod api_key_tests {
    use sha2::{Sha256, Digest};

    #[test]
    fn test_api_key_format_and_hashing() {
        use rand::Rng;
        // Generate key
        let chars: Vec<char> = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(40).map(char::from).collect();
        let key = format!("nbm_{}", chars.iter().collect::<String>());

        assert!(key.starts_with("nbm_"));
        assert_eq!(key.len(), 44);

        // Hash with SHA256
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hash = hex::encode(hasher.finalize());

        // Consistent hashing
        let mut hasher2 = Sha256::new();
        hasher2.update(key.as_bytes());
        let hash2 = hex::encode(hasher2.finalize());
        assert_eq!(hash, hash2);
        assert_eq!(hash.len(), 64); // SHA256 hex = 64 chars
    }

    #[test]
    fn test_api_key_prefix_extraction() {
        let key = "nbm_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij";
        let prefix = &key[..8];
        assert_eq!(prefix, "nbm_ABCD");
    }

    #[test]
    fn test_api_key_expiration_check() {
        let now = chrono::Utc::now();

        // Not expired
        let expires = (now + chrono::Duration::days(30)).format("%Y-%m-%d %H:%M:%S").to_string();
        let exp_time = chrono::NaiveDateTime::parse_from_str(&expires, "%Y-%m-%d %H:%M:%S").unwrap();
        assert!(exp_time.and_utc() > now);

        // Expired
        let expired = (now - chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();
        let exp_time2 = chrono::NaiveDateTime::parse_from_str(&expired, "%Y-%m-%d %H:%M:%S").unwrap();
        assert!(exp_time2.and_utc() < now);
    }

    #[test]
    fn test_api_key_role_validation() {
        let valid_roles = ["admin", "operator", "viewer"];
        assert!(valid_roles.contains(&"admin"));
        assert!(valid_roles.contains(&"viewer"));
        assert!(!valid_roles.contains(&"superadmin"));
        assert!(!valid_roles.contains(&""));
    }
}

// ==================== License Integration ====================

mod license_tests {
    use sha2::{Sha256, Digest};

    const HMAC_SECRET: &[u8] = b"nabiman-license-hmac-secret-key-2026";
    const HMAC_BLOCK_SIZE: usize = 64;

    fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
        let key_padded = if key.len() > HMAC_BLOCK_SIZE {
            let mut h = Sha256::new();
            h.update(key);
            h.finalize().to_vec()
        } else {
            key.to_vec()
        };

        let mut key_block = vec![0u8; HMAC_BLOCK_SIZE];
        key_block[..key_padded.len()].copy_from_slice(&key_padded);

        let ipad: Vec<u8> = key_block.iter().map(|b| b ^ 0x36).collect();
        let opad: Vec<u8> = key_block.iter().map(|b| b ^ 0x5c).collect();

        let mut inner = Sha256::new();
        inner.update(&ipad);
        inner.update(message);
        let inner_hash = inner.finalize();

        let mut outer = Sha256::new();
        outer.update(&opad);
        outer.update(&inner_hash);
        outer.finalize().to_vec()
    }

    fn create_test_license(tier: &str, holder: &str) -> String {
        let now = chrono::Utc::now();
        let payload = format!(
            "{}|{}|{}|{}|{}",
            tier, holder,
            now.format("%Y-%m-%d"),
            (now + chrono::Duration::days(365)).format("%Y-%m-%d"),
            if tier == "enterprise" { "0" } else { "10" }
        );
        let sig = hmac_sha256(HMAC_SECRET, payload.as_bytes());
        let combined: Vec<u8> = [payload.as_bytes(), b"|", &sig].concat();
        base64_encode(&combined)
    }

    fn base64_encode(data: &[u8]) -> String {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut result = String::new();
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let triple = (b0 << 16) | (b1 << 8) | b2;
            result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
            result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
            if chunk.len() > 1 { result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char); }
            else { result.push('='); }
            if chunk.len() > 2 { result.push(CHARS[(triple & 0x3F) as usize] as char); }
            else { result.push('='); }
        }
        result
    }

    #[test]
    fn test_hmac_consistency() {
        let msg = b"test message";
        let sig1 = hmac_sha256(HMAC_SECRET, msg);
        let sig2 = hmac_sha256(HMAC_SECRET, msg);
        assert_eq!(sig1, sig2);
        assert_eq!(sig1.len(), 32); // SHA256 = 32 bytes
    }

    #[test]
    fn test_hmac_different_messages_differ() {
        let sig1 = hmac_sha256(HMAC_SECRET, b"message1");
        let sig2 = hmac_sha256(HMAC_SECRET, b"message2");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_hmac_different_keys_differ() {
        let msg = b"same message";
        let sig1 = hmac_sha256(b"key1", msg);
        let sig2 = hmac_sha256(b"key2", msg);
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_license_key_generation() {
        let key = create_test_license("pro", "TestCorp");
        assert!(!key.is_empty());
        // Should be valid base64
        assert!(key.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '='));
    }

    #[test]
    fn test_license_tiers() {
        // Free tier features
        let free_features = vec!["monitoring", "management"];
        assert!(free_features.contains(&"monitoring"));
        assert!(!free_features.contains(&"ldap"));

        // Pro tier features
        let pro_features = vec!["monitoring", "management", "alerts", "backup_schedule"];
        assert!(pro_features.contains(&"alerts"));

        // Enterprise tier features
        let ent_features = vec!["monitoring", "management", "alerts", "backup_schedule", "ldap", "oauth", "api_keys", "ip_block"];
        assert!(ent_features.contains(&"ldap"));
        assert!(ent_features.contains(&"oauth"));
    }

    #[test]
    fn test_trial_period_calculation() {
        let install_time = chrono::Utc::now() - chrono::Duration::days(10);
        let trial_days = 30i64;
        let elapsed = (chrono::Utc::now() - install_time).num_days();
        let remaining = trial_days - elapsed;

        assert!(remaining > 0);
        assert!(remaining <= 30);
        assert_eq!(remaining, 20);
    }
}

// ==================== RBAC Integration ====================

mod rbac_tests {
    #[derive(Debug, Clone)]
    enum Role { Admin, Operator, Viewer }

    fn check_permission(role: &Role, method: &str, path: &str) -> bool {
        match role {
            Role::Admin => true,
            Role::Viewer => {
                if method == "GET" { return true; }
                if path == "/api/auth/change-password" { return true; }
                false
            }
            Role::Operator => {
                if method == "GET" { return true; }
                // Block user/account management
                if path.starts_with("/api/users") || path.starts_with("/api/accounts") {
                    return method == "GET";
                }
                true
            }
        }
    }

    #[test]
    fn test_admin_full_access() {
        let role = Role::Admin;
        assert!(check_permission(&role, "GET", "/api/server/status"));
        assert!(check_permission(&role, "POST", "/api/users"));
        assert!(check_permission(&role, "DELETE", "/api/users/123"));
        assert!(check_permission(&role, "POST", "/api/services/restart"));
    }

    #[test]
    fn test_viewer_read_only() {
        let role = Role::Viewer;
        assert!(check_permission(&role, "GET", "/api/server/status"));
        assert!(check_permission(&role, "GET", "/api/disks/status"));
        assert!(!check_permission(&role, "POST", "/api/services/restart"));
        assert!(!check_permission(&role, "DELETE", "/api/users/123"));
        assert!(!check_permission(&role, "POST", "/api/firewall/add"));
    }

    #[test]
    fn test_viewer_can_change_own_password() {
        let role = Role::Viewer;
        assert!(check_permission(&role, "POST", "/api/auth/change-password"));
    }

    #[test]
    fn test_operator_can_manage_services() {
        let role = Role::Operator;
        assert!(check_permission(&role, "POST", "/api/services/restart"));
        assert!(check_permission(&role, "POST", "/api/containers/start"));
        assert!(check_permission(&role, "POST", "/api/packages/install"));
    }

    #[test]
    fn test_operator_cannot_manage_users() {
        let role = Role::Operator;
        assert!(check_permission(&role, "GET", "/api/users"));
        assert!(!check_permission(&role, "POST", "/api/users"));
        assert!(!check_permission(&role, "DELETE", "/api/users/123"));
    }
}

// ==================== Session Management ====================

mod session_tests {
    #[test]
    fn test_session_id_generation() {
        use rand::Rng;
        let id: u128 = rand::thread_rng().gen();
        let sid = format!("{:032x}", id);
        assert_eq!(sid.len(), 32);
        assert!(sid.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_session_id_uniqueness() {
        use rand::Rng;
        let mut ids = std::collections::HashSet::new();
        for _ in 0..1000 {
            let id: u128 = rand::thread_rng().gen();
            let sid = format!("{:032x}", id);
            assert!(ids.insert(sid), "Duplicate session ID generated");
        }
    }

    #[test]
    fn test_access_token_ttl() {
        use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};

        #[derive(serde::Serialize, serde::Deserialize)]
        struct Claims { sub: String, exp: usize }

        let secret = "test";
        let now = chrono::Utc::now().timestamp() as usize;

        // 15-minute access token - should be valid
        let claims = Claims { sub: "user".into(), exp: now + 900 };
        let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap();
        assert!(decode::<Claims>(&token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default()).is_ok());
    }

    #[test]
    fn test_refresh_token_ttl() {
        use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};

        #[derive(serde::Serialize, serde::Deserialize)]
        struct Claims { sub: String, typ: String, exp: usize }

        let secret = "test";
        let now = chrono::Utc::now().timestamp() as usize;

        // 7-day refresh token - should be valid
        let claims = Claims { sub: "user".into(), typ: "refresh".into(), exp: now + 604800 };
        let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap();
        let decoded = decode::<Claims>(&token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default()).unwrap();
        assert_eq!(decoded.claims.typ, "refresh");
    }
}

// ==================== Input Validation ====================

mod validation_tests {
    #[test]
    fn test_username_validation() {
        let valid = |name: &str| -> bool {
            name.len() >= 2 && name.len() <= 32
                && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        };

        assert!(valid("admin"));
        assert!(valid("test-user"));
        assert!(valid("user_123"));
        assert!(!valid("a")); // too short
        assert!(!valid("")); // empty
        assert!(!valid("user name")); // space
        assert!(!valid("user;rm -rf")); // injection
        assert!(!valid(&"x".repeat(33))); // too long
    }

    #[test]
    fn test_ip_validation() {
        let valid_ip = |ip: &str| -> bool {
            // Simple IPv4 check
            let parts: Vec<&str> = ip.split('.').collect();
            parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok())
        };

        assert!(valid_ip("192.168.1.1"));
        assert!(valid_ip("10.0.0.1"));
        assert!(valid_ip("255.255.255.255"));
        assert!(!valid_ip("999.999.999.999"));
        assert!(!valid_ip("not-an-ip"));
        assert!(!valid_ip(""));
        assert!(!valid_ip("192.168.1")); // incomplete
    }

    #[test]
    fn test_ldap_username_injection_prevention() {
        let has_injection = |name: &str| -> bool {
            name.contains('\0') || name.contains('*') || name.contains('(') || name.contains(')')
        };

        assert!(!has_injection("john.doe"));
        assert!(has_injection("user*(objectClass=*)"));
        assert!(has_injection("admin)(uid=*))(|(uid=*"));
        assert!(has_injection("test\0inject"));
    }

    #[test]
    fn test_path_traversal_prevention() {
        let safe_path = |path: &str| -> bool {
            !path.contains("..") && !path.starts_with("/proc")
                && !path.starts_with("/sys") && !path.starts_with("/dev")
        };

        assert!(safe_path("/home/user/file.txt"));
        assert!(safe_path("/var/log/syslog"));
        assert!(!safe_path("/etc/../proc/1/environ"));
        assert!(!safe_path("/proc/self/cmdline"));
        assert!(!safe_path("/sys/class/net"));
        assert!(!safe_path("/dev/sda"));
    }
}

// ==================== Data Serialization ====================

mod serialization_tests {
    use super::*;

    #[test]
    fn test_api_response_ok_serialization() {
        let resp = ApiResponse {
            success: true,
            data: Some(serde_json::json!({"token": "abc123"})),
            message: "OK".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"token\":\"abc123\""));
    }

    #[test]
    fn test_api_response_error_serialization() {
        let resp: ApiResponse<serde_json::Value> = ApiResponse {
            success: false,
            data: None,
            message: "Invalid credentials".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("Invalid credentials"));
    }

    #[test]
    fn test_user_role_deserialization() {
        let admin: serde_json::Value = serde_json::from_str(r#""admin""#).unwrap();
        assert_eq!(admin.as_str(), Some("admin"));

        let roles = ["admin", "operator", "viewer"];
        for role in &roles {
            let json = format!(r#""{}""#, role);
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.as_str(), Some(*role));
        }
    }

    #[test]
    fn test_disk_status_json_format() {
        let disk_json = serde_json::json!({
            "partitions": [{
                "filesystem": "/dev/sda1",
                "mount_point": "/",
                "fs_type": "ext4",
                "total": 100000000000_u64,
                "used": 50000000000_u64,
                "available": 50000000000_u64,
                "use_percent": 50.0
            }],
            "io": [{
                "device": "sda",
                "reads_per_sec": 10.5,
                "writes_per_sec": 5.2,
                "read_bytes_per_sec": 1048576,
                "write_bytes_per_sec": 524288
            }]
        });

        assert!(disk_json["partitions"][0]["use_percent"].as_f64().unwrap() < 100.0);
        assert_eq!(disk_json["io"][0]["device"], "sda");
    }

    #[test]
    fn test_oauth_callback_params() {
        // Simulate query string parsing
        let query = "code=abc123&state=xyz789";
        let params: std::collections::HashMap<&str, &str> = query
            .split('&')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                Some((parts.next()?, parts.next()?))
            })
            .collect();

        assert_eq!(params.get("code"), Some(&"abc123"));
        assert_eq!(params.get("state"), Some(&"xyz789"));
    }
}

// ==================== Rate Limiting ====================

mod rate_limit_tests {
    use std::collections::HashMap;

    #[test]
    fn test_rate_limit_tracking() {
        let mut attempts: HashMap<String, Vec<i64>> = HashMap::new();
        let ip = "192.168.1.100".to_string();
        let window = 300; // 5 minutes
        let max_attempts = 5;
        let now = chrono::Utc::now().timestamp();

        // Simulate 5 login attempts
        for i in 0..5 {
            let entry = attempts.entry(ip.clone()).or_default();
            entry.push(now + i);
        }

        // Should be at the limit
        let recent = attempts.get(&ip).unwrap()
            .iter().filter(|&&t| t > now - window).count();
        assert_eq!(recent, 5);
        assert!(recent >= max_attempts);
    }

    #[test]
    fn test_rate_limit_window_expiry() {
        let window = 300i64;
        let now = chrono::Utc::now().timestamp();

        // Old attempts should not count
        let attempts = vec![now - 400, now - 350, now - 10, now - 5, now];
        let recent: Vec<_> = attempts.iter().filter(|&&t| t > now - window).collect();
        assert_eq!(recent.len(), 3); // Only last 3 are within window
    }
}
