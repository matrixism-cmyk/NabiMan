# 나비맨 경쟁 제품과 연구논문 벤치마킹

조사 기준일은 2026년 10월 5일이다. 이 문서는 공식 문서와 논문의 핵심 설계를 검토한 결과이며, 경쟁 제품을 설치해 측정한 성능 비교 결과는 아니다. 제품 문서는 기능의 존재와 설계 의도를 뒷받침하지만 실제 사용성, 장애 복구 성공률, 보안 수준을 증명하지 않는다. 다음 단계의 실측 방법은 [벤치마크 실행 명세](benchmark-protocol.md)에 정의한다.

상용화 전략에 사용할 사실, 나비맨에 적용할 제안, 아직 검증하지 않은 가설을 구분한다. 아래 제품별 적용 방향과 위험 평가는 조사자의 판단이다. 명시하지 않은 기능을 해당 제품이 지원하지 않는다고 해석하면 안 된다. SaaS, 자체 설치형, 무료판과 기업판은 동일 조건으로 비교하지 않는다.

## 비교 대상 선정

직접 경쟁하는 Linux 서버 관리 제품, 호스팅 사업자의 고객 관리 제품, 컨테이너 관리 제품, 안전한 원격 접속 제품, 자동화 제품, 관측성 제품으로 범위를 나눴다. 특정 제품 하나를 그대로 복제하면 다른 고객 작업을 놓칠 수 있어 13개 제품 또는 제품군을 선정했다.

