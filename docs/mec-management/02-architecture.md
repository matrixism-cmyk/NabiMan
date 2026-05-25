# 02. 아키텍처 (Architecture)

## 1. 전체 구성도

```
┌─────────────────────────────────────────────────────────────────────┐
│                         사용자 (관리자)                                │
│                    Web Browser (Chrome/Edge)                         │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ HTTPS
┌──────────────────────────────▼──────────────────────────────────────┐
│                     NabiMan Frontend (React)                         │
│                                                                       │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────────────────────────┐ │
│  │ Server   │ │ Container│ │ 🆕 MEC Management (신규 모듈)        │ │
│  │ Mgmt     │ │ Mgmt     │ │                                      │ │
│  │ (기존)   │ │ (기존)   │ │ - Tenants  - Nodes  - GPU            │ │
│  │          │ │          │ │ - Network  - Firewall  - Storage    │ │
│  └──────────┘ └──────────┘ └──────────────────────────────────────┘ │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ REST API (JSON)
┌──────────────────────────────▼──────────────────────────────────────┐
│              NabiMan Backend (Rust / Actix-web)                      │
│                                                                       │
│  ┌─────────────┐  ┌─────────────┐  ┌──────────────────────────────┐ │
│  │ Auth Layer  │  │ 기존 API    │  │ 🆕 MEC API                   │ │
│  │ (JWT)       │  │ Handlers    │  │                              │ │
│  └─────────────┘  └─────────────┘  │  ┌─────────────────────────┐ │ │
│                                     │  │ tenants module          │ │ │
│  ┌─────────────────────────────┐   │  │ nodes module            │ │ │
│  │ Services Layer              │   │  │ gpu module              │ │ │
│  │                             │   │  │ network module          │ │ │
│  │  - KubeService (kube-rs)    │◄──┤  │ firewall module         │ │ │
│  │  - RancherService (reqwest) │◄──┤  │ storage module          │ │ │
│  │  - AxgateService (ssh2)     │◄──┤  │ docgen module           │ │ │
│  │  - HarborService (reqwest)  │◄──┤  │ audit module            │ │ │
│  │  - AuditLogger (SQLite)     │   │  └─────────────────────────┘ │ │
│  └─────────────────────────────┘   └──────────────────────────────┘ │
└──────────┬──────────────┬──────────────┬──────────────┬─────────────┘
           │              │              │              │
           ▼              ▼              ▼              ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ Kubernetes   │ │ Rancher      │ │ AXGATE       │ │ Harbor       │
│ API Server   │ │ API          │ │ 방화벽       │ │ Registry     │
│              │ │              │ │              │ │              │
│ 172.20.26.   │ │ rancher.jcia.│ │ 121.147.13.  │ │ 172.20.26.   │
│ 239:6443     │ │ mec.local    │ │ 228:2222     │ │ 234          │
└──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘
```

## 2. 컴포넌트 상세

### 2.1 Frontend (React)

**신규 페이지 구조** (기존 NabiMan 라우팅에 추가):

```
/mec/
  dashboard         # MEC 종합 대시보드
  tenants/
    list            # 테넌트 목록
    new             # 신규 테넌트 생성 마법사
    :id             # 테넌트 상세/편집
  nodes/
    list            # 노드 목록 + 자원 현황
    :name           # 노드 상세 (Taint/Label/GPU)
  gpu/
    overview        # GPU 자원 현황
    allocation      # Slot 할당 매핑
  network/
    lb-pool         # MetalLB IP Pool
    services        # LB 서비스 목록
    ingress         # Ingress 규칙
  firewall/
    nat-rules       # AXGATE NAT 규칙
    public-ips      # 공인 IP 매핑
  storage/
    pvcs            # PVC 현황
    harbor          # Harbor 이미지
  audit             # Audit Log
```

**상태 관리**: TanStack Query(React Query) 사용 권장 (K8s 상태 주기적 폴링에 적합)

### 2.2 Backend (Rust / Actix-web)

**모듈 구조** (기존 `backend/src`에 추가):

