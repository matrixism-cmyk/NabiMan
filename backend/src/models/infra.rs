use serde::{Deserialize, Serialize};

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