| 제품 또는 제품군 | 공식 문서에서 확인한 설계 | 나비맨에 적용할 제안 | 비교 시 주의할 범위 |
|---|---|---|---|
| Cockpit | 로그인 사용자의 OS 권한과 Polkit·sudo를 통한 권한 상승 | 웹 API와 특권 실행을 분리하고 OS의 권한 체계를 존중 | 사용자 세션 기반 호스트 관리와 SaaS 고객 격리는 별개다. [권한 문서](https://docs.cockpit-project.org/cockpit-guide/main/guide/privileges.html) |
| Webmin | 사용자·그룹에 모듈 접근과 모듈 내부 제한을 부여 | 역할 이름뿐 아니라 서비스·경로·계정별 실제 허용 범위를 제공 | 모듈 접근 허용과 안전한 자유 셸 실행을 혼동하지 않는다. [사용자 관리](https://webmin.com/docs/modules/webmin-users/) |
| Plesk | 고객, reseller, 서비스 계획과 구독을 구분 | MSP 조직, 고객 조직, 상품 한도, 실사용 자원을 별도 모델로 설계 | 호스팅 구독과 Kubernetes namespace는 동일한 tenant가 아니다. [고객과 reseller](https://doc.plesk.com/en-US/obsidian/administrator-guide/customers-and-resellers.70622/) |
| cPanel과 WHM | reseller 기능을 선택적으로 위임하고 자원 제한을 설정 | 고객 관리자를 최고관리자와 구분하고 위임 가능한 기능을 명시 | root 수준의 reseller 권한은 강한 권한이다. [reseller 권한](https://docs.cpanel.net/whm/resellers/edit-reseller-nameservers-and-privileges/) |
| Portainer | 자원 접근 통제와 Kubernetes 역할 매핑을 구분하고 Edge Agent를 제공 | 환경별 역할, 자원 소유권, NAT 내부에서 외부로 연결하는 실행기 | 상세 RBAC 문서는 Business Edition 범위다. [자원 접근](https://docs.portainer.io/advanced/access-control), [Kubernetes 역할](https://docs.portainer.io/advanced/kubernetes-roles-and-bindings), [Edge Agent](https://docs.portainer.io/advanced/edge-agent) |
| SUSE Rancher Manager | 전역, 클러스터, 프로젝트 역할과 소속을 구분 | 조직과 실행 환경의 권한 계층을 분리하고 상속을 설명 | Rancher 프로젝트는 고객 계약·과금 조직을 대신하지 않는다. [클러스터와 프로젝트 역할](https://ranchermanager.docs.rancher.com/v2.13/how-to-guides/new-user-guides/authentication-permissions-and-global-configuration/manage-role-based-access-control-rbac/cluster-and-project-roles) |
| Proxmox VE | 자원 경로별 사용자·그룹·API token 권한과 VM 관리·백업을 제공 | 자원 상세에서 권한·작업·복구를 연결하고 실제 유효 권한을 보여준다 | 가상화와 HA 클러스터 전체를 초기 제품에 재구현하지 않는다. [공식 기능](https://proxmox.com/en/products/proxmox-virtual-environment/features) |
| Teleport | 세션 기록과 임시 권한 요청을 제공하고 기록 위치의 보안 차이를 설명 | 접속 요청, 만료, 실제 OS identity, 기록, 회수까지 일관된 흐름으로 설계 | 완전한 웹 JIT 요청은 Enterprise 범위다. 접속 녹화가 모든 활동의 완전한 감사라는 뜻은 아니다. [세션 기록](https://goteleport.com/docs/reference/architecture/session-recording/), [접근 요청](https://goteleport.com/docs/identity-governance/access-requests/) |
| Apache Guacamole | 브라우저 기반 원격 접속과 세션 기록 재생을 문서화 | 터미널 외 원격 데스크톱 요구가 확인되면 접속 계층을 연동 | 기록 저장·권한·프로토콜마다 검증 조건을 별도로 둔다. [매뉴얼](https://guacamole.apache.org/doc/gug/), [기록 재생](https://guacamole.apache.org/doc/gug/recording-playback.html) |
| Red Hat Ansible Automation Platform과 AWX | AAP workflow approval node는 실행 흐름을 중지하고 승인·시간 제한을 적용 | 변경 계획, 승인, 실행, 검증, 복구가 연결된 작업 모델 | 여기서 확인한 사실은 AAP 2.7 문서다. AWX의 특정 버전에 동일 기능이 있는지는 별도 검증한다. [승인 노드](https://docs.redhat.com/en/documentation/red_hat_ansible_automation_platform/2.7/develop-ref_controller_approval_nodes) |
| Rundeck과 Runbook Automation | ACL 정책과 프로젝트 수준 실행기 관리 | 승인된 운영 절차를 비개발자도 반복 실행하게 하되 입력을 제한 | OSS와 상용 Runbook Automation의 GUI·Runner 범위를 분리한다. [ACL](https://docs.rundeck.com/docs/learning/getting-started/acl-overview.html), [실행기 관리](https://docs.rundeck.com/docs/administration/runner/runner-management/managing-runners.html) |
| Netdata | Space·Room에 대한 역할과 접근 모델을 제공 | 운영자가 자기 담당 자원의 건강 상태부터 보도록 구성 | 모니터링 데이터 접근과 서버 쓰기 권한은 별도로 검사한다. [역할 모델](https://learn.netdata.cloud/docs/netdata-cloud/authentication-%26-authorization/role-based-access-model) |
| Grafana | 서버 관리자, 조직 역할, dashboard·folder 권한을 구분 | 관측성 화면의 권한을 원천 데이터 조회까지 일치시킨다 | dashboard를 숨기는 것만으로 데이터 소스 접근을 차단할 수 없다. 세밀한 데이터 소스 권한은 Enterprise·Cloud 범위를 확인한다. [역할과 권한](https://grafana.com/docs/grafana/latest/administration/roles-and-permissions/) |

## 제품 비교에서 얻은 결론

공통으로 필요한 것은 한눈에 보이는 자원, 설명 가능한 권한, 검증 가능한 실행 결과, 장애 시 복구 경로다. 제품군마다 중심 작업은 다르다. Linux 관리, 호스팅 고객 운영, Kubernetes 운영, 접속 통제, 자동화, 관측성을 하나의 점수로 합치면 제품 선택을 왜곡한다.

나비맨의 차별화 가설은 다음과 같다. MSP 또는 작은 운영팀이 여러 고객의 Linux·Docker·Kubernetes 자원을 관리할 때, 접속·진단·변경 승인·실행·복구·감사를 한 작업 흐름으로 제공하면 도구 전환과 실수를 줄일 수 있다. 한국어 사용성과 자체 설치의 간편함도 구매 이유가 될 수 있다. 현재 이 가설의 고객 인터뷰·구매 증거는 없다.

기존 솔루션보다 “기능이 많다”는 주장은 구매 가치가 아니다. 먼저 서버 등록, tenant 위임, 장애 조사, 안전한 변경, 복구라는 다섯 작업에서 시간·오류·보안 조건을 비교해야 한다. 어느 작업에서 우월한지 입증한 뒤 제품 메시지를 정한다.

### 접근 관리 위임의 구현 기준 보강

2026년 10월 5일 공식 자료를 추가 확인했다. [Teleport reviewer 설정](https://goteleport.com/docs/identity-governance/access-requests/access-request-configuration/)은 role별 review 조건을 명시하고, [Rancher project role](https://ranchermanager.docs.rancher.com/v2.14/how-to-guides/new-user-guides/authentication-permissions-and-global-configuration/manage-role-based-access-control-rbac/cluster-and-project-roles)은 회원 관리 권한을 별도로 정의한다. 여기에서 나비맨의 위임을 manager ID·현재 tenant/realm/project·role ceiling·expiry·approval version으로 제한하고 실제 dispatch 직전에 확인한다는 설계 기준을 도출했다. 이는 나비맨의 설계 선택이며 경쟁 제품에 같은 저장·회수 구현이 있다는 주장이 아니다. 신규 managed grant에 적용한 개발 구현과 실제 시험 범위는 [실행 기록](execution.md)에 기록했다. 유사 솔루션의 직접 설치 비교, custom role 분석과 2인 승인 전체는 잔여 gate다.

## 상용 모델 비교

가격 페이지는 수시로 변한다. 여기서는 과금 단위를 확인하며, 할인·세금·환율·파트너 계약이 다른 가격을 단순 비교하지 않는다.

| 공식 자료에서 확인한 모델 | 나비맨의 가격 설계에 주는 시사점 |
|---|---|
| Portainer는 node 기준 가격과 별도 지원 수준을 안내한다. [가격](https://www.portainer.io/pricing) | 운영자 수 과금 대신 관리 대상 자원 기준을 우선 실험한다. node 정의와 CPU tier 조건까지 읽어야 비교할 수 있다. |
| cPanel은 account 수 기반 상품을 안내한다. [가격](https://www.cpanel.net/pricing/) | 고객 계정 수가 직접 가치와 연결되는 호스팅 제품의 모델이다. 나비맨의 계정 또는 tenant 수를 그대로 같은 단위로 가정하지 않는다. |
| Teleport의 웹 JIT 요청은 Enterprise 기능이다. [접근 요청](https://goteleport.com/docs/identity-governance/access-requests/) | 기업이 승인·감사 기능에 가치를 둘 가능성이 있다. 이것만으로 실제 지불 의사가 입증되지는 않는다. |
| Plesk에는 reseller와 서비스 계획 모델이 있다. [고객과 reseller](https://doc.plesk.com/en-US/obsidian/administrator-guide/customers-and-resellers.70622/) | MSP 판매에서는 기술 라이선스뿐 아니라 고객 위임·계약·지원 권한을 함께 제공해야 한다. |

공통 보안 통제, 업데이트의 무결성, tenant 간 기본 격리를 유료 옵션으로 해제하는 구조는 채택하지 않는다는 제품 정책을 제안한다. 기업 상품은 HA, 지원, 감사 보존, 대규모 관리, 외부 시스템 연동으로 차등화한다.

## 연구논문 선정과 검토 방법

인가, 특권 분리, 운영 모델, 자원 공정성, 관측성, 장애 검증, 합의, AI 보안을 다루는 9개 논문·연구 보고서를 선택했다. 연구의 핵심 모델과 적용 한계를 검토했다. 오래된 논문은 설계 원리를 제공하며 현재 라이브러리·제품 기능은 별도의 공식 문서로 확인한다. 논문의 대규모 서비스 성능을 나비맨이 달성했다고 해석하면 안 된다. 이 목록은 체계적 문헌고찰 전체나 모든 최신 연구의 망라를 의미하지 않는다.

### Zanzibar 2019

Google의 인가 시스템은 관계 기반 권한 모델과 권한 변경·콘텐츠 변경의 일관성을 다룬다. 오래된 권한이 새로운 자원에 적용되는 문제를 설명한다. [USENIX 논문 소개](https://www.usenix.org/conference/atc19/presentation/pang), [논문 본문](https://www.usenix.org/system/files/atc19-pang.pdf)

**적용 제안:** 조직, 팀, 사용자, 환경, 서버, namespace의 관계를 일관되게 모델링한다. 권한 변경에 버전을 부여하고, 실행·재접속·스트림·캐시마다 최신 권한을 확인한다. “소속 삭제 직후 기존 세션으로 실행 가능한가”를 시험한다.

**한계:** Google의 전역 분산 저장소와 처리 규모를 초기 제품에 복제할 필요는 없다. 중앙 DB와 짧은 수명의 권한 캐시로 시작하고, 여러 리전에 걸친 관계 조회가 필요해질 때 분산 인가를 재검토한다.

### Cedar 2024

Cedar는 RBAC·속성·관계 조건을 표현하는 인가 언어다. 정책 검증·분석과 Rust 구현을 설명한다. 모델의 성질이 검증되어도 나비맨이 principal·action·resource를 잘못 넘기면 전체 애플리케이션은 안전하지 않다. [Amazon Science 논문](https://www.amazon.science/publications/cedar-a-new-language-for-expressive-fast-safe-and-analyzable-authorization)

**적용 제안:** Rust 내장 인가 엔진의 우선 후보로 평가한다. URL 문자열 조건을 `action × resource × principal × context`로 치환하고, 금지 우선·기본 거부·정책 설명을 제공한다. 기존 권한 모델과 동일 입력의 결과를 비교하는 차등 시험을 한다.

**한계:** Cedar, OPA, OpenFGA를 동시에 도입하면 운영 복잡도가 커진다. PoC에서 한 방식을 선택한다. 정책 엔진의 테스트 결과를 tenant 격리의 완전한 증명으로 홍보하지 않는다.

### Saltzer와 Schroeder 1975

최소 권한, 안전한 기본값, 모든 접근의 인가 검사, 작은 보호 기구 같은 보안 설계 원리를 정리한 연구다. [MIT 본문](https://web.mit.edu/Saltzer/www/publications/protection/Basic.html)

**적용 제안:** 외부 요청을 받는 API를 비특권 사용자로 실행한다. root 작업은 작은 실행 helper에 한정하고 typed operation·대상·인자를 다시 확인한다. tenant 관리자에게 최고관리자의 공유 SSH 계정을 넘기지 않는다.

**한계:** 작은 모듈이나 Rust 사용만으로 안전성이 보장되지 않는다. 운영자의 합법적 실수, OS 권한 설정, 세션과 파일의 모든 접근 경로를 함께 검사한다.

### Borg 2015

Google의 cluster manager는 선언적 작업 모델, admission control, 자원 공유와 격리, 운영 관측을 설명한다. [Google Research](https://research.google/pubs/large-scale-cluster-management-at-google-with-borg/)

**적용 제안:** 원하는 상태와 실제 상태를 분리한다. 변경은 사전 검사 → 계획 → 승인 → 실행 → 결과 확인을 거친다. quota와 실행 한도를 작업 수락 단계에서도 검사하고, 부분 실패를 별도 상태로 보여준다.

**한계:** 나비맨에서 Kubernetes scheduler를 재구현하지 않는다. 기존 orchestrator와 Ansible 등의 기능을 연동한다. overcommit은 고객 격리와 자원 계약이 정의되기 전 기본 활성화하지 않는다.

### Dominant Resource Fairness 2011

여러 자원을 사용하는 사용자에게 가장 많이 점유한 자원 비율을 기준으로 배분하는 모델이다. [USENIX](https://www.usenix.org/conference/nsdi11/dominant-resource-fairness-fair-allocation-multiple-resource-types), [논문 본문](https://www.usenix.org/events/nsdi11/tech/full_papers/Ghodsi.pdf)

**적용 제안:** CPU, 메모리, GPU, 저장 공간을 하나의 숫자로 합치지 않고 tenant별 제한·예약·사용량·요청 대기열을 구분한다. 장기적으로 공유 GPU 작업의 공정성 정책을 실험한다.

**한계:** 단순 quota는 DRF 구현이 아니다. GPU 이질성, MIG, topology, gang scheduling은 원래 모델만으로 해결되지 않는다. 출시 초기에는 계약된 hard quota와 실행 동시성 제한부터 적용한다.

### Dapper 2010

Google의 분산 추적 보고서는 낮은 overhead, 공통 계층 계측, sampling을 통한 운영 가치를 설명한다. [Google Research](https://research.google/pubs/dapper-a-large-scale-distributed-systems-tracing-infrastructure/)

**적용 제안:** 브라우저 요청부터 API, 작업 queue, 실행기, SSH 또는 Kubernetes 호출까지 같은 trace·job 식별자로 연결한다. 현재 구현에는 OpenTelemetry 표준을 우선 사용한다. [신호 모델](https://opentelemetry.io/docs/concepts/signals/)

**한계:** 성능 추적과 보안 감사는 다르다. 추적을 sampling하더라도 권한 변경·승인·실행·민감 자원 조회의 감사 기록은 sampling하지 않는다. 로그·trace에 비밀값을 넣지 않는다.

### Simple Testing Can Prevent Most Critical Failures 2014

분산 데이터 시스템의 실제 실패를 분석해 오류 처리와 입력 순서 시험의 중요성을 설명한다. [USENIX](https://www.usenix.org/conference/osdi14/technical-sessions/presentation/yuan), [논문 본문](https://www.usenix.org/system/files/conference/osdi14/osdi14-paper-yuan.pdf)

**적용 제안:** 정상 설치뿐 아니라 디스크 가득 참, DB 쓰기 실패, 실행기 연결 끊김, 인증 만료, 중복 요청, 작업 중 재시작을 시험한다. 저장 실패 후 성공으로 표시되거나 실제 연결 실패가 demo로 바뀌면 출시를 차단한다.

**한계:** 다른 시스템의 실패 비율을 나비맨의 예상 실패율로 옮기지 않는다. 작은 테스트 환경에서 재현 가능하다는 연구가 실제 규모 부하 시험을 대신하지도 않는다.

### Raft 2014

복제 로그의 합의에서 leader election, log replication, safety를 분리해 설명한다. [USENIX](https://www.usenix.org/conference/atc14/technical-sessions/presentation/ongaro)

**적용 제안:** HA 설계에서 리더 교체와 stale worker가 같은 변경을 중복 실행하는 상황을 시험한다. job lease, fencing token, idempotency를 명세하고 저장소 장애 모드를 문서화한다.

**한계:** 나비맨 자체 합의 알고리즘을 새로 만들지 않는다. 검증된 DB·coordination 제품을 이용한다. Raft를 채택했다는 사실만으로 원격 명령의 exactly once 실행이 보장되지는 않는다.

### AgentDojo 2024

외부 도구가 반환한 불신 데이터에 대한 prompt injection으로 agent의 동작이 바뀔 수 있음을 시험하는 연구다. [논문](https://arxiv.org/abs/2406.13352)

**적용 제안:** 서버 로그·파일·웹 결과를 지시로 해석하지 않게 하고, AI는 자신의 제한된 서비스 identity로만 도구를 호출한다. 초기 AI는 읽기·진단·실행 계획 작성만 수행하며 변경은 기존 인가·승인 경로를 거친다. 악성 로그가 비밀 반출이나 타 tenant 실행을 유도하는 시험을 만든다.

**한계:** prompt 문구를 강화하거나 사람 승인 버튼 하나를 넣는 것만으로 안전해지지 않는다. AI task success와 공격 성공률을 별도로 측정하고 adaptive attack도 시험한다. 해당 연구의 수치를 나비맨의 방어 성능으로 주장하지 않는다.

## 보안과 운영 기준 문서

| 공식 문서 | 확인한 원칙 | 나비맨에 적용할 제안 |
|---|---|---|
| [NIST SP 800 207](https://csrc.nist.gov/pubs/sp/800/207/final) | 네트워크 위치만으로 신뢰를 부여하지 않는다 | 사용자·기기·자원·실행기 identity를 검사한다. “Zero Trust 인증 제품”이라고 주장하지 않는다. |
| [OWASP ASVS](https://owasp.org/projects/asvs) | 애플리케이션 보안 통제의 검증 요구사항 | 확인한 5.0.0을 기준으로 요구사항 ID와 시험 증거를 고정한다. 기본 L2를 목표로 하되 특권 실행·비밀 관리에는 더 엄격한 통제를 적용한다. |
| [SLSA 1.2](https://slsa.dev/spec/v1.2/) | 빌드 provenance와 공급망 검증을 위한 track·요구사항 | checksum에 추가로 artifact 서명, provenance, SBOM, 배포 전 검증을 제공한다. 평가 전 특정 level 달성을 주장하지 않는다. |
| [WebAuthn](https://www.w3.org/TR/webauthn-3/) | 웹용 공개키 기반 인증 API | passkey·보안키, 복구 수단, 관리자 step up을 설계한다. 브라우저·authenticator 지원 범위를 별도로 시험한다. |
| [SPIFFE](https://spiffe.io/docs/latest/spiffe-about/overview/) | workload identity를 위한 표준 | 실행기 mTLS와 identity 수명을 관리한다. SPIRE 구축은 고객 규모에 따라 선택한다. |
| [Kubernetes multi tenancy](https://kubernetes.io/docs/concepts/security/multi-tenancy/) | API·데이터 plane 격리와 자원 공유 고려사항 | RBAC, quota, network policy, storage·node 격리를 함께 설계한다. |
| [Pod Security Standards](https://kubernetes.io/docs/concepts/security/pod-security-standards/) | Pod의 보안 수준과 제한 사항 | 고객 namespace의 privileged·hostPath·host network 같은 escape 경로를 제한하고 예외 승인을 기록한다. |
| [PostgreSQL RLS](https://www.postgresql.org/docs/current/ddl-rowsecurity.html) | 행 접근 정책과 superuser·owner 등의 bypass 조건 | 애플리케이션 계정을 table owner·BYPASSRLS로 실행하지 않는다. RLS와 API 인가를 함께 시험한다. |
| [Google SRE의 SLO 구현](https://sre.google/workbook/implementing-slos/) | 사용자 관점의 지표와 목표·오류 예산 | 관리 API, 작업 수락, 세션 접속을 분리해 측정하고 위험한 릴리스를 제한한다. |

특히 NetworkPolicy는 적용 가능한 CNI가 있어야 동작한다. YAML 생성 성공을 격리 성공으로 처리하면 안 된다. 실제 패킷 흐름을 시험한다. [Kubernetes 설명](https://kubernetes.io/docs/concepts/security/multi-tenancy/)

## 아직 확보하지 않은 증거

고객 인터뷰, 전체 지원 환경에서의 경쟁 제품 실설치 결과, 독립 침투 시험, 대규모 성능 수치, 제품별 지원 대응 품질, 고객 지불 의사, 장기 운영 비용은 미확인이다. 이 자료가 없으므로 전체 시장 점유율, 시장 규모, 제품 종합 순위, 매출 예측을 수치로 제시하지 않는다.

문서의 사실 확인은 기준일에 접근 가능한 자료에 한정된다. 구매 전에 edition·계약·지원·가격을 다시 확인하고, PoC에서는 실제 설치 버전과 image digest를 고정한다. 라이선스가 허용하는 사용·연동 범위와 dependency의 배포 의무는 별도 검토한다.


## 2026-10-06 — 최초 제한적 실동작 비교

[Webmin 2.670 공식 릴리스](https://github.com/webmin/webmin/releases/tag/2.670)의 원본을 받아 GitHub asset SHA-256과 대조하고, 별도 최소 chroot에 설치했다. 공식 [Scheduled Cron Jobs 문서](https://webmin.com/docs/modules/scheduled-cron-jobs/)가 설명하는 기존 작업 삭제를 대상으로 했다. 나비맨은 07:33 KST 배포된 바이너리의 동일 해시를 사용했다. 두 제품 모두 실제 distro crontab을 사용했으며 cron daemon은 실행하지 않았다.

| 관찰 항목 | Webmin 2.670 | 배포된 나비맨 |
|---|---|---|
| 선택한 작업 삭제 후 나머지 주석·환경변수·탭·순서 보존 | 3/3 바이트 일치 | 3/3 바이트 일치 |
| 화면 조회 후 외부에서 마지막 주석 추가 | 주석을 유지하며 선택 작업 삭제 3/3 | 검토 revision 불일치로 409, 원문 전체 보존 3/3 |
| 삭제 화면을 연 뒤 선택 작업 앞에 새 작업 삽입 | 새 작업 삭제, 원래 선택 작업 유지 3/3 | 브라우저 제출도 409, 두 작업 모두 보존 3/3 |

Webmin의 관찰 결과는 이 버전·cron 모듈·조건에 한정한다. 제3자 코드의 idx 기반 재조회 경로와 일치하지만 제품 전체의 안전성 평가나 공개 보안 취약점 판정은 아니다. 나비맨도 revision 확인 직후 외부 writer와의 모든 경쟁을 원자적으로 막는 것은 아니다.

두 시험은 같은 호스트의 별도 최소 chroot, loopback HTTP, root 시험 계정을 사용했다. kernel/PID/network namespace는 공유한다. Webmin setup은 postinstall을 생략했고 OS 탐지는 Debian 12.0이었다. Authentic 테마의 독립 페이지 자동화가 완료되지 않아 번들 gray-theme로 전환했으며, 자동화 문제를 제품 실패로 점수화하지 않았다. 나비맨의 외부 주석 시나리오는 API로 검증했고, 외부 작업 삽입은 API와 실제 브라우저로 검증했다. 전체 지원 배포판 설치, 기본 테마 사용성, 성능·대규모·tenant 격리·실제 예약 실행은 비교하지 않았다.

증거는 `/workspace/nabiman/benchmarks/webmin-cron-20261006-v1/`의 metadata, 두 제품별 3회 결과, crontab 전후 원문, 스크린샷, 재현 코드, 18행 CSV, SHA-256 목록이다. 다운로드 해시는 GitHub asset 대조이며 독립 publisher 서명 검증은 아니다. 나비맨의 변경 대상 검토를 유지하는 기준을 확인한 첫 모듈 실험이고, 전반적인 경쟁 우위나 GA 완료 근거로 확대하지 않는다.
