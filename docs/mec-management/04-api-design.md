# 04. API 설계

## 1. 설계 원칙

- **RESTful**: 리소스 중심, HTTP 메서드 의미 준수
- **일관된 응답 포맷**: 모든 응답은 `{ data, meta, error }` 구조
- **인증**: NabiMan 기존 JWT Bearer Token
- **버저닝**: URL prefix `/api/mec/v1/...`
- **Idempotency**: POST/PUT/DELETE 재요청 안전하도록 Idempotency-Key 헤더 지원
- **비동기 작업**: 30초 이상 걸리는 작업은 Job ID 반환 + SSE/polling

## 2. 공통 응답 포맷

### 성공
```json
{
  "data": { ... },
  "meta": {
    "timestamp": "2026-04-20T15:32:10Z",
    "request_id": "req-a8f3d1e2"
  }
}
```

### 리스트
```json
{
  "data": [ ... ],
  "meta": {
    "total": 5,
    "page": 1,
    "per_page": 50,
    "timestamp": "..."
  }
}
```

### 에러
```json
{
  "error": {
    "code": "TENANT_NOT_FOUND",
    "message": "테넌트 'xyz'을(를) 찾을 수 없습니다",
    "details": { "tenant_id": "xyz" }
  },
  "meta": { "timestamp": "...", "request_id": "..." }
}
```

### HTTP 상태 코드

| 코드 | 의미 |
|------|------|
| 200 | OK (조회/업데이트 성공) |
| 201 | Created (생성 성공) |
| 202 | Accepted (비동기 Job 시작) |
| 400 | Bad Request (검증 실패) |
| 401 | Unauthorized (인증 실패) |
| 403 | Forbidden (권한 부족) |
| 404 | Not Found |
| 409 | Conflict (중복 생성 등) |
| 422 | Unprocessable Entity (의미적 오류) |
| 500 | Internal Server Error |
| 502 | Bad Gateway (외부 시스템 연동 실패) |

## 3. 엔드포인트 목록

### 3.1 대시보드

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/dashboard/summary` | 전체 자원 요약 (노드/GPU/테넌트/LB/스토리지) |
| GET | `/api/mec/v1/dashboard/activity` | 최근 Audit Log 10건 |

### 3.2 테넌트 관리

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/tenants` | 테넌트 목록 (필터/검색) |
| POST | `/api/mec/v1/tenants` | 신규 테넌트 생성 (Job 반환) |
| GET | `/api/mec/v1/tenants/{id}` | 테넌트 상세 |
| PATCH | `/api/mec/v1/tenants/{id}` | 테넌트 편집 (Quota 변경 등) |
| DELETE | `/api/mec/v1/tenants/{id}` | 테넌트 삭제 (Job 반환) |
| GET | `/api/mec/v1/tenants/{id}/resources` | Pods/Services/PVCs 목록 |
| GET | `/api/mec/v1/tenants/{id}/quota-usage` | 쿼터 사용량 |
| POST | `/api/mec/v1/tenants/{id}/starter-kit` | 스타터킷 재배포 |
| DELETE | `/api/mec/v1/tenants/{id}/starter-kit` | 스타터킷 삭제 |
| GET | `/api/mec/v1/tenants/{id}/members` | 멤버 목록 |
| POST | `/api/mec/v1/tenants/{id}/members` | 멤버 추가 |
| DELETE | `/api/mec/v1/tenants/{id}/members/{user_id}` | 멤버 제거 |
| GET | `/api/mec/v1/tenants/{id}/guide.docx` | 접속 가이드 다운로드 |
| GET | `/api/mec/v1/tenants/{id}/guide.pdf` | 접속 가이드 PDF |

### 3.3 노드 관리

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/nodes` | 노드 목록 |
| GET | `/api/mec/v1/nodes/{name}` | 노드 상세 |
| GET | `/api/mec/v1/nodes/{name}/pods` | 노드에서 실행 중인 Pod |
| GET | `/api/mec/v1/nodes/{name}/metrics` | 실시간 메트릭 (CPU/Mem/GPU) |
| PATCH | `/api/mec/v1/nodes/{name}/labels` | 라벨 추가/제거 |
| PATCH | `/api/mec/v1/nodes/{name}/taints` | Taint 추가/제거 |
| POST | `/api/mec/v1/nodes/{name}/cordon` | Cordon (스케줄링 차단) |
| POST | `/api/mec/v1/nodes/{name}/uncordon` | Uncordon |
| POST | `/api/mec/v1/nodes/{name}/gpu-mode` | GPU 모드 전환 (VFIO↔Time-Slicing) |

### 3.4 GPU 자원

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/gpu/overview` | GPU 종합 현황 |
| GET | `/api/mec/v1/gpu/allocations` | Slot 할당 매핑 (노드→테넌트) |
| GET | `/api/mec/v1/gpu/usage` | 실시간 사용률 (DCGM) |

