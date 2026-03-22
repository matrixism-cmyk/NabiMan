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

#[derive(Serialize, Deserialize, Clone)]
pub struct ServiceConfig {
    pub service_name: String,
    pub config_path: String,
    pub content: String,
    pub is_running: bool,
}

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    pub service_name: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TrafficSnapshot {
    pub timestamp: String,
    pub rx_bytes_per_sec: u64,
    pub tx_bytes_per_sec: u64,
    pub active_connections: u32,
    pub interface: String,
}

// --- Container models ---
#[derive(Serialize, Deserialize, Clone)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
    pub ports: String,
    pub created: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ContainerImage {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
    pub created: String,
}

#[derive(Deserialize)]
pub struct ContainerActionRequest {
    pub id: String,
}

// --- Service models ---
#[derive(Serialize, Deserialize, Clone)]
pub struct SystemService {
    pub name: String,
    pub description: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct ServiceActionRequest {
    pub name: String,
    pub action: String,
}

// --- Firewall models ---
#[derive(Serialize, Deserialize, Clone)]
pub struct FirewallStatus {
    pub backend: String,
    pub active: bool,
    pub rules: Vec<FirewallRule>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FirewallRule {
    pub number: u32,
    pub action: String,
    pub protocol: String,
    pub port: String,
    pub source: String,
    pub destination: String,
}

#[derive(Deserialize)]
pub struct FirewallAddRuleRequest {
    pub port: String,
    pub protocol: String,
    pub action: String,
}

#[derive(Deserialize)]
pub struct FirewallDeleteRuleRequest {
    pub number: u32,
}

// --- Log models ---
#[derive(Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub unit: String,
    pub priority: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct LogQueryRequest {
    pub unit: Option<String>,
    pub lines: Option<u32>,
    pub priority: Option<String>,
    pub since: Option<String>,
}

// --- Cron models ---
#[derive(Serialize, Deserialize, Clone)]
pub struct CronJob {
    pub id: u32,
    pub user: String,
    pub schedule: String,
    pub command: String,
}

#[derive(Deserialize)]
pub struct CreateCronRequest {
    pub user: String,
    pub schedule: String,
    pub command: String,
}

#[derive(Deserialize)]
pub struct DeleteCronRequest {
    pub user: String,
    pub id: u32,
}

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: "OK".to_string(),
        }
    }

    pub fn error(msg: &str) -> Self {
        Self {
            success: false,
            data: None,
            message: msg.to_string(),
        }
    }
}
