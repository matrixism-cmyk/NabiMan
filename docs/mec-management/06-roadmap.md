# 06. 구현 로드맵

## 1. 단계별 계획

### Phase 0: 기반 구축 (1주)

**목표**: K8s/Rancher/AXGATE 연동 기반 레이어 구축

| Week | 작업 | 산출물 |
|------|------|-------|
| W1 Mon-Tue | `KubeService` 구현 (kube-rs) | Namespace/Pod/Service 기본 CRUD |
| W1 Wed | `RancherService` 구현 | Project/User CRUD |
| W1 Thu | `AxgateService` 구현 (russh) | NAT 규칙 조회/추가 |
| W1 Fri | SQLite 스키마 + Audit Logger | `mec.db` 초기화 |

**검증 기준**:
- [ ] `cargo test` 모든 외부 시스템 연동 테스트 통과
- [ ] Mock 환경에서 각 Service가 정상 동작
- [ ] Audit Log가 SQLite에 저장됨

### Phase 1: MVP - 테넌트 관리 (2주)

**목표**: 테넌트 생성/조회/삭제를 UI에서 완전 수행

| Week | 작업 | 산출물 |
|------|------|-------|
| W2 Mon-Wed | `POST/GET/DELETE /api/mec/v1/tenants` | 테넌트 CRUD API |
| W2 Thu-Fri | Job 큐 + SSE 진행 상황 | `job` 모듈 |
| W3 Mon-Tue | 프론트: 테넌트 목록 페이지 | `/mec/tenants/list` |
| W3 Wed-Thu | 프론트: 신규 테넌트 마법사 | `/mec/tenants/new` (4-step) |
| W3 Fri | 프론트: 테넌트 상세 + Audit | `/mec/tenants/:id`, `/mec/audit` |

**검증 기준 (Acceptance Test)**:
- [ ] UI 클릭으로 `sample-poc` 테넌트 생성 (노드 지정, GPU 할당, Quota 적용)
- [ ] 생성 후 Rancher UI에서 실제 Project/User 확인
- [ ] UI에서 삭제 시 Namespace + Project + User + NAT 규칙 모두 정리
- [ ] Audit Log에 모든 작업 기록

### Phase 2: 노드/GPU/네트워크 관리 (2주)

**목표**: 노드 Taint/GPU 모드 전환, LB/Ingress 관리

| Week | 작업 | 산출물 |
|------|------|-------|
| W4 Mon-Tue | 노드 API (`/nodes`, taint, label) | `nodes` 모듈 |
| W4 Wed-Thu | GPU API (`/gpu/overview`, mode-switch) | `gpu` 모듈 |
| W4 Fri | 네트워크 API (`/network/*`) | `network` 모듈 |
| W5 Mon-Tue | 프론트: MEC 대시보드 | `/mec/dashboard` |
| W5 Wed | 프론트: 노드 목록/상세 | `/mec/nodes/*` |
| W5 Thu | 프론트: GPU 시각화 | `/mec/gpu/overview` (bar chart) |
| W5 Fri | 프론트: 네트워크 페이지 | `/mec/network/*` |

**검증 기준**:
- [ ] 대시보드에서 전체 자원 현황 실시간 확인
- [ ] UI에서 노드에 Taint 추가/제거
- [ ] mec-wn01을 VFIO ↔ Time-Slicing 전환 (진행 상황 SSE)
- [ ] LB IP Pool 사용률 시각화

### Phase 3: 방화벽 + 문서 생성 (2주)

**목표**: AXGATE 통합 관리, 접속 가이드 자동 생성

| Week | 작업 | 산출물 |
|------|------|-------|
| W6 Mon-Tue | 방화벽 API (`/firewall/*`) | `firewall` 모듈 |
| W6 Wed | AXGATE 동기화 로직 | `firewall_cache` 무효화 |
| W6 Thu-Fri | 프론트: 방화벽 페이지 | `/mec/firewall/*` |
| W7 Mon-Tue | 문서 생성 엔진 (docx-rs) | `docgen` 모듈 |
| W7 Wed-Thu | 프론트: 문서 생성 페이지 | `/mec/docs` |
| W7 Fri | 통합 테스트 + 문서화 | 사용자 매뉴얼 |

