use super::bootstrap::*;
use super::*;

#[test]
fn bootstrap_rejects_published_and_short_passwords() {
    assert!(validate_bootstrap_password("nabiman", true).is_err());
    assert!(validate_bootstrap_password("changeme", true).is_err());
    assert!(validate_bootstrap_password("short-pass", true).is_err());
    assert!(validate_bootstrap_password("a-unique-initial-password", true).is_ok());
    let published_hash = bcrypt::hash("nabiman", 4).unwrap();
    assert!(validate_bootstrap_password(&published_hash, false).is_err());
}

#[test]
fn jwt_secret_is_private_and_stable() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("nabiman-jwt-test-{}", uuid::Uuid::new_v4()));
    let path = dir.join("jwt_secret");
    let first = load_or_create_jwt_secret(&path);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(load_or_create_jwt_secret(&path), first);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::remove_dir_all(dir).unwrap();
}

// Test-only 24h token minter (production code uses jwt_sessions::create_token_pair).
fn create_jwt(secret: &str, username: &str, role: &str) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: username.into(),
        role: role.into(),
        iat: now,
        exp: now + 86400,
        sid: "test-session".into(),
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

#[test]
fn test_jwt_create_and_verify() {
    let secret = "test_secret_key_for_jwt_12345678";
    let token = create_jwt(secret, "admin", "admin").unwrap();
    assert!(!token.is_empty());
    let claims = verify_jwt(secret, &token).unwrap();
    assert_eq!(claims.sub, "admin");
    assert_eq!(claims.role, "admin");
}

#[test]
fn test_jwt_expired_rejected() {
    let secret = "test_secret";
    // Create a token that's already expired
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: "u".into(),
        role: "admin".into(),
        iat: now - 200,
        exp: now - 100,
        sid: "test-session".into(),
    };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();
    assert!(verify_jwt(secret, &token).is_none());
}

#[test]
fn test_jwt_wrong_secret_rejected() {
    let token = create_jwt("secret1", "admin", "admin").unwrap();
    assert!(verify_jwt("secret2", &token).is_none());
}

#[test]
fn test_check_auth_token() {
    let secret = "test123";
    let token = create_jwt(secret, "admin", "admin").unwrap();
    let sessions = crate::jwt_sessions::new_session_store();
    crate::jwt_sessions::register_session(&sessions, "test-session", "admin", "admin", "127.0.0.1");
    assert!(check_auth_token(secret, &token, &sessions));
    sessions.lock().unwrap().remove("test-session");
    assert!(!check_auth_token(secret, &token, &sessions));
    assert!(!check_auth_token(secret, "garbage", &sessions));
}

#[test]
fn test_decode_claims_unverified() {
    let token = create_jwt("any_secret", "testuser", "viewer").unwrap();
    let data = decode_claims_unverified(&token).unwrap();
    assert_eq!(data.claims.sub, "testuser");
    assert_eq!(data.claims.role, "viewer");
}
#[test]
fn public_login_discovery_does_not_expose_admin_configuration() {
    let sessions = crate::jwt_sessions::new_session_store();
    let users = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let secret = std::sync::Arc::new("test-secret".to_string());
    for path in [
        "/api/auth/oauth/provider",
        "/api/auth/ldap/provider",
        "/api/auth/oauth/authorize",
    ] {
        let request = actix_web::test::TestRequest::get()
            .uri(path)
            .to_srv_request();
        assert!(matches!(
            check_auth(&request, &secret, &sessions, &users),
            AuthDecision::Allow
        ));
        let request = actix_web::test::TestRequest::post()
            .uri(path)
            .to_srv_request();
        assert!(matches!(
            check_auth(&request, &secret, &sessions, &users),
            AuthDecision::Unauthorized
        ));
    }
    for path in ["/api/auth/oauth/config", "/api/auth/ldap/config"] {
        let request = actix_web::test::TestRequest::get()
            .uri(path)
            .to_srv_request();
        assert!(matches!(
            check_auth(&request, &secret, &sessions, &users),
            AuthDecision::Unauthorized
        ));
    }
}

#[test]
fn refresh_token_cannot_be_used_as_an_access_token() {
    let secret = "fixture-token-purpose-secret";
    let sessions = crate::jwt_sessions::new_session_store();
    crate::jwt_sessions::register_session(&sessions, "sid", "member", "immutable-id", "127.0.0.1");
    let (access, refresh) =
        crate::jwt_sessions::create_token_pair(secret, "member", "admin", "sid").unwrap();
    assert!(check_auth_token(secret, &access, &sessions));
    assert!(!check_auth_token(secret, &refresh, &sessions));
}
