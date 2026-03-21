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

export interface ApiResponse<T> {
  success: boolean;
  data: T | null;
  message: string;
}
