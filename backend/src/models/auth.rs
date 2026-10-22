use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum UserRole {
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "operator")]
    Operator,
    #[serde(rename = "viewer")]
    Viewer,
    /// Only the remote workspace, on servers the member registered.
    #[serde(rename = "remote")]
    Remote,
}

impl UserRole {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Admin => "admin",
            Self::Operator => "operator",
            Self::Viewer => "viewer",
            Self::Remote => "remote",
        }
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct MfaState {
    pub recovery_hashes: Vec<String>,
    pub last_used_step: Option<u64>,
    pub pending_expires_at: Option<i64>,
    pub revision: String,
}

/// What a remote-only member may do beyond working on their servers, each
/// granted by an administrator. The administrators may always do all of it.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RemotePermissions {
    /// Share links to their terminal sessions (세션 공유).
    #[serde(default)]
    pub share_sessions: bool,
    /// Share links to a server's files, and the file panel in a session's link.
    #[serde(default)]
    pub share_files: bool,
}

impl RemotePermissions {
    /// Whether anything granted before is no longer granted.
    pub fn narrowed_from(&self, before: &Self) -> bool {
        (before.share_sessions && !self.share_sessions) || (before.share_files && !self.share_files)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NabimanUser {
    pub id: String,
    pub username: String,
    /// Durable generation for delegated terminal capabilities; omitted from APIs.
    #[serde(default, skip_serializing)]
    pub share_authorization_version: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    #[serde(default)]
    pub status: AccountStatus,
    #[serde(default)]
    pub memberships: Vec<Membership>,
    #[serde(default)]
    pub requested_memberships: Vec<Membership>,
    #[serde(default)]
    pub scope_restricted: bool,
    pub created_at: String,
    #[serde(default)]
    pub last_login: Option<String>,
    #[serde(default)]
    pub totp_enabled: bool,
    #[serde(default, skip_serializing)]
    pub totp_secret: Option<String>,
    #[serde(default, skip_serializing)]
    pub mfa: MfaState,
    #[serde(default)]
    pub remote_permissions: RemotePermissions,
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
    pub remote_permissions: Option<RemotePermissions>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: Option<String>,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    #[default]
    Active,
    Pending,
    Rejected,
    Suspended,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Server,
    Tenant,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Membership {
    pub kind: ScopeKind,
    pub scope_id: String,
    pub role: UserRole,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub allow_exec: Option<bool>,
    #[serde(default)]
    pub expires_at: Option<String>,
}
fn enabled_by_default() -> bool {
    true
}
impl Membership {
    pub fn accessible(&self) -> bool {
        self.enabled
            && self
                .expires_at
                .as_ref()
                .map(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .map(|t| t > chrono::Utc::now())
                        .unwrap_or(false)
                })
                .unwrap_or(true)
    }
}

impl NabimanUser {
    pub fn active(&self) -> bool {
        self.status == AccountStatus::Active
    }
    pub fn super_admin(&self) -> bool {
        self.active() && self.role == UserRole::Admin
    }
    pub fn restricted(&self) -> bool {
        self.scope_restricted || !self.memberships.is_empty()
    }
    pub fn scope_role(&self, kind: &ScopeKind, id: &str) -> Option<&UserRole> {
        self.memberships
            .iter()
            .find(|m| &m.kind == kind && m.scope_id == id && m.accessible())
            .map(|m| &m.role)
    }
    pub fn manages(&self, kind: &ScopeKind, id: &str) -> bool {
        self.super_admin() || (self.active() && self.scope_role(kind, id) == Some(&UserRole::Admin))
    }
}
