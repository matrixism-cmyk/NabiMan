// MEC Management 도메인 타입 정의.
// 백엔드 src/models/mec 구조체와 동기화되어야 합니다.

export interface MecResponse<T> {
  data?: T;
  meta: { timestamp: string; request_id: string; total?: number };
  error?: { code: string; message: string; details?: unknown };
}

export interface MecListResponse<T> {
  data: T[];
  meta: { total: number; timestamp: string; request_id: string };
}

export type TenantStatus =
  | { state: 'provisioning' }
  | { state: 'active' }
  | { state: 'terminating' }
  | { state: 'failed'; reason: string };

export interface NodeAllocation {
  type: 'dedicated' | 'shared';
  node?: string | null;
  gpu_label?: string | null;
  tier?: string | null;
}

export interface ResourceQuota {
  cpu_requests: string;
  cpu_limits: string;
  memory_requests: string;
  memory_limits: string;
  gpu: number;
  storage: string;
  pods: number;
  pvcs: number;
  lb_services: number;
}

export interface Tenant {
  id: string;
  display_name: string;
  task_name?: string | null;
  contact_email?: string | null;
  namespace: string;
  rancher_project_id?: string | null;
  rancher_user_id?: string | null;
  allocation: NodeAllocation;
  quota: ResourceQuota;
  starter_kit?: unknown;
  created_at: string;
  updated_at: string;
  status: TenantStatus;
}

export interface Taint {
  key: string;
  value?: string | null;
  effect: 'NoSchedule' | 'PreferNoSchedule' | 'NoExecute';
}

export interface GpuInfo {
  model: string;
  count: number;
  total_slots: number;
  mode: 'container' | 'vm-passthrough' | 'mig' | 'time-slicing';
  sharing_strategy?: string | null;
  replicas?: number | null;
}

export interface MecNode {
  name: string;
  roles: string[];
  status: 'Ready' | 'NotReady' | 'Unknown';
  capacity: { cpu: string; memory: string; pods: number; gpu?: number | null };
  allocatable: { cpu: string; memory: string; pods: number; gpu?: number | null };
  allocated: {
    cpu_requests: string;
    cpu_limits: string;
    memory_requests: string;
    memory_limits: string;
    gpu_requests: number;
    pod_count: number;
  };
  labels: Record<string, string>;
  taints: Taint[];
  architecture: string;
  os_image: string;
  kernel_version: string;
  cuda_driver?: string | null;
  gpu_info?: GpuInfo | null;
  current_tenant?: string | null;
}

export interface GpuByModel {
  model: string;
  total_slots: number;
  allocated_slots: number;
  available_slots: number;
  nodes: string[];
  sharing_strategy: string;
}

export interface GpuOverview {
  total_slots: number;
  allocated_slots: number;
  available_slots: number;
  by_model: GpuByModel[];
}

export interface PublicIp {
  ip: string;
  assigned_to?: string | null;
  proxy_arp_enabled: boolean;
  status: 'available' | 'assigned' | 'system_reserved';
}

export interface NatRule {
  id: string;
  from_zone: string;
  to_zone: string;
  rule_type: 'dnat' | 'snat' | 'pat';
  label?: string | null;
  source: { any: true } | { groups: string[] };
  destination: string[];
  service_groups: string[];
  translated_to: string;
  enabled: boolean;
  hits: number;
  last_hit?: string | null;
  created_at: string;
}

export interface DashboardSummary {
  tenants: { active: number; total: number };
  nodes: { ready: number; total: number };
  gpu: { total_slots: number; allocated_slots: number; available_slots: number };
  network: { lb_services: number; public_ips_assigned: number };
  firewall: { nat_rules: number; security_policies: number };
}

export interface AuditLog {
  id: string;
  timestamp: string;
  user: string;
  action: string;
  resource_type: string;
  resource_id: string;
  status: 'success' | 'failed' | 'partial';
  duration_ms: number;
  operations: Array<{
    name: string;
    target: string;
    status: string;
    duration_ms: number;
  }>;
}