### 3.5 네트워크

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/network/lb-pools` | MetalLB IP Pool 현황 |
| POST | `/api/mec/v1/network/lb-pools/{name}/extend` | IP Pool 확장 |
| GET | `/api/mec/v1/network/services` | LB 서비스 목록 |
| GET | `/api/mec/v1/network/ingresses` | Ingress 목록 |
| POST | `/api/mec/v1/network/ingresses` | Ingress 규칙 추가 |

### 3.6 방화벽 (AXGATE)

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/firewall/public-ips` | 공인 IP 할당 현황 |
| GET | `/api/mec/v1/firewall/nat-rules` | NAT 규칙 목록 |
| POST | `/api/mec/v1/firewall/nat-rules` | NAT 규칙 추가 |
| DELETE | `/api/mec/v1/firewall/nat-rules/{id}` | NAT 규칙 삭제 |
| PATCH | `/api/mec/v1/firewall/nat-rules/{id}` | NAT 규칙 편집 |
| GET | `/api/mec/v1/firewall/security-policies` | 보안정책 목록 |
| POST | `/api/mec/v1/firewall/sync` | AXGATE running-config 동기화 |

### 3.7 스토리지

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/storage/overview` | Longhorn 용량 현황 |
| GET | `/api/mec/v1/storage/pvcs` | 전체 PVC 목록 |
| GET | `/api/mec/v1/storage/harbor/projects` | Harbor 프로젝트 |
| POST | `/api/mec/v1/storage/harbor/projects` | Harbor 프로젝트 생성 |
| GET | `/api/mec/v1/storage/harbor/projects/{name}/images` | 이미지 목록 |

### 3.8 사용자 관리

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/users` | Rancher 사용자 목록 (테넌트 dev 계정) |
| POST | `/api/mec/v1/users` | 사용자 생성 |
| DELETE | `/api/mec/v1/users/{id}` | 사용자 삭제 |
| POST | `/api/mec/v1/users/{id}/reset-password` | 비밀번호 리셋 |

### 3.9 문서 생성

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/docs/templates` | 사용 가능한 문서 템플릿 |
| POST | `/api/mec/v1/docs/generate` | 문서 생성 요청 (Job 반환) |
| GET | `/api/mec/v1/docs/downloads/{id}` | 생성된 문서 다운로드 |

### 3.10 Audit Log

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/audit/logs` | Audit Log 조회 (필터/검색) |
| GET | `/api/mec/v1/audit/logs/{id}` | Audit Log 상세 |
| GET | `/api/mec/v1/audit/export` | 기간별 export (CSV/JSON) |

### 3.11 Job (비동기 작업)

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/mec/v1/jobs` | 전체 Job 목록 |
| GET | `/api/mec/v1/jobs/{id}` | Job 상태 (polling용) |
| GET | `/api/mec/v1/jobs/{id}/stream` | SSE로 실시간 진행 상황 |
| POST | `/api/mec/v1/jobs/{id}/cancel` | Job 취소 (가능한 경우) |

## 4. 주요 엔드포인트 상세

### 4.1 `POST /api/mec/v1/tenants` (테넌트 생성)

**Request**:
```json
{
  "tenant_id": "ygram-poc",
  "display_name": "와이그램",
  "task_name": "페르소나 AI 토이 'NOVA'의 5G MEC 기반 한국형 서비스 실증",
  "contact_email": "ygram.alien@gmail.com",
  "allocation": {
    "type": "dedicated",
    "node": "mec-wn04",
    "gpu_label": "gh200",
    "tier": "high"
  },
  "quota": {
    "cpu_req": "64",
    "cpu_lim": "70",
    "mem_req": "400Gi",
    "mem_lim": "500Gi",
    "gpu": 7,
    "storage": "1Ti",
    "pods": 150,
    "lb_services": 5
  },
  "options": {
    "deploy_starter_kit": true,
    "create_harbor_project": true,
    "generate_guide": true,
    "include_egress_policy": true,
    "ingress_domain": null
  }
}
```

**Response (202 Accepted)**:
```json
{
  "data": {
    "job_id": "job-7c4e2f81",
    "tenant_id": "ygram-poc",
    "status": "running",
    "stream_url": "/api/mec/v1/jobs/job-7c4e2f81/stream"
  },
  "meta": { ... }
}
```

**SSE 스트림 예시**:
```
event: progress
data: {"step": "create_namespace", "status": "completed", "duration_ms": 120}

event: progress
data: {"step": "apply_quota", "status": "completed", "duration_ms": 80}

event: progress
data: {"step": "deploy_starter_kit", "status": "running", "progress": 30}

...

