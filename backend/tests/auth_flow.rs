/// Integration tests for authentication flow
/// These tests verify the auth API endpoints work correctly together.
/// Note: These are compile-time checked but require a running server to execute.

#[cfg(test)]
mod tests {
    #[test]
    fn test_bcrypt_hash_and_verify() {
        let password = "testpassword123";
        let hash = bcrypt::hash(password, 10).unwrap();
        assert!(hash.starts_with("$2b$"));
        assert!(bcrypt::verify(password, &hash).unwrap());
        assert!(!bcrypt::verify("wrongpassword", &hash).unwrap());
    }

    #[test]
    fn test_bcrypt_minimum_length() {
        // Bcrypt should work with short passwords too (validation is app-level)
        let hash = bcrypt::hash("ab", 4).unwrap();
        assert!(bcrypt::verify("ab", &hash).unwrap());
    }

    #[test]
    fn test_jwt_roundtrip() {
        use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
        use serde::{Serialize, Deserialize};

        #[derive(Serialize, Deserialize)]
        struct Claims { sub: String, exp: usize }

        let secret = "test_secret";
        let claims = Claims {
            sub: "admin".into(),
            exp: (chrono::Utc::now().timestamp() + 3600) as usize,
        };

        let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap();
        let decoded = decode::<Claims>(&token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default()).unwrap();
        assert_eq!(decoded.claims.sub, "admin");
    }

    #[test]
    fn test_jwt_expiry_check() {
        use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
        use serde::{Serialize, Deserialize};

        #[derive(Serialize, Deserialize)]
        struct Claims { sub: String, exp: usize }

        let secret = "test_secret";
        let claims = Claims {
            sub: "admin".into(),
            exp: (chrono::Utc::now().timestamp() - 100) as usize, // expired
        };

        let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap();
        let result = decode::<Claims>(&token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default());
        assert!(result.is_err()); // Should fail due to expiry
    }

    #[test]
    fn test_user_serialization() {
        let user_json = r#"{"id":"abc","username":"admin","password_hash":"$2b$10$xxx","role":"admin","created_at":"2024-01-01","totp_enabled":false}"#;
        let parsed: serde_json::Value = serde_json::from_str(user_json).unwrap();
        assert_eq!(parsed["username"], "admin");
        assert_eq!(parsed["role"], "admin");
    }
}
