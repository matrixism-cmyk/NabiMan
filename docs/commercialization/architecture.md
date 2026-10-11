# 나비맨 상용 제품 구조와 tenant 운영 설계

이 문서는 구현 완료 설명이 아니라 제안 설계다. 현재 Rust와 React를 유지하면서 인터넷 요청 처리, 인가, 특권 실행, 접속, 작업, 감사의 책임을 나눈다. 구조를 한 번에 전면 교체하지 않고 [로드맵](roadmap.md)에 따라 이전한다.

2026년 10월 5일 `/api/services/action`의 [제한된 root helper 경계](privileged-service-control.md)를 개발 구현했다. 이는 아래 목표 구조의 일부다. 기존 installer/Docker와 다른 legacy 관리 기능은 root 이전 대상이며, 사람별 OS identity·전체 중앙 인가·독립 감사·고객 이전을 완료한 구조로 해석하지 않는다.

## 제품 구성

초기에는 Rust 내부의 모듈 경계를 강화한 관리 서버와 독립 실행기를 기본으로 한다. API·인가·작업 관리까지 모두 microservice로 나누지 않는다. 네트워크에 노출되는 API와 root 실행 helper는 처음부터 프로세스·identity를 분리한다. 이 선택은 작은 보호 기구와 최소 권한 원리를 적용한 제안이다. [보안 설계 연구](https://web.mit.edu/Saltzer/www/publications/protection/Basic.html)

```text
운영자 브라우저
  └─ TLS / 로그인 / 조직·환경 선택
      └─ 비특권 관리 API
          ├─ Identity와 Policy 평가
          ├─ Inventory와 tenant 설정
          ├─ 변경 계획과 승인
          ├─ 작업 상태와 감사 기록
          └─ 세션 ticket 발급
              ├─ 세션 Gateway ── SSH / 원격 데스크톱
              └─ 실행기 Gateway ── mTLS ── 고객 실행기
                                             ├─ 읽기 수집기
                                             ├─ 제한된 typed operation
                                             └─ 별도 특권 helper

저장 계층
  ├─ 초기 단일 노드 SQLite transactional state
  ├─ Enterprise와 SaaS PostgreSQL state
  ├─ 외부 secret manager 또는 암호화 vault
  ├─ 감사·세션 기록 object storage
  └─ 기존 metrics·logs·traces backend 연동
```

SQLite와 PostgreSQL의 동시 지원은 유지 비용이 있다. 먼저 저장 repository 경계와 schema version을 정한 뒤, SQLite 단일 노드 상품을 검증한다. 다중 관리 서버가 필요할 때 PostgreSQL을 추가한다. HA를 JSON 파일 공유나 SQLite 네트워크 파일시스템 공유로 구현하지 않는다. PostgreSQL만 지원하도록 단순화할지는 고객의 설치 부담과 2주 PoC 결과로 결정한다.

작업 transport는 초기에 DB 기반 durable queue와 worker lease를 우선 평가한다. tenant 단위 소비 속도·backpressure 요구가 커질 때 별도 message broker를 선택한다. 어떤 broker를 택해도 원격 명령의 중복·부분 실패 문제는 별도로 해결해야 한다.

## 조직과 자원 모델

현재 `Membership(kind=server|tenant, scope_id, role)`은 초기 위임 모델이다. 상용 모델은 고객의 계약 조직과 Kubernetes tenant를 구분한다. 기존 `Tenant`는 infrastructure workspace로 이전하되 ID와 자원 연결을 유지한다.

| 모델 | 역할과 필수 필드 |
|---|---|
| Organization | 고객 또는 내부 조직. contract, owner, timezone, billing profile, status |
| Workspace | 조직 내 환경 또는 프로젝트. production 여부, 담당 팀, 허용 정책 |
| ManagedEnvironment | Linux host, Docker environment, Kubernetes cluster, 외부 hypervisor 연결 |
| Resource | server, service, container, namespace, workload, volume. organization_id, workspace_id, environment_id, immutable id |
| Principal | human, team, service account, runner, AI assistant. 조직과 수명·인증 방식 |
| Grant | principal, action 또는 role template, resource selector, expiry, conditions, policy version |
| AccessRequest | 신청자, 대상, 요청 권한, 이유, 승인자, 만료, 결정 |
| ChangePlan | 대상 집합, diff, 사전 조건, 위험 등급, 복구 경로, plan hash |
| Job | 승인된 plan, lease, fencing token, 단계, attempt, 결과, 외부 effect ID |
| CredentialReference | secret 자체 대신 provider·reference·회전·용도·소유권 |
| AuditEvent | actor, 조직, 대상, action, decision, approval, job, before·after 요약, reason, timestamp |
| Entitlement | 계약 상품과 한도. 보안 인가와 상품 사용 가능 여부를 독립 판단 |

다른 조직의 자원 ID를 요청 body에 넣는 경우, 목록·검색·export·SSE·WebSocket·작업 결과·감사 조회·비밀 참조에서도 조직을 검증한다. 자원 ID를 추측하기 어렵게 만드는 것만으로 격리를 대신하지 않는다. 신규 코드에서 tenant context는 서버가 검증한 값을 사용하고 클라이언트가 보낸 organization_id만 신뢰하지 않는다.

## 회원 가입과 관리 흐름

가입 신청 → 최고관리자 승인 → 조직·workspace 배정 → 필요 시 tenant 관리자에게 하위 회원 관리 권한 부여 → 실제 OS·Kubernetes 권한 발급 → 접속·작업 기록의 순서다. 최초 가입 승인 권한은 현재 사용자의 요구대로 최고관리자에게 유지한다. tenant 초대가 있더라도 승인 전 접근을 허용하지 않는다.

최고관리자는 가입 대기와 조직 전체 상태를 본다. tenant 관리자는 자기 조직·workspace의 승인된 회원을 추가하고 위임 가능한 권한만 부여한다. 관리자가 자신에게 없는 권한이나 플랫폼 최고관리자 권한을 하위 회원에게 줄 수 없다. 서비스 권한과 Unix user, SSH principal, Kubernetes service account는 별도 표로 연결한다.

관리자가 보는 회원 상세에는 다음 정보를 묶는다.

- 계정 상태, 인증 방법, MFA·복구 상태, 최근 활동과 소속.
- 실제 유효 권한과 그 권한이 생긴 이유, 적용 정책과 만료일.
- 연결 가능한 자원과 실제 실행 identity. 비밀값 자체는 표시하지 않는다.
- 현재 세션, 승인 대기, 최근 작업, 접근 회수 결과.
- 소속 추가·해제와 영구 역할 변경의 영향 미리보기.

## 권한 역할과 위임 한도

다음은 제안 기본값이며 custom role은 같은 action catalog로 표현한다. 계정 관리자와 서버 root를 동일한 권한으로 묶지 않는다.

| 역할 | 회원·권한 | 서버·서비스 | Kubernetes | 비밀·감사 |
|---|---|---|---|---|
| 최고관리자 | 최초 가입 승인, 조직 생성, 위임 한도 설정 | 전역 운영 정책과 복구 접근 | cluster와 quota 상한 관리 | 정책 변경과 긴급 접근 모두 감사 |
| 조직 관리자 | 승인된 소속 회원·팀·제한된 역할 관리 | 자원 배정과 운영 정책 | 배정된 workspace와 quota 요청 | 자기 조직 감사 조회, 비밀 사용 위임 |
| 인프라 운영자 | 회원 최고 권한 부여 불가 | 배정 host의 승인된 작업 | 배정 cluster의 별도 권한 | credential 사용과 평문 조회를 분리 |
| tenant 운영자 | 회원 역할 변경 불가 | 허용 service·container 작업 | 자기 namespace의 승인된 workload 작업 | secret 평문, node, cluster role 변경은 기본 거부 |
| 조회자 | 자기 계정 관리 | 상태·허용 로그 조회 | 배정 workload 상태 조회 | audit 민감 필드와 secret 조회는 별도 권한 |
| 감사자 | 권한 변경 불가 | 실행 불가 | 실행 불가 | 허용 범위의 감사·기록 조회와 export |
| 서비스 계정과 AI | 자기 identity에 지정된 수명·action | token scope 내 작업만 | 별도 resource selector | 사람의 세션과 권한을 재사용하지 않음 |

개발 구현의 위임 경로는 최고관리자가 tenant별로 승인한 manager ID·realm/project·read-only/project-member·최대 30일 expiry와 별도 grant/revoke action이다. 기존 위임의 이전은 grant만 허용하며 회수 권한을 자동 추가하지 않는다. 현재 접근 가능한 하위 회원의 신규 managed grant와 별도로 승인한 기존 managed binding 회수만 허용한다. 자기 자신·동급 관리자·global 최고관리자·foreign tenant와 project-owner는 제외한다. 현재 하위 소속이 유지되는 비활성 회원의 기존 연결은 회수할 수 있다. 저장·실행 직전의 version과 현재 역할을 검사하고 승인 snapshot을 intent에 남긴다. 위임 만료/해제는 미래 변경을 차단하고 기존 연결의 수명은 회원 source 정책으로 관리한다. 관리 목록은 저장된 소유권과 변경 상태이며 실제 유효 접근 전체의 증명이 아니다. 이 경로의 구현·시험과 미완성인 전역 custom role/approval 엔진의 범위는 [실행 기록](execution.md)에 구분했다. 운영 배포 전이다.

특히 `shell.exec`와 `service.restart`는 다르다. 셸 문자열에서 위험한 단어만 차단하는 방식으로 명령을 안전하게 제한하지 않는다. 제한된 운영 역할에는 allowlisted typed operation을 제공하고 자유 셸은 별도 고위험 권한과 실제 OS 격리를 요구한다.

인가 PoC의 우선 후보는 Rust에 내장할 수 있는 Cedar다. 정책 언어를 채택해도 resource ownership 전달 오류는 앱에서 막아야 한다. 관계 규모가 커지면 OpenFGA 계열도 비교하되 여러 엔진을 동시에 운영하지 않는다. [Cedar 연구](https://www.amazon.science/publications/cedar-a-new-language-for-expressive-fast-safe-and-analyzable-authorization)

## 정책 상속과 변경

플랫폼 최대 한도 → 조직 계약 한도 → workspace 정책 → 자원 정책 → 개인별 grant 순서로 권한을 좁힌다. 상위에서 금지한 권한을 하위에서 다시 활성화할 수 없도록 설계한다. temporary grant는 명시된 예외 승인 범위에서만 적용한다. 정책 변경은 version, diff, actor, effective_at, rollback 가능 여부를 저장한다.

승인 이후 실제 실행 직전에 현재 계정 상태·정책·자원 소속·plan hash를 재검사한다. 정책 캐시에는 조직·주체·자원·version을 포함한다. 권한 회수는 새 HTTP 요청뿐 아니라 대기 작업, 재시도, 열린 세션, 스트림 구독에도 적용한다. 목표 회수 전파 시간은 출시 단계별로 검증한다. [Zanzibar의 일관성 문제](https://www.usenix.org/system/files/atc19-pang.pdf)

## tenant별 설정 목록

현재 구현은 담당자·공지·회원 수 제한·서버 명령 실행 정책과 회원별 접근 제어다. 아래 추가 설정은 신규 개발 대상이다. 화면 입력과 실제 인프라 집행의 일치가 완료 조건이다.

| 설정 영역 | 제안 설정 | 설정 가능한 주체 | 실제 집행 위치와 검증 |
|---|---|---|---|
| 조직 기본 정보 | 이름, 담당자, timezone, 언어, 계약 종료, 운영 시간 | 조직 관리자 | UI·알림·audit 시간대를 UTC 저장과 분리 |
| 가입과 초대 | 초대 허용 대상, 유효 시간, 요청 소속, 대기 알림 | 최고관리자 정책 안의 조직 관리자 | 승인 전 인증·접속 거부 시험 |
| 인증 | MFA 필수, 허용 IdP, session idle·최대 수명, step up | 상위 정책 안의 조직 관리자 | token 발급, refresh, WS ticket, 실제 세션 검증 |
| 접속 | 허용 host·Unix principal, 시간·망 조건, JIT TTL, 동시 접속 수 | 위임 범위의 관리자 | gateway·OS·SSH CA 또는 승인된 credential 검사 |
| 실행 | 허용 action·service·path, 유지보수 시간, concurrency, 승인자 | 위임 범위의 관리자 | job admission와 runner 양쪽 검사 |
| 자원 | CPU·RAM·GPU·storage·Pod 한도, 예약량, 초과 요청 | 최고관리자 상한 안의 조직 관리자 | ResourceQuota·LimitRange·job queue와 수락 결과 검증 |
| 네트워크 | 기본 거부, 승인된 egress·DNS·ingress·LB, VPN | 플랫폼 제공 옵션 안의 관리자 | CNI와 실제 packet probe. 미지원 CNI면 격리 미지원 표시 |
| workload 보안 | 허용 registry·image digest, privileged 제한, hostPath·node 접근 | 플랫폼 보안 정책 안의 관리자 | admission·Pod Security와 실제 거부 시험 |
| 비밀 | secret provider, 용도·소유권, 회전 주기, 폐기, break glass | 비밀 관리 권한 보유자 | vault·gateway에서 참조만 사용하고 회수 검증 |
| 백업 | 데이터 범위, 목적지, 주기, 보존, encryption, restore 승인 | 예산·보존 상한 안의 관리자 | 실제 restore drill와 소유권 검사 |
| 알림 | 수신자, 중요도, escalation, maintenance mute, webhook | 조직 관리자 | 조직별 routing·재시도·유출 차단 |
| 감사 | 기록 범위, 보존, export·삭제 권한, session recording | 상위 보존 정책 안의 관리자 | 감사 backend와 object storage 수명 정책 |
| 비용 | 월 예산, 사용량 알림, soft·hard 한도, quota 증액 요청 | 계약 권한자 | 사용량 ledger. 예산 초과만으로 운영 workload 자동 삭제 금지 |
| 기능 정책 | 허용 connector·automation·AI, 외부 전송 허용 | 상위 정책 안의 관리자 | entitlement와 보안 정책 각각 평가 |

Kubernetes namespace만으로 적대적인 고객 사이의 완전한 격리가 생기지는 않는다. 고객 신뢰 수준에 따라 공유 namespace 정책, 별도 node pool, 별도 cluster 또는 가상 control plane을 선택한다. 지원 CNI·storage·runtime까지 검증한 topology만 판매한다. [Kubernetes multi tenancy](https://kubernetes.io/docs/concepts/security/multi-tenancy/)

## 원격 접속과 인증정보

기존 공유 비밀번호 연결은 migration 경로로 제한하고, 사람별 SSH certificate·Unix principal 또는 외부 접속 플랫폼 연동을 우선 설계한다. 외부 CA, IdP, vault 장애 시 긴급 접근은 별도 이유·수명·감사를 요구한다. 긴급 접근 기능 자체가 영구적인 tenant 경계 우회가 되어서는 안 된다.

OpenSSH의 최초 접속 신뢰와 서버 키 pinning·회전 승인 상태를 구분한다. AXGate를 포함해 모든 connector가 서버 identity를 검증하게 한다. 오래된 SSH 암호 알고리즘은 host별 예외·사유·기한을 갖도록 설계하고 전역으로 켜지 않는다.

WebSocket은 장기 bearer token을 URL에 직접 실어 보내는 방식을 짧은 수명의 단일 사용 session ticket으로 이전한다. origin, 대상, 사용자, 정책 version을 ticket에 연결하고 gateway 로그에서 credential을 제거한다. 기존 tmux 세션은 계정과 자원 ownership을 연결한 뒤 이전한다.

녹화는 화면 재생과 명령·자원 이벤트의 목적을 분리한다. 민감 내용의 최소화, 접근 권한, 암호화, 보존 기간, 저장 실패 시 동작을 고객 정책으로 명시한다. 법적 보존 필요 기간을 제품이 임의로 정하지 않는다. Teleport 문서는 기록 위치와 동기 기록 실패의 tradeoff를 설명한다. [세션 기록](https://goteleport.com/docs/reference/architecture/session-recording/)

## 변경 작업과 장애 복구

모든 변경의 상태는 Draft → Validated → PendingApproval → Queued → Running → Verifying → Succeeded 또는 Failed·Partial·NeedsRecovery로 표현한다. cancel과 compensation 결과도 별도로 남긴다. 승인자는 실제 대상을 보고 승인하며 대상·인자가 바뀌면 plan hash가 바뀌어 기존 승인은 무효화된다.

작업에는 durable 상태, idempotency key, resource lock, worker lease와 fencing token을 둔다. “재전송하지 않았으니 한 번만 실행했다”는 보장은 하지 않는다. SSH 연결이 끊겨 성공 여부를 모르면 UnknownResult로 표시하고 사후 조회·수동 조정을 제공한다. package update처럼 자동 rollback이 불가능한 작업은 복구 가능성을 과장하지 않는다.

전체 fleet 변경은 사전 검사 → 작은 canary → 관측 → 점진 확대 순서로 실행한다. 실패율 또는 오류 예산을 넘으면 다음 batch를 멈춘다. 고객 workload는 관리 서버 장애와 독립적으로 계속 실행되어야 한다.

## 저장과 감사

파일 기반 users·memberships·정책·credentials를 transaction이 있는 저장소로 이전한다. 초기엔 SQLite 백업·schema migration을 검증하고, HA 단계에 PostgreSQL을 검증한다. PostgreSQL RLS를 보조 경계로 쓰면 app 계정에 owner·superuser·BYPASSRLS를 주지 않는다. [RLS 공식 문서](https://www.postgresql.org/docs/current/ddl-rowsecurity.html)

local hash chain은 우발적 손상 검출에 도움이 될 수 있지만 root가 전체 기록을 재작성하면 독립 증거가 되지 않는다. tenant별 audit event를 외부 보존 저장소로 전송하고 서명·외부 checkpoint·object retention을 검증한다. 저장 실패는 숨기지 않고 정책에 따라 위험한 신규 변경을 거부한다. 운영 관측 로그의 sampling과 감사 이벤트 저장은 별도 경로다.

backup에는 DB, 정책, credential 복구에 필요한 암호화 키, SSH host trust, 연결 설정, audit 참조가 포함된다. 암호화된 데이터만 백업하고 키 복원 경로를 빠뜨리는 경우도 시험한다. 다른 tenant의 backup·restore·export를 거부하는 시험을 포함한다.

## 실제 데이터와 demo 데이터

connector는 Connected, Unreachable, Unauthorized, Expired, Misconfigured, Unsupported, Demo를 명시적으로 구분한다. production 연결 실패 후 자동으로 fixture를 반환하지 않는다. demo mode는 별도 환경·저장 경로·브라우저 표시를 갖고 실자원 변경을 하지 않는다.

각 화면은 마지막 성공 시간과 데이터 source를 표시한다. 전체 목록이 비어 있는 상태와 연결 실패 상태를 구분한다. UI에서 적용 중인 정책과 실제 인프라 적용 결과도 분리해 drift를 알려준다.

## 라이선스와 배포

공통 HMAC 비밀을 제품에 포함하는 구조를 비대칭 서명 검증으로 이전한다. issuer에만 private signing key를 두고 제품은 public verification key를 가진다. key id·회전·계약 한도·offline 기간·clock skew를 명세한다. 라이선스 만료가 OS 서비스 중지·데이터 삭제·복구 불능으로 이어지지 않도록 유예·제한 정책을 별도로 정의한다.

배포는 signed artifact, SBOM, provenance, checksum, 지원 matrix, offline 설치·검증, versioned schema migration, 사전 backup과 rollback 문서로 구성한다. 자동 업데이트는 고객 maintenance window와 canary를 지원한다. 변경된 binary뿐 아니라 frontend와 schema의 호환성을 같이 확인한다. [SLSA](https://slsa.dev/spec/v1.2/)

초기 지원 조합은 인터뷰와 CI 결과로 확정한다. 제안은 지원 중인 Linux LTS 두 계열과 x86_64부터 시작하고 ARM64·Windows·복잡한 hypervisor 관리 확대는 구매 수요가 확인된 뒤 진행하는 것이다.

## AI 운영 지원

AI는 최초에는 지표·로그 요약, 원인 후보, runbook 검색, 변경 계획 작성만 지원한다. 외부 모델로 전송할 수 있는 데이터의 조직 정책과 민감 값 제거를 적용한다. 고객이 외부 전송을 금지하면 해당 기능을 끄거나 검증한 자체 운영 모델을 선택하게 한다.

실행 도구는 AI도 일반 service account처럼 인가한다. LLM 출력은 실행 권한이 아니며 승인된 typed operation만 작업 경로로 보낸다. prompt injection, 타 tenant 데이터 검색, 비밀 반출, 명령 대상 바꾸기를 평가한다. 품질 기준을 충족한 작은 action만 이후 자동화를 검토한다. [AgentDojo](https://arxiv.org/abs/2406.13352)

## 기존 설치의 이전 순서

1. 실제 데이터·demo 연결·legacy 전역 권한·공유 credential·세션·라이선스 버전을 inventory한다.
2. backup과 복원 시험을 먼저 끝낸다. 계정 password hash와 TOTP secret을 API 출력과 분리해 보존한다.
3. 신규 DB와 grant 모델로 dry run import를 수행하고 ID·소속·권한의 차이를 보고한다.
4. legacy 전역 계정은 자동으로 권한을 넓히지 않는다. 최고관리자가 소속과 역할을 명시적으로 확정하게 한다.
5. 신·구 인가를 shadow 평가해 결과 차이를 고친 뒤 점진 전환한다. shadow mode가 작업을 중복 실행해서는 안 된다.
6. 사람별 credential과 session ticket을 적용하되 기존 세션을 무조건 삭제하지 않고 ownership·재인증·기한 정책으로 이전한다.
7. DB schema의 되돌리기 가능 여부를 릴리스마다 기록하고, 되돌릴 수 없는 변경은 backup restore 계획으로 검증한다.
8. 최소 두 이전 버전의 upgrade·restore 검증을 통과한 후 고객 확대를 허용한다.
