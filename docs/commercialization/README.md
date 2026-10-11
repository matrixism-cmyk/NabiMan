# 나비맨 상용화 종합 계획

작성일 2026년 10월 5일. 대상은 나비맨이며 다른 제품의 출시 이력은 포함하지 않는다. 기존 [업그레이드 계획](../upgrade-plan.md)은 당시 구현 기록으로 유지하고, 상용화 판단에는 이 문서와 연결된 최신 조사·설계·실험 명세를 사용한다.

**최근 운영 반영:** 2026년 10월 6일 07:57 KST, [man.bebo.kr](https://man.bebo.kr/). 예약 작업의 기존 행 수정, 변경 전·후 및 기존 환경 검토, 외부 변경 차단, 표준 @예약어와 명령 끝 공백 보존을 배포했다. 서버 479개/프런트엔드 189개, 실제 crontab과 브라우저 검증을 통과했다. 운영 검증은 cron 쓰기 0건이며 서버 6개·등록 terminal 16개·tmux 17개·예약 작업 원문과 키를 보존했다. Webmin 2.670과 cron 삭제/외부 변경에 대한 제한된 실동작 비교도 각각 3회 수행했다. root API 분리, durable job, 잔여 의존성 위험, 독립 서명/보안 검토·전체 비교 실측·고객 검증은 미완료다. 세부 범위와 증거는 [실행 기록](execution.md)을 따른다.

## 목표와 첫 시장

나비맨을 브라우저에서 서버를 안전하게 운영하고 고객별로 위임할 수 있는 상용 제품으로 발전시킨다. 첫 시장은 **여러 고객의 Linux 서버와 컨테이너 환경을 관리하는 MSP와 중소기업 운영팀**, 첫 배포 방식은 **고객 환경에 설치하는 제품**으로 제안한다. 이는 아직 고객 인터뷰로 검증하지 않은 가정이다. 팀·예산·목표 고객에 대한 추가 정보가 없으므로 8명 기준 계획과 1~2명 축소안을 함께 제공한다.

초기 제품의 핵심 작업은 고객·환경을 선택하고, 접근을 요청·승인하며, 장애를 진단하고, 변경 계획을 검토·실행하고, 결과와 복구를 확인하는 것이다. 서버·Docker·Kubernetes·VPN·관측성 화면을 늘리는 것만으로 이 작업이 완성되지는 않는다. 조직과 실제 자원의 권한을 일치시키고, 운영자가 실행 결과를 믿을 수 있게 만드는 데 우선 투자한다.

세계 최고 수준이라는 목표는 특정 고객 작업의 안전성·완료 시간·성공률·복구 능력을 동등한 조건에서 비교해 입증하는 것으로 정의한다. 현재 나비맨 또는 다른 제품이 이 목표를 달성했다고 평가하지 않는다. 시장 규모, 점유율, 매출 예측, 경쟁 제품보다 빠르다는 수치는 아직 제시할 근거가 없다.

## 조사와 산출물

| 문서 | 내용 | 증거의 상태 |
|---|---|---|
| [제품과 연구논문 벤치마킹](benchmark.md) | 13개 제품군, 9편의 논문, 보안·운영 표준, 적용 방향과 한계 | 공식 문서와 논문의 핵심 설계 검토 |
| [목표 구조와 회원 및 tenant 설계](architecture.md) | 비특권 API·실행기, 조직·자원·인가·작업·감사·복구, tenant 설정, 기존 데이터 이전 | 구현을 위한 제안 |
| [벤치마크 실행 명세](benchmark-protocol.md) | 14개 실제 작업, 부하·부정 접근·고장·사용성 시험, 측정 환경과 수용 기준 | 향후 실험 계획 |
| [개발과 사업 실행 계획](roadmap.md) | 28개 작업, 선행 관계·공수·담당, 단계별 종료 조건, 인력·예산·가격·지원 | 가정에 기반한 계획 |
| [서비스 제어의 특권 분리](privileged-service-control.md) | 비특권 API·root helper, kernel UID/PID 입장, 허용 서비스/action, durable intent·중복 실행 차단·결과 조회 | C05 일부 개발 구현, 전체 설치·legacy 기능 이전과 systemd sandbox 검증 잔여 |
| [터미널 일회용 접속권과 접속 회수](terminal-admission.md) | query JWT 제거·대상/Origin binding·원자 소비·현재 계정 재검사·열린 연결 회수 | 운영 반영, 공개 HTTPS terminal 입출력 확인 |
| [터미널 세션 저장과 상태 확인](terminal-registry.md) | 단일 writer·원자 snapshot·생성/변경 전 용량 검사·조회 불가와 부재 구분·정확한 tmux 대상·환경설정 장애 격리 | 운영 반영·16개 session 이전, OS 작업·감사 트랜잭션과 HA 잔여 |
| [공유 터미널의 수명과 접속 회수](shared-terminal-access.md) | owner ID·읽기 전용 기본·진행 중 회수·원자 저장·감사 redaction·계정 승인 version·일회용 입장권·시도/연결 상한 | 검증 코드 운영 반영, 기존 9개 만료 링크 유지; 분산/복구 인가·JIT/기록 잔여 |
| [오프라인 라이선스 발급과 검증](license-signing.md) | 서버·발급자 분리, wire format, 공개키 rotation·이전과 실제 제한의 잔여 범위 | 검증 코드 운영 반영; 실제 유료 발급·한도 집행은 미완료 |
| [실측 결과 양식](benchmark-results-template.csv) | 버전·환경·반복·지표·증거를 기록할 CSV | 머리글만 있으며 측정값 없음 |

제품 문서는 기능과 설계를 확인한 자료다. 경쟁 제품을 설치해 비교한 결과, 독립 침투 시험 결과, 고객의 구매 의사를 대신하지 않는다. 모든 논문을 체계적 문헌고찰 방식으로 선정하거나 전부 재현한 것은 아니다. 상세 문서의 사실에는 공식 출처를 연결하고 나비맨 적용 방향은 제안으로 구분했다.

## 벤치마킹이 바꾼 설계 방향

| 확인한 설계 | 나비맨의 개발 결정 제안 |
|---|---|
| Cockpit의 OS 권한과 권한 상승 구분 | 웹 API를 일반 사용자로 실행하고 필요한 동작만 특권 helper에 위임한다. [공식 문서](https://docs.cockpit-project.org/cockpit-guide/main/guide/privileges.html) |
| Plesk의 고객·reseller·구독 구분 | 고객 조직, 관리자 소속, 실행 환경, 상품 한도를 별도 모델로 관리한다. [공식 문서](https://doc.plesk.com/en-US/obsidian/administrator-guide/customers-and-resellers.70622/) |
| Portainer와 Rancher의 환경·클러스터·프로젝트 권한 | 조직 역할과 실제 자원의 권한을 연결하고 유효 권한을 보여준다. [Portainer](https://docs.portainer.io/advanced/kubernetes-roles-and-bindings), [Rancher](https://ranchermanager.docs.rancher.com/v2.13/how-to-guides/new-user-guides/authentication-permissions-and-global-configuration/manage-role-based-access-control-rbac/cluster-and-project-roles) |
| Teleport의 임시 접근 요청과 세션 기록 | 개인 identity, 기간, 대상, 승인자, 기록, 회수를 하나의 접속 흐름으로 연결한다. [접근 요청](https://goteleport.com/docs/identity-governance/access-requests/), [기록](https://goteleport.com/docs/reference/architecture/session-recording/) |
| AAP의 workflow 승인 노드 | 실행 전 계획과 승인, 실행 직전 재인가, 사후 검사를 durable job에 포함한다. [공식 문서](https://docs.redhat.com/en/documentation/red_hat_ansible_automation_platform/2.7/develop-ref_controller_approval_nodes) |
| Zanzibar의 인가 변경과 데이터 일관성 문제 | 권한 회수가 캐시·열린 세션·대기 작업에도 전달되도록 version과 재검사를 설계한다. [논문](https://www.usenix.org/conference/atc19/presentation/pang) |
| Cedar의 명시적 인가 언어와 검증 | 허용 동작을 typed action으로 정의하고 policy 부정 시험을 만든다. Cedar 채택 자체는 PoC 후 결정한다. [논문](https://www.amazon.science/publications/cedar-a-new-language-for-expressive-fast-safe-and-analyzable-authorization) |
| OSDI 오류 처리 연구와 AgentDojo 공격 평가 | 연결·저장·재시도 실패를 주입하고, AI가 읽는 로그·문서도 신뢰할 수 없는 입력으로 시험한다. [오류 처리 논문](https://www.usenix.org/conference/osdi14/technical-sessions/presentation/yuan), [AgentDojo](https://arxiv.org/abs/2406.13352) |

DRF·Borg는 자원 한도와 선언적 운영의 설계에, Dapper는 관측성의 연결에, Raft는 다중 실행기와 fencing의 이해에, Saltzer와 Schroeder는 특권 최소화에 참고한다. 이 논문의 대규모 운영 결과를 나비맨의 성능이나 보안 수준으로 전용하지 않는다. 자세한 적용 조건과 한계는 비교 문서에 기록했다.

## 현재 코드에서 확인한 출시 위험

아래는 조사 시점의 작업 디렉터리 소스를 읽어 확인한 내용이다. 커밋된 코드와 아직 커밋되지 않은 변경이 함께 있는 상태이며, 배포 바이너리 전체와의 일치나 실제 공격 성공을 입증한 결과는 아니다. 위험 등급은 상용 출시 범위의 우선순위 판단이며 외부 취약점 등급이 아니다. 구체적인 악용 가능성과 영향은 별도 시험으로 확인해야 한다.

| 우선순위 | 확인한 코드 상태 | 상용화 영향과 필요한 변경 | 근거 |
|---|---|---|---|
| P0 | 서비스가 root 계정으로 실행 | API 침해가 호스트 권한으로 이어지는 범위를 줄이기 위해 특권 경계 분리 | [서비스 설정](../../nabiman.service) |
| P0 | 초기 조사에서 AXGate server key 무조건 허용 확인. 후속 구현에서 지문 일치 검증 적용 | AXGate 인증 전 차단과 만료된 legacy 예외를 검증. 일반 SSH·승인 UI·운영 rollout은 후속 범위 | [SSH handler](../../backend/src/mec/services/axgate_real/session.rs) |
| P0 | 초기 조사에서 내장 HMAC 비밀값과 unsigned 저장 정보 신뢰 확인. 후속 구현에서 Ed25519 검증으로 교체 | 발급자 개인키 분리·조회마다 재검증. 실제 기능과 한도 집행·기존 고객 재발급·운영 rollout은 후속 범위 | [라이선스 서명 운영](license-signing.md), [라이선스 구현](../../backend/src/license.rs) |
| P0 | 초기 조사에서 실제 connector 실패의 mock 전환 확인. 후속 구현에서 제거 | real·auto의 실패를 연결 불가와 503으로 표시하는 backend 구현·격리 검증 완료. 운영 배포와 UI 상세 상태는 후속 확인 | [서비스 초기화](../../backend/src/mec/services/bootstrap.rs) |
| P0 | 초기 조사에서 MEC DB 실패의 memory 전환 확인. 후속 구현에서 제거 | 영구 DB 오류·빈 경로·메모리 DB 지정 시 시작 중단 구현·검증 완료. 운영 rollout은 후속 작업 | [서버 초기화](../../backend/src/main.rs) |
| P0 | 초기 조사에서 job 저장 오류 무시 확인. 후속 구현에서 오류 전파·checkpoint와 event 경계 적용 | 저장 실패·atomic 종료·중복 ID·상태 전이 시험 통과. queue·복구·승인·실행 전 재인가·최종 transaction은 후속 범위 | [job runner](../../backend/src/mec/job_runner.rs) |
| P0 | users·membership·scope 정책은 파일 저장, MEC는 SQLite | 관련 변경의 transaction·migration·복원 경계를 정의하고 다중 관리 서버 전에 저장 모델 정리 | [회원 저장](../../backend/src/users.rs), [정책 저장](../../backend/src/scope_settings.rs), [MEC DB](../../backend/src/mec/db/mod.rs) |
| P0 | 승인 회원↔Rancher realm/account ID 명시적 연결·중복 방지·원자 감사 구현, scoped tenant 접근은 자원·quota 등 읽기 중심이고 legacy role 접근도 유지 | 역할명과 실제 쓰기 권한의 차이를 설명하고 legacy 계정을 명시적 grant로 안전하게 이전 | [인가 경로](../../backend/src/scope_access.rs) |
| P0 | 서버 등록 identity·credential이 서버 단위로 존재 | 사람별 실제 OS identity와 실행 범위를 연결하고 회수·회전·최소 권한을 검증 | [서버 모델](../../backend/src/models/remote.rs), [서버 등록](../../backend/src/remote_servers.rs) |
| P1 | 초기 조사에서 terminal·dashboard·download query JWT 사용 확인. 후속 구현에서 계정 ticket·header client로 교체 | 원자적 일회용 admission·현재 계정 인가·열린 계정 연결 회수. proxy ticket redaction·공유 링크·OS identity·JIT·운영 rollout은 후속 범위 | [계정 접속권](terminal-admission.md), [인증](../../backend/src/auth.rs) |
| P1 | 로컬 audit hash chain과 외부 전송 기능 존재 | 풍부한 작업·대상 증거, 전송 실패 처리, 독립 보존·검증을 추가해야 하며 hash만으로 관리자 변조 방지 보장 불가 | [감사 구현](../../backend/src/audit.rs) |
| P1 | API key CRUD는 있으나 중앙 인증 경로에서 해당 검증 연결을 확인하지 못함 | service account의 scope·만료·회수까지 실제 API E2E 인가 구현 | [API key](../../backend/src/api_keys.rs), [인증](../../backend/src/auth.rs) |
| P1 | 신규 가입·회원·scope 설정 화면에 한국어 직접 문자열 존재 | 국제화·접근성·오류 안내와 모바일 사용성을 지원 범위에 포함 | [가입 화면](../../frontend/src/components/RegistrationForm.tsx), [설정 화면](../../frontend/src/components/ScopeSettingsPanel.tsx) |
| P1 | CI와 release에 빌드·시험·checksum 등 기반이 존재 | artifact 서명·SBOM·출처 증명·실제 upgrade와 restore를 하나의 출시 gate로 완성 | [CI](../../.github/workflows/ci.yml), [release](../../.github/workflows/release.yml) |

현재 production 전체가 mock이라는 뜻은 아니다. 위험은 오류 상황에서 그런 코드 경로가 실행될 수 있다는 점이다. SQLite 또는 JSON 파일 자체를 취약점으로 단정하지 않으며, transaction과 실패 처리, 확장 topology에 필요한 조건을 지적한다. 이미 있는 인증·감사·CI를 제거하거나 다시 만들기보다 올바른 경계와 실행 증거를 보강한다.

현재 신규 가입 신청, 최고관리자 승인, 서버·tenant 소속과 역할, 접근 활성화·만료·명령 허용, scope 안내·인원 한도·명령 정책 관리가 기반으로 구현돼 있다. 개발 소스에는 최고관리자의 Rancher 계정 ID 연결·조회와 tenant 회원 삭제의 project 소속 검증도 추가했다. 외부 identity 연결은 provider 접근 권한을 자동으로 부여하거나 회수하지 않으며 운영 배포 전이다. 회원별 실제 tenant 접근 확인에서는 소속 정책과 Rancher 직접 프로젝트 연결의 차이 및 미검증 접근 경로를 표시한다. 관리자 위임 범위와 전체 PRTB 페이지 조회를 검증하는 개발 기반이다. 기존 최고관리자의 외부 회원 변경에는 durable intent·정확한 binding 소유권·결과 재조회·원자 감사·미완료 재시도 차단을 추가했고, MEC tenant의 외부 프로젝트 회원 화면에서 변경 기록을 조회한다. durable binding ID가 남은 결과 불명 요청은 최고관리자의 사유·정확한 요청 ID 확인 후 provider GET으로만 재확정한다. 최고관리자가 explicit identity·소유 binding·승인 소속을 연결하면 만료·소속 접근 해제·정지·역할 변경 시 주기 작업이 그 직접 binding만 회수하고 부재를 확인한다. 재활성화 시 자동 재부여하지 않는다. 신규 grant는 최고관리자의 명시적 identity/role 검토와 자동 회수 승인 intent를 먼저 저장하고 확인된 ownership·관리 정책·성공 감사를 함께 commit한다. candidate 없는 요청의 재조정, 미확정 managed cleanup, scoped 쓰기 위임·회수 SLA·queue/HA fencing는 남아 있다. 최고관리자는 `/#administration/usermgmt`, 소속 관리자는 같은 회원 화면에서 위임 범위의 하위 회원을 관리하고, `/#administration/scopeSettings`에서 자신의 scope 정책을 관리한다. 현재 tenant 설정 화면의 인프라 quota 조회와 향후 quota 변경·network·backup·workload 정책 집행은 구분해야 한다.

이전 구현 검증에서 backend 230건과 frontend 대상 suite 15건, 회원·scope smoke를 통과한 기록은 특정 변경의 근거다. 이번 문서 작업에서 전체 시험을 다시 실행하거나 상용 품질을 인증한 것은 아니다. 새 보안 경계·job·이전 작업은 별도 시험을 요구한다.

## 회원 관리와 tenant 상품 설계

사용자가 요청한 신규 가입 신청의 최종 승인은 **최고관리자**에게 유지한다. 조직·tenant 관리자는 승인된 회원 중 자신의 범위에 속한 하위 회원의 역할·접근 조건을 관리한다. 자신보다 높은 권한을 부여하거나 다른 고객의 회원을 조회·수정할 수 없어야 한다. 위임 상한과 승인 책임을 화면과 API 모두에 표현한다.

고객 조직은 계약·결제·회원의 경계이고, infrastructure tenant는 namespace·프로젝트 등 실제 실행 자원의 경계다. 두 개념을 동일한 ID로 고정하면 한 고객의 여러 환경이나 MSP의 여러 고객을 표현하기 어렵다. 기존 tenant는 실행 환경으로 유지하고 상위 Organization을 추가하는 설계를 제안한다.

| 설정 영역 | 상품에 필요한 기능 | 집행 위치 |
|---|---|---|
| 회원과 인증 | 소속·초대·최고 승인·MFA·IdP·일시 권한·만료 | API·session·외부 identity |
| 자원과 접속 | 서버·cluster·namespace·실제 OS 계정·credential·접속 허용 시간 | gateway·agent·SSH·Kubernetes |
| 변경과 실행 | 허용 action·위험 수준·계획·승인자·작업 시간대·동시 실행 한도 | durable job·실행 직전 policy·helper |
| 용량과 격리 | CPU·RAM·GPU·storage·pods·network·사용량·한도 | 실제 quota·CNI·runtime와 결과 재조회 |
| 백업과 감사 | 일정·보존·복원 승인·세션 기록·export·접근 로그 | backup 실행기·독립 감사 저장 |
| 운영과 상품 | 알림·담당자·maintenance·지원·상품 한도·고객별 원가 | 알림 routing·entitlement·지원 정책 |

기능 스위치나 설정 화면을 만들었다고 집행이 완료된 것은 아니다. 설정 적용 결과·오류·현재 상태 차이를 확인하고, 허용 자원 밖에서 실제 요청이 실패하는지 시험한다. Kubernetes namespace만으로 강한 격리가 완성되지 않는다는 공식 지침을 설계 기준으로 사용한다. [Kubernetes multi tenancy](https://kubernetes.io/docs/concepts/security/multi-tenancy/)

## 제품 구조와 도입 순서

기존 Rust backend와 React frontend를 중심으로 모듈 경계를 정리한다. 웹 API·인가·job·audit를 담당하는 control plane, 브라우저 접속 gateway, 고객 서버의 outbound 실행기, 최소 특권 helper를 분리한다. 데이터 저장은 단일 설치형의 transaction 저장부터 안정화하고 PostgreSQL과 HA는 기업 확장 단계에서 실제 필요와 운영 비용을 검증한다.

사용자 흐름은 조직·환경 선택 → 내 자원과 상태 → 접근 요청 → 장애 진단 → 변경 계획 → 승인 → 실행 → 결과 검증 → 필요 시 복구 순서다. 관리자 흐름은 신청함 → 회원과 유효 권한 → 환경 설정 → 정책 적용 상태 → 작업·감사 → 지원·상품 관리다. 모든 화면에는 대상 고객·환경과 현재 실제 데이터 상태를 일관되게 표시한다.

관측성은 기존 Netdata·Grafana·OpenTelemetry 등과 연동할 수 있게 하고, 초기부터 모든 저장소·가상화·자동화 도구를 재구현하지 않는다. 고객 작업을 완성하는 연동을 먼저 선정하고 인증·지원·라이선스 범위를 검증한다. AI는 읽기 진단과 승인 가능한 계획부터 도입하며 단독으로 root 명령을 실행하게 하지 않는다.

## 일정과 예산 제안

| 기간 | 우선 결과 | 다음 단계 조건 |
|---|---|---|
| 1~4주 | 고객 작업 검증·lab baseline·위협 모델·P0 재현·범위 확정 | 동일 문제의 고객 증거와 검토 가능한 설계 |
| 5~12주 | 특권 분리·저장·인가·실연결·credential 기반 | 격리·실패·이전·복원 시험 통과 |
| 13~24주 | 20~100대 범위의 제한된 유료 pilot | 안전한 접속·작업·감사·복구 및 지원 비용 증거 |
| 25~36주 | 제한된 지원 범위의 상용 GA | 독립 보안·부하·upgrade·restore·tenant gate 통과 |
| 37~52주 이후 | HA·MSP 연동·offline·규모 확장 | 실제 구매 수요와 운영·지원 역량 확인 |

T0는 팀·예산·범위·실험 환경이 확보된 시점이다. calendar가 지나도 gate가 실패하면 출시는 연기한다. GA 공수는 125~200 사람 주, 추가 확장은 선택에 따라 48~82 사람 주로 추정했다. 8명·36주·delivery 70% 가정의 약 202 사람 주는 상한과 가까워 일정 여유가 매우 작다. 4주차에 의존 관계와 실제 인력 역량을 반영해 재산정한다.

8명·12개월 계획의 예산 가정은 예비비를 포함해 **약 9.48~16.34억원**이다. 현재 시장 견적이 아니며 인력 비용, 외부 보안·계약 검토, lab·도구만으로 계산한 예시다. 고객 획득·출장·판매·24시간 지원 비용은 별도다. 1~2명은 Linux·SSH·최소 위임·안전한 작업·audit·복원에 범위를 제한하고, 2명·12개월의 예산 예시는 약 2.37~3.91억원이다. 좁은 첫 상품의 9~12개월 목표도 gate 통과를 전제로 하며 넓은 제품의 완성을 약속하지 않는다.

첫 상품은 자체 설치형 Team을 제안하고 수요에 따라 Enterprise·MSP 계약을 확장한다. 가격은 아직 확정하지 않았다. 관리 대상 host 또는 environment와 지원 수준을 중심으로 검증하고, 기본 보안·데이터 반환을 가격 때문에 생략하지 않는다. 현재 저장소 [LICENSE](../../LICENSE)의 내부 사용·별도 SaaS 계약 조건을 MSP 판매 권리와 함께 전문 검토한다.

## 출시를 결정할 측정 기준

아래는 실제 측정 결과가 아니라 초기 수용 기준 제안이다. 규모·hardware·protocol·외부 지연·지원 topology를 고정하고, 원시 측정값과 실패 증거를 함께 남긴다.

| 영역 | 초기 GA 수용 기준 제안 |
|---|---|
| 고객 격리 | 정의된 다른 조직·자원 접근 부정 시험 전부 차단, 독립 검토의 미해결 High·Critical 없음 |
| 설치와 첫 작업 | 지원 환경 설치 중앙값 15분 이내, 대표 첫 작업 성공률 90% 이상 |
| 인가 회수 | 연결된 환경에서 열린 접근까지 회수 p99 5초 이내, 단절 환경에서 권한 연장 기본 거부 |
| API | 100대·100사용자 명세 부하에서 내부 API p95 300ms·p99 1초, 외부 connector 지연 별도 기록 |
| 접속 | RTT 30ms 조건의 SSH 첫 prompt p95 3초 이내, 사람의 승인·MFA 대기 별도 |
| 복구 | 관리 데이터 RPO 24시간·RTO 2시간의 실제 drill, 암호화 key·사용자·인가까지 확인 |
| 부하와 안정성 | 50개 동시 SSH 세션 1시간, 24시간 soak, upgrade·단절·재시도·저장 실패 시험 |
| 운영 | 목표 관리 경로 SLO 99.9%의 관측 설계·pilot 실적 확인, workload SLA와 구분 |
| 공급망 | artifact·서명·SBOM·provenance 검증, 이전 버전 upgrade와 복구 증거 |

세부 실행 조건은 benchmark 명세를 따른다. 1,000개 가상 inventory와 1,000대 실제 접속 부하를 혼동하지 않는다. HA의 더 높은 복구·성능 목표는 이후 별도 검증한다. “작업 시간이 30% 줄어든다”는 차별화 가설은 사용자 실험에서 검증할 대상이며 현재 홍보 문구로 사용할 수 없다.

현재 구현·검증·배포 상태는 [상용화 실행 기록](execution.md)에 별도로 유지한다. 최근 구현은 신규 외부 권한과 자동 회수 정책의 동시 저장, 결과 불명 관리 요청의 최고관리자 조회, 저장된 정확한 연결 ID에 한정한 후속 확인·회수를 포함한다. 운영 배포 전이며, 연결 ID가 없는 응답 유실은 자동 복구하지 않는다. 최고관리자가 지정한 만료 위임에 따라 tenant 관리자가 자기 하위 회원의 새 managed 권한을 부여하고, 별도 회수 승인을 받은 경우 확인한 기존 managed 연결을 회수하는 경로도 구현했다. **MEC → 내 소속 tenant → 외부 권한 관리**에서 자신의 하위 회원의 관리 연결과 변경 상태를 확인한다. 기존 위임은 부여만 허용하며 회수 권한은 최고관리자가 명시적으로 추가해야 한다. 직접 연결의 부재와 전체 외부 접근 차단은 구분한다. 조회 오류나 잘못된 응답을 빈 목록으로 표시하지 않는다. 전역 identity와 외부 account 생성·raw binding CRUD·불명 결과 재조정은 최고관리자 전용이다. 계획 완료와 제품 완성을 구분하고 릴리스마다 실제 증거를 갱신한다.

## 첫 달에 확정할 의사결정

1. 첫 고객군과 지원 OS·SSH·컨테이너 topology를 정하고 제외 범위를 문서화한다.
2. 현재 host·권한·credential·저장소를 조사하고 상용 P0 재현·수정 항목에 담당자를 지정한다.
3. 최소 특권 helper, 저장 transaction, Organization·Grant·typed action 설계의 PoC를 검토한다.
4. 경쟁 제품의 필요한 edition을 고정하고 14개 시나리오 중 첫 고객 작업부터 실측한다.
5. 회원 신청·승인·접속·변경·복구 prototype을 운영자에게 시험하고 비용·구매 가설을 확인한다.
6. 4주차에 범위·공수·지원·출시 gate를 확정해 개발 backlog를 갱신한다.

이 계획의 문서 조사와 실행 설계는 완료됐고, 후속 제품 구현은 실행 기록에 따라 진행 중이다. 고객 연락·경쟁 제품 lab 설치·침투 시험·실측 결과 수집·전체 제품 완성은 수행 완료로 표시하지 않는다. 개발을 시작할 때 가장 먼저 현재 운영의 권한·데이터·실제 연결 경계를 안전하게 정리하고, 그 위에 고객이 반복해서 구매할 운영 흐름을 완성한다.
