use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct RemoteServer {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth_method: String,
    pub tags: Vec<String>,
    pub memo: String,
    pub created_at: String,
    #[serde(default)]
    pub last_checked: String,
    #[serde(default)]
    pub status: String,
    /// Password encrypted at rest (see `secret_store`). Written to the data
    /// file, but blanked before any API response — only `has_password` leaves
    /// the server.
    #[serde(default)]
    pub password_enc: String,
    /// Derived from `password_enc` on every save/list so the UI can show
    /// whether a password is stored without ever receiving it.
    #[serde(default)]
    pub has_password: bool,
    #[serde(default)]
    pub host_trust: Option<HostKeyTrust>,
    /// Approval generation survives revocation to prevent an ABA stale update.
    #[serde(default)]
    pub host_trust_version: String,
    /// How a terminal reaches it: `ssh`, `telnet` or `none`. Records saved
    /// before this existed are SSH servers.
    #[serde(default = "default_terminal")]
    pub terminal: String,
    /// How its files are reached: `sftp`, `ftps`, `ftp` or `none`.
    #[serde(default = "default_files")]
    pub files: String,
    /// The FTP control port; SSH and Telnet use `port`.
    #[serde(default = "default_ftp_port")]
    pub ftp_port: u16,
    /// An FTPS certificate the operator verified by hand (`SHA256:…`), for a
    /// server whose certificate no public authority vouches for. Empty means
    /// the certificate must be valid for the host name.
    #[serde(default)]
    pub tls_fingerprint: String,
    /// FTPS that speaks TLS from the first byte (usually port 990) instead of
    /// switching to it with AUTH TLS.
    #[serde(default)]
    pub ftps_implicit: bool,
    /// The server speaks only the old SSH algorithms (legacy key exchange,
    /// host key or ciphers). Set when its host key is approved; lets the UI
    /// mark it and is informational — the SSH options always allow the old
    /// algorithms as a low-priority fallback regardless.
    #[serde(default)]
    pub legacy_ssh: bool,
    /// The remote-only member who registered it; empty for the servers the
    /// administrators keep (see `member_servers`).
    #[serde(default)]
    pub owner_id: String,
}

fn default_terminal() -> String {
    "ssh".into()
}
fn default_files() -> String {
    "sftp".into()
}
fn default_ftp_port() -> u16 {
    21
}

impl RemoteServer {
    /// Some part of it runs over SSH, so its host key has to be approved.
    pub fn uses_ssh(&self) -> bool {
        self.terminal == "ssh" || self.files == "sftp"
    }
    /// FTP or FTPS, which have a connection of their own.
    pub fn uses_ftp(&self) -> bool {
        self.files == "ftp" || self.files == "ftps"
    }
    /// A remote-only member's own server.
    pub fn member_owned(&self) -> bool {
        !self.owner_id.is_empty()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RemoteServerStatus {
    pub id: String,
    pub name: String,
    pub host: String,
    pub status: String,
    pub hostname: String,
    pub os: String,
    pub uptime: String,
    pub cpu_usage: String,
    pub memory: String,
    pub disk: String,
    pub load: String,
    pub checked_at: String,
    /// One line per way of reaching it that was tried, such as SSH and FTP.
    #[serde(default)]
    pub checks: Vec<ProtocolCheck>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProtocolCheck {
    pub protocol: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Deserialize)]
pub struct AddRemoteServerRequest {
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub auth_method: Option<String>,
    pub tags: Option<Vec<String>>,
    pub memo: Option<String>,
    /// Optional SSH password, stored encrypted.
    pub password: Option<String>,
    pub terminal: Option<String>,
    pub files: Option<String>,
    pub ftp_port: Option<u16>,
    pub tls_fingerprint: Option<String>,
    pub ftps_implicit: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateRemoteServerRequest {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub auth_method: Option<String>,
    pub tags: Option<Vec<String>>,
    pub memo: Option<String>,
    /// `Some("")` clears the stored password, `Some(pw)` replaces it,
    /// `None` leaves it untouched.
    pub password: Option<String>,
    pub terminal: Option<String>,
    pub files: Option<String>,
    pub ftp_port: Option<u16>,
    pub tls_fingerprint: Option<String>,
    pub ftps_implicit: Option<bool>,
}

#[derive(Deserialize)]
pub struct RemoteExecRequest {
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SshKeyInfo {
    pub exists: bool,
    pub key_path: String,
    pub public_key: Option<String>,
    pub fingerprint: Option<String>,
}

#[derive(Deserialize)]
pub struct DeployKeyRequest {
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HostKeyTrust {
    pub public_key: String,
    pub fingerprint: String,
    pub verified_by: String,
    pub verified_at: String,
    pub reason: String,
}
