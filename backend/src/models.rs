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

// --- Config manager models (registry-based) ---
#[derive(Serialize, Deserialize, Clone)]
pub struct ServiceDefinition {
    pub id: String,
    pub display_name: String,
    pub config_paths: Vec<String>,
    #[serde(skip_deserializing)]
    pub systemd_names: Vec<String>,
    #[serde(skip_deserializing)]
    pub process_name: String,
    #[serde(skip_deserializing)]
    pub binary_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_running: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_installed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_found: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_by: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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
    pub content: String,
}

// --- Process models ---
#[derive(Serialize, Deserialize, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub user: String,
    pub cpu: f32,
    pub memory: f32,
    pub vsz: u64,
    pub rss: u64,
    pub command: String,
    pub started: String,
}

#[derive(Deserialize)]
pub struct KillProcessRequest {
    pub pid: u32,
    pub signal: Option<String>,
}

// --- Disk models ---
#[derive(Serialize, Deserialize, Clone)]
pub struct DiskPartition {
    pub filesystem: String,
    pub mount_point: String,
    pub fs_type: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub use_percent: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiskIo {
    pub device: String,
    pub reads_per_sec: f64,
    pub writes_per_sec: f64,
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiskStatus {
    pub partitions: Vec<DiskPartition>,
    pub io: Vec<DiskIo>,
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

// --- Remote server models ---
#[derive(Serialize, Deserialize, Clone)]
pub struct RemoteServer {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth_method: String, // "key" or "password"
    pub tags: Vec<String>,
    pub memo: String,
    pub created_at: String,
    #[serde(default)]
    pub last_checked: String,
    #[serde(default)]
    pub status: String, // "online", "offline", "unknown"
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
