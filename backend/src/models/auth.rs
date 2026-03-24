use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum UserRole {
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "operator")]
    Operator,
    #[serde(rename = "viewer")]
    Viewer,
}

impl UserRole {
    pub fn as_str(&self) -> &str {
        match self { Self::Admin => "admin", Self::Operator => "operator", Self::Viewer => "viewer" }
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NabimanUser {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    pub created_at: String,
    #[serde(default)]
    pub last_login: Option<String>,
    #[serde(default)]
    pub totp_enabled: bool,
    #[serde(default, skip_serializing)]
    pub totp_secret: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: UserRole,
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub password: Option<String>,
    pub role: Option<UserRole>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: Option<String>,
    pub password: String,
}
