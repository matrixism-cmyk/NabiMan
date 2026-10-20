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
  is_logged_in: boolean | null;
  can_manage: boolean;
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
  revision: string;
  can_manage: boolean;
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
  identity: string;
  can_signal: boolean;
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
  /** True when an SSH password is stored (encrypted) for this server. */
  has_password?: boolean;
  host_trust_version?: string;
  host_trust?: { public_key: string; fingerprint: string; verified_by: string; verified_at: string; reason: string } | null;
  /** How a terminal reaches it; records from before this are SSH. */
  terminal?: 'ssh' | 'telnet' | 'none';
  /** How its files are reached; records from before this are SFTP. */
  files?: 'sftp' | 'ftps' | 'ftp' | 'none';
  ftp_port?: number;
  /** A hand-verified FTPS certificate (SHA256:…), for one no authority signs. */
  tls_fingerprint?: string;
}

/** One way of reaching a server, as a check found it. */
export interface ProtocolCheck {
  protocol: string;
  ok: boolean;
  message: string;
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
  /** Each way of reaching it that was tried. */
  checks?: ProtocolCheck[];
}

// --- SSH Key types ---
export interface SshKeyInfo {
  exists: boolean;
  key_path: string;
  public_key?: string;
  fingerprint?: string;
}

// --- Updates types ---
export interface UpdatablePackage {
  name: string;
  current_version: string;
  new_version: string;
}

export interface UpdateCheckResult {
  count: number;
  packages: UpdatablePackage[];
}

// --- Diagnostics types ---
export interface PortCheckResult {
  host: string;
  port: number;
  open: boolean;
}

// --- SSL types ---
export interface SslCertificate {
  domain: string;
  issuer: string;
  expiry: string;
  days_left: number;
  path: string;
  status: string;
}

export interface ApiResponse<T> {
  code?: string;
  success: boolean;
  data: T | null;
  message: string;
}
