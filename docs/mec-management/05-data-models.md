# 05. 데이터 모델

## 1. Rust 도메인 모델

### 1.1 테넌트 (Tenant)

```rust
// backend/src/models/mec/tenant.rs

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,                  // "ygram-poc"
    pub display_name: String,        // "와이그램"
    pub task_name: String,           // "페르소나 AI 토이..."
    pub contact_email: Option<String>,

    pub namespace: String,           // K8s namespace
    pub rancher_project_id: String,  // "p-jdtk4"
    pub rancher_user_id: String,     // "u-sdgpf"

    pub allocation: NodeAllocation,
    pub quota: ResourceQuota,
    pub starter_kit: Option<StarterKit>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: TenantStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TenantStatus {
    Provisioning,
    Active,
    Terminating,
    Failed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAllocation {
    pub alloc_type: AllocationType,
    pub node: Option<String>,        // 단독 할당 시
    pub gpu_label: String,           // "gh200", "a40", "t4"
    pub tier: String,                // "high", "backup"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AllocationType {
    Dedicated,   // Taint + NoSchedule
    Shared,      // nodeSelector만
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    pub cpu_requests: String,        // "64"
    pub cpu_limits: String,          // "70"
    pub memory_requests: String,     // "400Gi"
    pub memory_limits: String,       // "500Gi"
    pub gpu: u32,                    // 7
    pub storage: String,             // "1Ti"
    pub pods: u32,                   // 150
    pub pvcs: u32,                   // 30
    pub lb_services: u32,            // 5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarterKit {
    pub ubuntu_ssh: StarterService,
    pub vscode: StarterService,
    pub ssh_user: String,
    pub ssh_password_hash: String,   // 저장 시 bcrypt
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarterService {
    pub deployed: bool,
    pub external_ip: Option<String>,
    pub port: u16,
    pub deployed_at: Option<DateTime<Utc>>,
}
```

### 1.2 노드 (Node)

```rust
// backend/src/models/mec/node.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub name: String,                // "mec-wn04"
    pub roles: Vec<String>,          // ["worker"], ["control-plane"]
    pub status: NodeStatus,

    pub capacity: NodeCapacity,
    pub allocatable: NodeCapacity,
    pub allocated: NodeAllocated,

    pub labels: std::collections::HashMap<String, String>,
    pub taints: Vec<Taint>,

    pub architecture: String,        // "amd64", "arm64"
    pub os_image: String,            // "Ubuntu 22.04.5 LTS"
    pub kernel_version: String,
    pub cuda_driver: Option<String>,

    pub gpu_info: Option<GpuInfo>,
    pub current_tenant: Option<String>,  // Taint의 value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    Ready,
    NotReady,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapacity {
    pub cpu: String,
    pub memory: String,
    pub pods: u32,
    pub gpu: Option<u32>,            // nvidia.com/gpu
    pub storage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAllocated {
    pub cpu_requests: String,
    pub cpu_limits: String,
    pub memory_requests: String,
    pub memory_limits: String,
    pub gpu_requests: u32,
    pub pod_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Taint {
    pub key: String,
    pub value: Option<String>,
    pub effect: TaintEffect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaintEffect {
    NoSchedule,
    PreferNoSchedule,
    NoExecute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub model: String,               // "T4", "A40", "GH200"
    pub count: u32,                  // 물리 카드 수
    pub total_slots: u32,            // Time-Slicing 적용 후 슬롯 수
    pub mode: GpuMode,
    pub sharing_strategy: Option<String>, // "time-slicing", "mig", "none"
    pub replicas: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuMode {
    Container,                       // 일반 컨테이너 할당
    VmPassthrough,                   // VFIO (KubeVirt VM 전용)
    Mig,                             // GH200 MIG
}
```

### 1.3 네트워크 (Network)

```rust
// backend/src/models/mec/network.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LbIpPool {
    pub name: String,                // "mec-pool", "ingress230-pool"
    pub address_ranges: Vec<String>, // ["172.20.26.151-172.20.26.200", ...]
    pub total_ips: u32,
    pub used_ips: u32,
    pub auto_assign: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerService {
    pub namespace: String,
    pub name: String,
    pub external_ip: String,
    pub ports: Vec<ServicePort>,
    pub selector_summary: String,
    pub age_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePort {
    pub name: Option<String>,
    pub port: u16,
    pub target_port: u16,
    pub node_port: Option<u16>,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressRule {
    pub namespace: String,
    pub name: String,
    pub class: Option<String>,       // "nginx"
    pub host: Option<String>,
    pub paths: Vec<IngressPath>,
    pub tls: Vec<IngressTls>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressPath {
    pub path: String,
    pub path_type: String,           // "Prefix", "Exact"
    pub backend_service: String,
    pub backend_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressTls {
    pub hosts: Vec<String>,
    pub secret_name: Option<String>,
}
```

### 1.4 방화벽 (Firewall)