**검증 기준**:
- [ ] UI에서 공인 IP 할당 + NAT 규칙 추가 → 외부 접속 테스트
- [ ] 테넌트 5개에 대한 접속 가이드 docx 일괄 생성
- [ ] 생성된 docx가 기존 Python 스크립트 결과와 동등한 내용

### Phase 4 (선택): 고급 기능 (2-4주)

| 기능 | 설명 | 예상 기간 |
|------|------|---------|
| Harbor 이미지 관리 | 이미지 목록, 취약점 스캔 결과 | 1주 |
| Prometheus/Grafana 임베드 | 대시보드 내부에 메트릭 차트 | 1주 |
| 스타터킷 템플릿 커스터마이징 | 기업별 추가 Pod 템플릿 등록 | 1주 |
| 다중 클러스터 지원 | 여수/광주 확장 대비 | 2주 |
| Slack/Email 알림 | Quota 초과, 장애 감지 시 | 1주 |

## 2. 전체 일정

```
Week 1: Phase 0 (기반 구축)
Week 2-3: Phase 1 (MVP - 테넌트 관리)
Week 4-5: Phase 2 (노드/GPU/네트워크)
Week 6-7: Phase 3 (방화벽/문서)
Week 8+: Phase 4 (선택 기능)
```

**MVP 도달**: Week 3 말 (3주)
**전체 구현 완료**: Week 7 말 (7주)

## 3. 작업 분해 (WBS)

### Phase 1 MVP 상세

```
Phase 1: MVP - 테넌트 관리
├── Backend
│   ├── [B1] TenantService.create()
│   │   ├── KubeService.create_namespace()
│   │   ├── KubeService.apply_resource_quota()
│   │   ├── KubeService.apply_limit_range()
│   │   ├── KubeService.apply_network_policies() (5개)
│   │   ├── KubeService.apply_rbac()
│   │   ├── KubeService.create_pvc() (workspace)
│   │   ├── KubeService.label_node() + taint_node()
│   │   ├── RancherService.create_project()
│   │   ├── RancherService.create_user()
│   │   ├── RancherService.create_prtb()
│   │   └── AuditLogger.log()
│   ├── [B2] TenantService.delete()
│   │   ├── KubeService.delete_namespace() (cascade)
│   │   ├── RancherService.delete_project()
│   │   ├── RancherService.delete_user()
│   │   ├── KubeService.remove_node_taint()
│   │   └── AuditLogger.log()
│   ├── [B3] TenantService.list() / get()
│   ├── [B4] Job Queue + SSE endpoint
│   └── [B5] SQLite Audit Log
├── Frontend
│   ├── [F1] API client (React Query hooks)
│   ├── [F2] 테넌트 목록 페이지
│   ├── [F3] 신규 테넌트 마법사 (4-step form)
│   ├── [F4] 테넌트 상세 페이지 (탭)
│   └── [F5] Audit Log 페이지
└── Testing
    ├── [T1] 단위 테스트 (Service 레이어)
    ├── [T2] 통합 테스트 (실제 클러스터)
    └── [T3] E2E 테스트 (Playwright)
```

## 4. 위험 요소 및 완화 방안

| 위험 | 영향 | 확률 | 완화 방안 |
|------|------|------|---------|
| Rancher API 변경 | 중 | 낮 | 기존 operator-kit 스크립트와 호환 유지, Rancher v2.12 고정 |
| kube-rs 크레이트 버전 호환성 | 중 | 중 | `k8s-openapi` 버전 핀 고정, MSRV 명시 |
| AXGATE SSH 세션 불안정 | 높 | 중 | 연결 풀링 + 재시도 로직, 조회 결과 캐싱 |
| 동시 요청 시 K8s API 부하 | 중 | 낮 | Rate Limiting + 쿼리 배치 |
| NabiMan 기존 기능과 충돌 | 높 | 낮 | feature flag로 MEC 모듈 on/off |
| **맨인블록 기존 NS 데이터 보존** | 높 | 완료 | ✅ 삭제됨 |
| **기업 담당자 교육 부담** | 중 | 중 | 접속 가이드 docx 자동 생성 (이미 스크립트 존재) |

