export interface ServerStatus {
  hostname: string;
  os: string;
  uptime: number;
  cpu_usage: number;
  memory_total: number;
  memory_used: number;
  disk_total: number;
  disk_used: number;
  load_average: [number, number, number];
}

export interface NetworkInterface {
  name: string;
  ip_address: string;
  mac_address: string;
  rx_bytes: number;
  tx_bytes: number;
  is_up: boolean;
}

export interface NetworkStatus {
  interfaces: NetworkInterface[];
  open_connections: number;
  dns_servers: string[];
}

export interface UserAccount {
  username: string;
  uid: number;
  gid: number;
  home: string;
  shell: string;
  is_logged_in: boolean;
}

export interface ServiceConfig {
  service_name: string;
  config_path: string;
  content: string;
  is_running: boolean;
}

export interface TrafficSnapshot {
  timestamp: string;
  rx_bytes_per_sec: number;
  tx_bytes_per_sec: number;
  active_connections: number;
  interface: string;
}

export interface TrafficSummary {
  total_rx_bytes: number;
  total_tx_bytes: number;
  total_rx_mb: number;
  total_tx_mb: number;
  active_connections: number;
  tcp_connections: {
    established: number;
    listen: number;
    time_wait: number;
    other: number;
  };
}

// --- Container types ---
export interface Container {
  id: string;
  name: string;
  image: string;
  status: string;
  state: string;
  ports: string;
  created: string;
}

export interface ContainerImage {
  id: string;
  repository: string;
  tag: string;
  size: string;
  created: string;
}

// --- Service types ---
export interface SystemService {
  name: string;
  description: string;
  load_state: string;
  active_state: string;
  sub_state: string;
  enabled: boolean;
}

// --- Firewall types ---
export interface FirewallStatus {
  backend: string;
  active: boolean;
  rules: FirewallRule[];
}

export interface FirewallRule {
  number: number;
  action: string;
  protocol: string;
  port: string;
  source: string;
  destination: string;
}

// --- Log types ---
export interface LogEntry {
  timestamp: string;
  unit: string;
  priority: string;
  message: string;
}

// --- Cron types ---
export interface CronJob {
  id: number;
  user: string;
  schedule: string;
  command: string;
}

// --- Config service definition ---
export interface ServiceDefinition {
  id: string;
  display_name: string;
  config_paths: string[];
  is_running?: boolean;
  is_installed?: boolean;
  config_found?: boolean;
  detected_by?: string[];
  version?: string;
}

// --- Process types ---
export interface ProcessInfo {
  pid: number;
  user: string;
  cpu: number;
  memory: number;
  vsz: number;
  rss: number;
  command: string;
  started: string;
}

// --- Disk types ---
export interface DiskPartition {
  filesystem: string;
  mount_point: string;
  fs_type: string;
  total: number;
  used: number;
  available: number;
  use_percent: number;
}

export interface DiskIo {
  device: string;
  reads_per_sec: number;
  writes_per_sec: number;
  read_bytes_per_sec: number;
  write_bytes_per_sec: number;
}

export interface DiskStatus {
  partitions: DiskPartition[];
  io: DiskIo[];
}

// --- Remote server types ---
export interface RemoteServer {
  id: string;
  name: string;
  host: string;
  port: number;
  user: string;
  auth_method: string;
  tags: string[];
  memo: string;
  created_at: string;
  last_checked: string;
  status: string;
}

export interface RemoteServerStatus {
  id: string;
  name: string;
  host: string;
  status: string;
  hostname: string;
  os: string;
  uptime: string;
  cpu_usage: string;
  memory: string;
  disk: string;
  load: string;
  checked_at: string;
}

export interface ApiResponse<T> {
  success: boolean;
  data: T | null;
  message: string;
}