event: done
data: {"status": "success", "tenant": {...}, "credentials": {...}}
```

### 4.2 `GET /api/mec/v1/nodes` (노드 목록)

**Response (200)**:
```json
{
  "data": [
    {
      "name": "mec-wn04",
      "roles": ["worker"],
      "status": "Ready",
      "capacity": {
        "cpu": "72",
        "memory": "555Gi",
        "nvidia.com/gpu": "7",
        "pods": "110"
      },
      "allocatable": { ... },
      "labels": {
        "tenant": "ygram-poc",
        "gpu": "gh200",
        "tier": "high",
        "dedicated": "true"
      },
      "taints": [
        {
          "key": "tenant",
          "value": "ygram-poc",
          "effect": "NoSchedule"
        }
      ],
      "architecture": "arm64",
      "node_info": {
        "kernel_version": "6.17.0-1014-nvidia-64k",
        "os_image": "Ubuntu 24.04.4 LTS",
        "cuda_driver": "590.48.01"
      },
      "allocated_resources": {
        "cpu_requests": "70",
        "memory_requests": "500Gi",
        "gpu_requests": 7,
        "pods": 25
      }
    },
    ...
  ],
  "meta": {
    "total": 7,
    "timestamp": "..."
  }
}
```

### 4.3 `POST /api/mec/v1/firewall/nat-rules` (NAT 규칙 추가)

**Request**:
```json
{
  "label": "nabi-ssh-246",
  "public_ip": "121.147.13.246",
  "private_ip": "172.20.26.199",
  "protocol": "tcp",
  "ports": [22, 8080],
  "source": "any",
  "create_proxy_arp": true,
  "enable_immediately": true
}
```

**Response (201 Created)**:
```json
{
  "data": {
    "id": "nat-15",
    "type": "dnat",
    "label": "nabi-ssh-246",
    "from_zone": "untrust",
    "to_zone": "trust",
    "public_ip": "121.147.13.246",
    "private_ip": "172.20.26.199",
    "service_groups": ["TCP_22", "TCP_8080"],
    "enabled": true,
    "created_at": "2026-04-20T15:40:00Z",
    "hits": 0
  }
}
```

### 4.4 `POST /api/mec/v1/nodes/{name}/gpu-mode` (GPU 모드 전환)

**Request**:
```json
{
  "mode": "time-slicing",
  "replicas": 2,
  "force": false
}
```

**Response (202 Accepted)** (비동기, 2~5분 소요):
```json
{
  "data": {
    "job_id": "job-gpu-switch-4eb7",
    "node": "mec-wn01",
    "from_mode": "vm-passthrough",
    "to_mode": "time-slicing",
    "status": "running",
    "estimated_duration_seconds": 300,
    "warnings": [
      "노드의 VFIO Passthrough VM이 없는지 확인하세요"
    ]
  }
}
```

### 4.5 `POST /api/mec/v1/docs/generate` (접속 가이드 생성)

**Request**:
```json
{
  "template": "tenant_access_guide",
  "format": "docx",
  "tenant_ids": ["ygram-poc", "sccreative-poc", "funit-poc", "witches-poc", "maninblock-poc"],
  "include_sections": ["summary", "starter_kit", "quota", "yaml_examples", "troubleshooting"],
  "language": "ko"
}
```

**Response (202)**:
```json
{
  "data": {
    "job_id": "job-doc-5f8a",
    "estimated_files": 5,
    "status": "running"
  }
}
```

**Job 완료 후 `/api/mec/v1/jobs/{id}`**:
```json
{
  "data": {
    "status": "completed",
    "artifacts": [
      { "filename": "접속가이드_와이그램.docx", "url": "/api/mec/v1/docs/downloads/abc123", "size": 46080 },
      ...
    ]
  }
}
```

## 5. 인증/권한

### 5.1 인증 헤더

```
Authorization: Bearer <JWT_TOKEN>
```

### 5.2 권한 체계

| 역할 | 권한 |
|------|------|
| `mec:admin` | 모든 MEC API 접근 |
| `mec:viewer` | 읽기 전용 |
| `mec:tenant-owner:{id}` | 특정 테넌트만 관리 |

**권한 체크 예시**:
```rust
#[post("/api/mec/v1/tenants")]
async fn create_tenant(
    auth: AuthExtractor,  // JWT 추출 및 검증
    body: web::Json<CreateTenantRequest>,
) -> Result<HttpResponse> {
    auth.require_role("mec:admin")?;  // 권한 체크
    // ...
}
```

## 6. Rate Limiting

| API 그룹 | 제한 |
|---------|------|
| 조회성 (`GET /...`) | 1000 req/min per user |
| 변경성 (`POST/PUT/DELETE`) | 60 req/min per user |
| 문서 생성 | 10 req/min per user |
| AXGATE 연동 | 30 req/min (전역, SSH 세션 보호) |

## 7. Webhook (Phase 3+)

외부 시스템(Slack 등)에 이벤트 알림:

```json
POST https://hooks.slack.com/...
{
  "event": "tenant.created",
  "tenant_id": "ygram-poc",
  "user": "admin",
  "timestamp": "..."
}
```

## 8. OpenAPI Spec

구현 시 `utoipa` crate로 자동 생성:

```
GET /api/mec/v1/openapi.json    # OpenAPI 3.0 spec
GET /api/mec/v1/docs             # Swagger UI
```