## 5. 품질 기준

### 5.1 코드 품질

- Rust: `cargo clippy -- -D warnings` 통과
- Rust: `cargo fmt` 적용
- TypeScript: ESLint + Prettier
- 단위 테스트 커버리지: 70% 이상
- 통합 테스트: 주요 시나리오 커버

### 5.2 성능

| 메트릭 | 목표 |
|-------|------|
| API 평균 응답 시간 (GET) | < 200ms |
| 대시보드 초기 로드 | < 2초 |
| 테넌트 생성 전체 소요 | < 60초 |
| 동시 접속 사용자 | 10명 이상 문제없음 |

### 5.3 보안

- [ ] 모든 API는 JWT 검증 필수
- [ ] AXGATE 크리덴셜은 암호화 저장 (age/sops)
- [ ] Audit Log에 민감 정보 마스킹 (비밀번호, 토큰)
- [ ] CSRF 방어 (NabiMan 기존 방식 준수)
- [ ] SQL Injection 방어 (prepared statements)

## 6. 배포 전략

### 6.1 개발 환경

```
Local Dev → Docker Compose (nabiman + mock k8s)
         → 실제 MEC 클러스터 연동 (VPN 필요)
```

### 6.2 스테이징

MEC 클러스터 내 별도 네임스페이스에 배포:
```
nabiman-staging NS
├── nabiman-mec Deployment (개발 버전)
└── mec.db (별도 PVC)
```

### 6.3 프로덕션

```
방안 A: 기존 NabiMan 인스턴스에 통합 (권장)
  - 기존 배포 방식 준수
  - feature flag로 점진적 공개

방안 B: 별도 nabiman-mec 인스턴스
  - 독립 운영 가능
  - 인증은 SSO/공통 세션
```

## 7. 완료 정의 (Definition of Done)

Phase 1 MVP가 다음을 만족하면 완료:

- [ ] 운영 관리자가 **UI 클릭만으로** 테넌트 1개 생성 가능
- [ ] 기존 operator-kit 스크립트와 **동등한 결과** 생성 (NS, Project, User, Quota, RBAC, PVC)
- [ ] **Audit Log**에 모든 변경 이력 기록
- [ ] **Job 진행 상황**이 SSE로 실시간 전달
- [ ] **접속 가이드 docx** 자동 생성 및 다운로드
- [ ] 기존 NabiMan의 서버 관리 기능에 **영향 없음**
- [ ] **사용자 매뉴얼** 작성 (`I:/docs/mec-management/user-guide.md`)

## 8. 릴리즈 계획

| 버전 | 내용 | 시점 |
|------|------|------|
| v0.1.0-alpha | Phase 0 + 테넌트 GET API | W1 말 |
| v0.2.0-alpha | Phase 1 Backend 완료 | W2 말 |
| **v1.0.0-beta** | **Phase 1 MVP 완료 (UI 포함)** | **W3 말** |
| v1.1.0 | 노드/GPU/네트워크 추가 | W5 말 |
| v1.2.0 | 방화벽/문서 생성 추가 | W7 말 |
| v2.0.0 | Phase 4 고급 기능 | W11+ |

## 9. 참여자 및 책임 (예시)

| 역할 | 담당 |
|------|------|
| 제품 오너 | MEC 운영팀 |
| 백엔드 개발 | Rust 개발자 1명 |
| 프론트엔드 개발 | React 개발자 1명 |
| QA | 1명 (겸직 가능) |
| 검수 | 김전일/이광선 자문위원 |

## 10. 참고 자료

- NabiMan 기존 구조: `I:\`
- operator-kit 스크립트: `/root/mec-project/operator-kit/scripts/`
- 운영 가이드: `C:\Users\master\5G MEC\JCIA_5G_MEC_실증기업_서비스배포_접속가이드.docx`
- 자원 할당 계획: `C:\Users\master\5G MEC\1차_실증기업_자원할당_계획_260401.md`
- 이 설계 문서: `I:\docs\mec-management\`