```rust
// backend/src/models/mec/firewall.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicIp {
    pub ip: String,                  // "121.147.13.246"
    pub assigned_to: Option<String>, // "nabi-ssh-246" (NAT label)
    pub proxy_arp_enabled: bool,
    pub status: PublicIpStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PublicIpStatus {
    Available,
    Assigned,
    SystemReserved,   // 228 (방화벽 자체)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatRule {
    pub id: String,                  // "nat-15" (policy index)
    pub from_zone: String,           // "untrust"
    pub to_zone: String,             // "trust"
    pub rule_type: NatType,
    pub label: Option<String>,
    pub source: NatSource,
    pub destination: Vec<String>,    // ["121.147.13.246/32"]
    pub service_groups: Vec<String>, // ["TCP_22", "TCP_8080"]
    pub translated_to: String,       // "172.20.26.199"
    pub enabled: bool,
    pub hits: u64,
    pub last_hit: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NatType {
    Dnat,   // 외부→내부
    Snat,   // 내부→외부 (static)
    Pat,    // 내부→외부 (dynamic)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NatSource {
    Any,
    Groups(Vec<String>),             // ["뉴젠스본사_222.233.105.0/24"]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub id: String,
    pub from_zone: String,
    pub to_zone: String,
    pub action: String,              // "pass", "drop", "reject"
    pub destination_group: Option<String>,
    pub service_groups: Vec<String>,
    pub label: Option<String>,
    pub enabled: bool,
}
```

### 1.5 GPU

```rust
// backend/src/models/mec/gpu.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuOverview {
    pub total_slots: u32,
    pub allocated_slots: u32,
    pub available_slots: u32,
    pub by_model: Vec<GpuByModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuByModel {
    pub model: String,               // "T4", "A40", "GH200"
    pub total_slots: u32,
    pub allocated_slots: u32,
    pub available_slots: u32,
    pub nodes: Vec<String>,
    pub sharing_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuAllocation {
    pub node: String,
    pub tenant: Option<String>,
    pub allocated: u32,
    pub total: u32,
    pub model: String,
    pub pods: Vec<GpuPodUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuPodUsage {
    pub namespace: String,
    pub pod: String,
    pub gpu_requests: u32,
    pub gpu_utilization_percent: Option<f32>,  // DCGM에서
    pub gpu_memory_used_mb: Option<u32>,
}
```

### 1.6 Job (비동기 작업)

```rust
// backend/src/models/mec/job.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,                  // "job-7c4e2f81"
    pub kind: JobKind,
    pub status: JobStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_by: String,          // user_id
    pub steps: Vec<JobStep>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobKind {
    TenantCreate { spec: serde_json::Value },
    TenantDelete { tenant_id: String },
    StarterKitDeploy { tenant_id: String },
    StarterKitDelete { tenant_id: String },
    GpuModeSwitch { node: String, from_mode: String, to_mode: String },
    DocGenerate { template: String, tenant_ids: Vec<String> },
    FirewallNatAdd { spec: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed { error: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStep {
    pub name: String,                // "create_namespace"
    pub status: JobStepStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub progress: Option<u32>,       // 0-100
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStepStatus {
    Pending,
    Running,
    Completed,
    Skipped,
    Failed,
}
```

### 1.7 Audit Log

```rust
// backend/src/models/mec/audit.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: String,                  // UUID
    pub timestamp: DateTime<Utc>,
    pub user: String,                // admin, etc.
    pub action: AuditAction,
    pub resource_type: String,       // "tenant", "node", "firewall_rule"
    pub resource_id: String,
    pub status: AuditStatus,
    pub duration_ms: u64,
    pub input: Option<serde_json::Value>,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub operations: Vec<OperationLog>,
    pub source_ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    Create, Read, Update, Delete,
    Custom(String),                  // "gpu_mode_switch", "firewall_sync"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditStatus {
    Success,
    Failed,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationLog {
    pub name: String,                // "create_namespace"
    pub target: String,              // "kubernetes"
    pub status: String,              // "success", "failed"
    pub duration_ms: u64,
    pub details: Option<serde_json::Value>,
}
```

## 2. SQLite 스키마