```
backend/src/
├── main.rs                      # 기존
├── auth/                        # 기존 인증
├── api/
│   ├── server/                  # 기존 (CPU/메모리/디스크)
│   ├── container/               # 기존 (Docker)
│   ├── service/                 # 기존 (systemd)
│   └── mec/                     # 🆕 MEC 관리 모듈
│       ├── mod.rs
│       ├── tenants.rs           # 테넌트 CRUD
│       ├── nodes.rs             # 노드 관리
│       ├── gpu.rs               # GPU 자원
│       ├── network.rs           # MetalLB/Ingress
│       ├── firewall.rs          # AXGATE
│       ├── storage.rs           # Longhorn/Harbor
│       ├── docgen.rs            # 문서 생성
│       └── audit.rs             # Audit Log
├── services/                    # 외부 시스템 클라이언트
│   ├── kube_service.rs          # 🆕 kube-rs wrapper
│   ├── rancher_service.rs       # 🆕 Rancher API client
│   ├── axgate_service.rs        # 🆕 SSH CLI wrapper
│   └── harbor_service.rs        # 🆕 Harbor API client
├── models/
│   └── mec/                     # 🆕 MEC 도메인 모델
└── db/
    └── audit_log.rs             # 🆕 SQLite 기반 감사로그
```

**Cargo.toml 추가 의존성**:

```toml
[dependencies]
# 기존 의존성 유지
actix-web = "4"
tokio = { version = "1", features = ["full"] }

# 🆕 MEC Management 용
kube = { version = "0.95", features = ["runtime", "derive"] }
k8s-openapi = { version = "0.23", features = ["latest"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
russh = "0.44"                   # AXGATE SSH CLI (or ssh2)
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = "0.4"
uuid = { version = "1", features = ["v4"] }
docx-rs = "0.4"                  # 접속 가이드 docx 생성
```

### 2.3 외부 시스템 연동

