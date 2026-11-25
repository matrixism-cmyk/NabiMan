/** What GET /api/rack/layout and /api/rack/status answer (backend/src/rack). */
export type DeviceKind = 'server' | 'switch' | 'firewall' | '5g' | 'storage' | 'external' | 'other';
/** `noreply`: a device that is no node was asked and did not answer (not `down`, which is a node NotReady). */
export type Level = 'ok' | 'warn' | 'down' | 'noreply' | 'unknown' | 'none';

export interface RackDevice {
  id: string;
  label: string;
  type: DeviceKind;
  /** The device's TOP slot (U1 is the bottom); null: not placed yet. */
  u: number | null;
  h: number;
  u_confirmed: boolean;
  node: string | null;
  /** The nodes this machine may be, while nobody has checked which. */
  node_candidates: string[];
  ip: string | null;
  bmc: string | null;
  gpu: string | null;
  cpu: number | null;
  arch: string | null;
  desc: string | null;
  order_hint: number | null;
  /** How a device that is no node is asked whether it answers: `ping`, `tcp:<port>`, `none`. */
  check: string;
  /** Not one of ours. */
  external: boolean;
}

export interface Rack {
  id: string;
  name: string;
  role: string | null;
  units: number;
  external: boolean;
  devices: RackDevice[];
}

export interface RackNode {
  name: string;
  ip: string | null;
  role: string | null;
  model: string | null;
  cpu: number | null;
  mem_gi: number | null;
  gpu: string | null;
  arch: string | null;
}

export interface RackLayout {
  version: string;
  title: string;
  sources: string[];
  notes: string | null;
  power: { label: string; value: string }[];
  racks: Rack[];
  nodes: RackNode[];
  overlaps: { rack: string; first: string; second: string }[];
}

export interface Fault { code: 'no_file' | 'unreadable' | 'invalid' | string; detail: string }

export interface LayoutAnswer { configured: boolean; layout: RackLayout | null; error: Fault | null }

export interface GpuStatus {
  index: number;
  mig_id: number | null;
  mig_profile: string | null;
  model: string | null;
  util_pct: number | null;
  temp_c: number | null;
  power_w: number | null;
  mem_used_mib: number | null;
  mem_total_mib: number | null;
}

export interface NodeStatus {
  level: Level;
  reasons: string[];
  ready: boolean | null;
  cpu_pct: number | null;
  mem_pct: number | null;
  disk_pct: number | null;
  temp_c: number | null;
  gpus: GpuStatus[];
}

export interface DeviceStatus {
  level: Level;
  nodes: string[];
  external: boolean;
  /** Without a node, why there is no answer to show (`no_address`, `check_none`, `ping_missing`, …). */
  reason: string | null;
  latency_ms: number | null;
  /** `prometheus-icmp`, `prometheus-tcp`, `ping`, `tcp:<port>`. */
  checked_by: string | null;
  checked_at: string | null;
}

export interface StatusSummary {
  nodes_total: number;
  nodes_ready: number;
  gpus: number;
  warnings: number;
  down: number;
  max_temp_c: number | null;
  /** Devices that are no node and were asked whether they answer; of those, how many did, and how many of ours / of others did not. */
  probed: number;
  reachable: number;
  noreply: number;
  external_noreply: number;
  over_limit: number;
}

export interface RackStatus {
  collected_at: string;
  source: 'ok' | 'partial' | 'unreachable' | 'not_configured';
  message: string | null;
  layout_version: string;
  nodes: Record<string, NodeStatus>;
  devices: Record<string, DeviceStatus>;
  summary: StatusSummary;
  thresholds: { temp_c: number; memory_pct: number; disk_pct: number };
}

export interface StatusAnswer { configured: boolean; status: RackStatus | null; error: Fault | null }
