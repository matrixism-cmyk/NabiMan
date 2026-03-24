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