```sql
-- backend/data/mec.db

-- 테넌트 메타데이터 (K8s 외부에 저장해야 할 정보)
CREATE TABLE tenant_profiles (
    id                  TEXT PRIMARY KEY,
    display_name        TEXT NOT NULL,
    task_name           TEXT,
    contact_email       TEXT,
    contact_manager     TEXT,
    contact_phone       TEXT,

    starter_ssh_password_hash  TEXT,
    harbor_project_created     INTEGER DEFAULT 0,

    created_at          TEXT NOT NULL,
    created_by          TEXT,
    updated_at          TEXT
);

-- Job 큐
CREATE TABLE jobs (
    id                  TEXT PRIMARY KEY,
    kind                TEXT NOT NULL,
    status              TEXT NOT NULL,
    spec                TEXT,                     -- JSON
    steps               TEXT,                     -- JSON array
    result              TEXT,                     -- JSON
    error               TEXT,
    created_by          TEXT,
    started_at          TEXT NOT NULL,
    completed_at        TEXT,

    INDEX idx_jobs_status (status),
    INDEX idx_jobs_created_by (created_by),
    INDEX idx_jobs_started_at (started_at)
);

-- Audit Log
CREATE TABLE audit_logs (
    id                  TEXT PRIMARY KEY,
    timestamp           TEXT NOT NULL,
    user                TEXT NOT NULL,
    action              TEXT NOT NULL,
    resource_type       TEXT NOT NULL,
    resource_id         TEXT NOT NULL,
    status              TEXT NOT NULL,
    duration_ms         INTEGER,
    input               TEXT,                     -- JSON
    output              TEXT,                     -- JSON
    error               TEXT,
    operations          TEXT,                     -- JSON array
    source_ip           TEXT,
    user_agent          TEXT,

    INDEX idx_audit_timestamp (timestamp),
    INDEX idx_audit_user (user),
    INDEX idx_audit_action (action),
    INDEX idx_audit_resource (resource_type, resource_id)
);

-- 생성된 문서 (다운로드 이력)
CREATE TABLE generated_docs (
    id                  TEXT PRIMARY KEY,
    template            TEXT NOT NULL,
    format              TEXT NOT NULL,            -- "docx", "pdf", "xlsx"
    filename            TEXT NOT NULL,
    file_path           TEXT NOT NULL,
    file_size_bytes     INTEGER,
    related_tenants     TEXT,                     -- JSON array
    generated_by        TEXT,
    generated_at        TEXT NOT NULL,
    expires_at          TEXT,                     -- 자동 삭제용
    download_count      INTEGER DEFAULT 0,

    INDEX idx_docs_template (template),
    INDEX idx_docs_generated_at (generated_at)
);

-- AXGATE 조회 캐시
CREATE TABLE firewall_cache (
    cache_key           TEXT PRIMARY KEY,         -- "nat-rules", "public-ips"
    data                TEXT NOT NULL,            -- JSON
    cached_at           TEXT NOT NULL,
    expires_at          TEXT NOT NULL
);

-- 설정 (암호화된 크리덴셜 등)
CREATE TABLE mec_config (
    key                 TEXT PRIMARY KEY,
    value               TEXT NOT NULL,            -- 암호화된 값
    value_type          TEXT,                     -- "string", "json", "secret"
    description         TEXT,
    updated_at          TEXT NOT NULL,
    updated_by          TEXT
);
```

## 3. 외부 시스템 매핑

### 3.1 Kubernetes 리소스 ↔ 내부 모델

| 내부 모델 | K8s 리소스 | Namespace |
|---------|-----------|-----------|
| `Tenant` | Namespace + ResourceQuota + LimitRange + NetworkPolicy + Role + RoleBinding + PVC | (자기 NS) |
| `Node` | Node | cluster-scoped |
| `LoadBalancerService` | Service (type=LoadBalancer) | 각 NS |
| `IngressRule` | Ingress | 각 NS |
| `GpuAllocation` | Pod(nvidia.com/gpu 요청) + Node(nvidia.com/gpu capacity) | - |
| `StarterKit` | Deployment + Service (ubuntu-ssh, vscode) | 각 NS |

### 3.2 Rancher 리소스 ↔ 내부 모델

| 내부 모델 | Rancher CRD |
|---------|-------------|
| `Tenant.rancher_project_id` | `Project` (management.cattle.io/v3) |
| `Tenant.rancher_user_id` | `User` (management.cattle.io/v3) |
| 멤버 권한 | `ProjectRoleTemplateBinding` |

### 3.3 AXGATE 리소스 ↔ 내부 모델

| 내부 모델 | AXGATE config |
|---------|---------------|
| `NatRule` | `ip nat policy from X to Y {num}` |
| `SecurityPolicy` | `ip security policy from X to Y {num}` |
| `PublicIp.proxy_arp_enabled` | `interface eth0-0 / ip proxy-arp alias` |

## 4. 캐시 전략

| 데이터 | TTL | 갱신 트리거 |
|-------|-----|-----------|
| K8s Node 목록 | 30초 | Watch API로 즉시 무효화 |
| Tenant 목록 | 30초 | CRUD 작업 시 즉시 무효화 |
| GPU Overview | 15초 | DCGM 메트릭 반영 |
| LB 서비스 | 30초 | Service Watch |
| AXGATE NAT 규칙 | **5분** | 수동 "동기화" 버튼 + CRUD 시 무효화 |
| Harbor 이미지 목록 | 2분 | - |

## 5. 이벤트 모델 (Phase 3+)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MecEvent {
    TenantCreated { tenant: Tenant },
    TenantDeleted { id: String },
    TenantQuotaUpdated { id: String, before: ResourceQuota, after: ResourceQuota },
    NodeTaintChanged { node: String, added: Vec<Taint>, removed: Vec<Taint> },
    GpuModeChanged { node: String, from: GpuMode, to: GpuMode },
    NatRuleAdded { rule: NatRule },
    NatRuleDeleted { id: String },
    QuotaExceeded { tenant: String, resource: String, current: String, limit: String },
}
```

이벤트는 내부 channel을 통해 **Webhook**, **Slack 알림**, **Audit Log** 등으로 fan-out.