| 시스템 | 프로토콜 | 인증 | 주요 용도 |
|--------|---------|------|---------|
| **Kubernetes API** | HTTPS (gRPC 간접) | ServiceAccount Token (In-cluster) or Kubeconfig | NS/Pod/Service/Quota CRUD |
| **Rancher API** | HTTPS REST | Bearer Token | Project/User/RoleBinding 관리 |
| **AXGATE 방화벽** | SSH (port 2222) | Username/Password (저장 시 암호화) | NAT 규칙, proxy-arp |
| **Harbor Registry** | HTTPS REST | Basic Auth (admin/Jcia12345!@#) | 이미지 목록, 프로젝트 관리 |

### 2.4 데이터 저장

**로컬 상태 저장 (SQLite)**:

```
backend/data/mec.db
├── audit_log            # 모든 변경 작업 이력
├── tenant_profiles      # 테넌트 메타데이터 (접속 가이드 생성용)
├── node_allocations     # 노드-기업 매핑 캐시
└── firewall_rules_cache # AXGATE 조회 결과 캐시
```

**Source of Truth**는 각 외부 시스템(K8s, Rancher, AXGATE). NabiMan DB는 **조회 캐시** 및 **감사로그**.

## 3. 인증/권한 모델

### 3.1 사용자 인증

- **NabiMan 기존 JWT 인증 재사용**
- MEC 모듈 접근은 **admin 역할** 필요 (향후 세분화 가능)

### 3.2 외부 시스템 자격 증명

```
NabiMan Backend (In-Cluster Pod로 배포 시):
  ├─ K8s API: In-cluster ServiceAccount (nabiman-mec-admin)
  ├─ Rancher API: Config로 관리자 토큰 주입 (env var)
  ├─ AXGATE: Config에 암호화된 크리덴셜 저장 (age/sops)
  └─ Harbor: Config에 암호화된 크리덴셜 저장
```

**ServiceAccount 권한** (K8s):

```yaml
# nabiman-mec-admin ClusterRole
rules:
- apiGroups: ["", "apps", "networking.k8s.io", "rbac.authorization.k8s.io"]
  resources: ["*"]
  verbs: ["*"]
- apiGroups: ["management.cattle.io"]  # Rancher CRD
  resources: ["projects", "users", "projectroletemplatebindings"]
  verbs: ["*"]
- apiGroups: ["metallb.io"]
  resources: ["ipaddresspools"]
  verbs: ["get", "list", "watch"]
```

## 4. 배포 형태

### 4.1 단일 바이너리 배포 (권장, 기존 NabiMan 방식 유지)

```
┌──────────────────────────┐
│  nabiman (단일 바이너리)  │
│                          │
│  + Frontend (embedded)   │
│  + Backend handlers      │
│  + MEC module            │
│  + SQLite (embedded)     │
└──────────────────────────┘
```

**장점**: 기존 NabiMan과 동일한 배포 방식 유지, CI/CD 변경 불필요
**단점**: MEC 기능 문제가 전체 NabiMan에 영향

### 4.2 대안: Kubernetes Pod 배포

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nabiman
  namespace: nabiman-system
spec:
  template:
    spec:
      serviceAccountName: nabiman-mec-admin
      containers:
      - name: nabiman
        image: harbor.jcia.mec.local/system/nabiman:latest
        env:
        - name: RANCHER_API_URL
          value: "https://rancher.jcia.mec.local"
        - name: RANCHER_API_TOKEN
          valueFrom:
            secretKeyRef:
              name: rancher-creds
              key: token
        - name: AXGATE_HOST
          value: "121.147.13.228:2222"
        - name: AXGATE_CREDS
          valueFrom:
            secretKeyRef:
              name: axgate-creds
              key: creds
```

**이점**: K8s API를 In-cluster ServiceAccount로 안전하게 접근, 재시작/롤링업데이트 자동화

## 5. 데이터 흐름 예시

### 5.1 테넌트 생성 플로우

```
[User] 브라우저에서 "신규 테넌트" 폼 작성
   │    기업명, 노드 지정(mec-wn04), GPU 종류(GH200), Quota
   ▼
[Frontend] POST /api/mec/tenants
   │
   ▼
[Backend] tenants::create_handler
   │
   ├─ 1. Validate input (노드 중복 할당 체크 등)
   │
   ├─ 2. KubeService.create_namespace(name, labels)
   │     └─► Kubernetes API
   │
   ├─ 3. KubeService.apply_resources(ResourceQuota, LimitRange, NetworkPolicy, RBAC, PVC)
   │
   ├─ 4. KubeService.label_node(node, taint)
   │
   ├─ 5. RancherService.create_project(name)
   ├─ 6. RancherService.create_user(user_id, pw_hash)
   ├─ 7. RancherService.create_prtb(project, user)
   │
   ├─ 8. (선택) Deploy starter kit (Ubuntu SSH + VS Code)
   │
   ├─ 9. AuditLogger.log(action="tenant_create", user, tenant, ...)
   │
   └─ 10. Return { tenant_id, user, password, node, ... }
   │
   ▼
[Frontend] 성공 화면 표시 + "가이드 다운로드" 버튼
```

### 5.2 실시간 대시보드 (노드/GPU 현황)

```
[Frontend] 대시보드 페이지 진입
   │
   ▼
[TanStack Query] 병렬 쿼리 (useQuery 5개):
   │  - GET /api/mec/nodes
   │  - GET /api/mec/gpu/overview
   │  - GET /api/mec/tenants/stats
   │  - GET /api/mec/network/lb-usage
   │  - GET /api/mec/firewall/public-ips
   │
   ▼
[Backend] 각 핸들러가 KubeService 호출
   │
   ├─ nodes: kube-rs로 Node list
   ├─ gpu: Node annotation + Pod resources 집계
   ├─ tenants: NS count + Quota used
   ├─ network: Service(LB) list + MetalLB IPAddressPool
   └─ firewall: AxgateService.list_nat_rules() [캐시 30초]
   │
   ▼
[Frontend] 카드 위젯 5개로 렌더링, 30초마다 자동 refetch
```

## 6. 비동기 작업 처리

긴 시간이 걸리는 작업(예: 테넌트 전체 삭제, 스타터킷 배포)은 **Job Queue** 방식으로 처리:

```rust
// 백그라운드 Job 큐 (Tokio task)
enum MecJob {
    CreateTenant { spec: TenantSpec },
    DeleteTenant { id: String },
    DeployStarterKit { tenant_id: String },
    SwitchGpuMode { node: String, mode: GpuMode },
}

// Frontend는 Job ID를 받고 polling 또는 SSE로 진행 상황 확인
// GET /api/mec/jobs/:id -> { status: "running|completed|failed", progress: 60, logs: [...] }
```

## 7. 확장성 고려

- **다중 클러스터 지원**: 향후 여수/광주 확장 대비, KubeService를 cluster_id 파라미터로 추상화
- **플러그인 아키텍처**: 기업별 커스텀 배포 템플릿을 YAML로 로드
- **API 버저닝**: `/api/mec/v1/...` 형태로 시작
