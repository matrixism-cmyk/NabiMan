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
/// set by an administrator. The administrators may always do all of it. The
/// defaults keep what members could already do, and grant no sharing.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RemotePermissions {
    /// Share links to their terminal sessions (세션 공유).
    #[serde(default)]
    pub share_sessions: bool,
    /// Share links to a server's files, and the file panel in a session's link.
    #[serde(default)]
    pub share_files: bool,
    /// Their links may last at most this many days (0: no limit, permanent allowed).
    #[serde(default)]
    pub share_max_days: u32,
    /// Their links must carry a password.
    #[serde(default)]
    pub share_require_password: bool,
    /// Their links only show: no typing, files at most browsed.
    #[serde(default)]
    pub share_view_only: bool,
    /// Voice dictation into the terminal (the administrators' speech service).
    #[serde(default = "yes")]
    pub dictation: bool,
    /// Registering servers of their own, and editing them.
    #[serde(default = "yes")]
    pub register_servers: bool,
    /// Changing files on servers an administrator assigned them (upload,
    /// delete, rename, permissions). Off: browse and download only.
    #[serde(default = "yes")]
    pub assigned_file_write: bool,
    /// Copying files from one server to another.
    #[serde(default = "yes")]
    pub copy_between_servers: bool,
    /// Sessions kept alive with no idle limit (세션 유지).
    #[serde(default = "yes")]
    pub keep_sessions: bool,
    /// At most this many sessions at once (0: no limit).
    #[serde(default)]
    pub max_sessions: u32,
    /// A session left idle this long ends (0: no limit).
    #[serde(default)]
    pub max_idle_minutes: u32,
    /// At most this many servers of their own (0: no limit).
    #[serde(default)]
    pub max_servers: u32,
    /// One upload may be at most this many megabytes (0: no limit).
    #[serde(default)]
    pub max_upload_mb: u32,
}

fn yes() -> bool {
    true
}

impl Default for RemotePermissions {
    fn default() -> Self {
        Self {
            share_sessions: false,
            share_files: false,
            share_max_days: 0,
            share_require_password: false,
            share_view_only: false,
            dictation: true,
            register_servers: true,
            assigned_file_write: true,
            copy_between_servers: true,
            keep_sessions: true,
            max_sessions: 0,
            max_idle_minutes: 0,
            max_servers: 0,
            max_upload_mb: 0,
        }
    }
}

impl RemotePermissions {
    /// Whether sharing that was allowed before is no longer: the links it
    /// made must not come back if it is allowed again.
    pub fn narrowed_from(&self, before: &Self) -> bool {
        (before.share_sessions && !self.share_sessions) || (before.share_files && !self.share_files)
    }
}

/// When and from where an account may be used, and how it signs in. Set by
/// the super administrator for any account but their own kind.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct AccessRules {
    /// The account stops working at this moment (RFC 3339).
    #[serde(default)]
    pub expires_at: Option<String>,
    /// Two-step sign-in is required of this account.
    #[serde(default)]
    pub require_mfa: bool,
    /// Signing in only from these addresses or networks (`203.0.113.7`,
    /// `203.0.113.0/24`); empty: from anywhere.
    #[serde(default)]
    pub allowed_networks: Vec<String>,
    /// Only within these hours, Korean time; none: at any time.
    #[serde(default)]
    pub allowed_hours: Option<HourWindow>,
}

/// Hours of the day (Korean time), from `start` up to `end` (exclusive; an end
/// before the start runs past midnight), on the given weekdays (0 Monday to
/// 6 Sunday; empty: every day).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HourWindow {
    pub start: u8,
    pub end: u8,
    #[serde(default)]
    pub days: Vec<u8>,
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
    #[serde(default)]
    pub access: AccessRules,
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
    pub access: Option<AccessRules>,
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
    /// Approved, not suspended, and (for anyone but an administrator) not past
    /// the end of its time.
    pub fn active(&self) -> bool {
        self.status == AccountStatus::Active && (self.role == UserRole::Admin || !self.access.expired())
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
