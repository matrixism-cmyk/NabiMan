/**
 * The remote workspace's own types: servers, how they were found to be
 * reached, and the SSH key NabiMan signs in with. They belong to Remote, so
 * they live here rather than among NabiMan's shared types.
 */

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
  /** FTPS with TLS from the first byte (usually port 990), not AUTH TLS. */
  ftps_implicit?: boolean;
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

export interface SshKeyInfo {
  exists: boolean;
  key_path: string;
  public_key?: string;
  fingerprint?: string;
}
