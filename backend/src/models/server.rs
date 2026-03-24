use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerStatus {
    pub hostname: String,
    pub os: String,
    pub uptime: u64,
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub disk_total: u64,
    pub disk_used: u64,
    pub load_average: [f64; 3],
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub ip_address: String,
    pub mac_address: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub is_up: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NetworkStatus {
    pub interfaces: Vec<NetworkInterface>,
    pub open_connections: u32,
    pub dns_servers: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserAccount {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub home: String,
    pub shell: String,
    pub is_logged_in: bool,
}

#[derive(Deserialize)]
pub struct CreateAccountRequest {
    pub username: String,
    pub password: String,
    pub shell: Option<String>,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub username: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct DeleteAccountRequest {
    pub username: String,
}
