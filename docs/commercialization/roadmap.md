# 나비맨 상용화 개발과 사업 실행 계획

이 일정과 공수는 제안이며 확정 견적이 아니다. T0는 핵심 인력, 첫 고객 범위, lab, 예산이 확보된 시점이다. 8명 팀을 기준으로 약 9개월에 제한된 범위의 GA를 목표로 하고, 12개월 이후 기업 확장을 진행한다. “모든 경쟁 제품의 전체 기능을 1년 안에 대체”하는 약속은 하지 않는다.

## 단계와 종료 조건

| 단계 | 기간 | 제품 결과 | 종료 조건 |
|---|---|---|---|
| 발견과 위험 제거 | 1~4주 | 고객 문제, benchmark baseline, 위협 모델, 출시 범위 | 고객 인터뷰 15건 제안, design partner 3곳 확보 목표, source audit P0 작업 발행, PoC 결정 |
| 보안과 데이터 기반 | 5~12주 | 비특권 API, 저장 transaction, action 인가, 실제 연결 상태 | privilege boundary·tenant 부정 시험, credential·backup 이전 검증, production mock fallback 차단 |
| 제한된 유료 pilot | 13~24주 | 실제 20~100 host 규모 고객별 설치, 접속·작업·감사 흐름 | pilot 조건과 장애 대응 합의, 실제 작업·복구 증거, 고객별 사용성·운영 비용 측정 |
| 상용 GA | 25~36주 | 지원 matrix, signed release, license·지원·업그레이드 상품 | 독립 보안 검토, upgrade·restore·부하·tenant gate 통과, 유료 전환 증거 |
| 기업 확장 | 37~52주 | 더 큰 fleet, HA PoC·상품 검증, MSP 연동과 운영 지원 | 1,000 inventory와 실제 host 부하의 분리 검증, 실패 topology와 지원 조건 명시 |
| 글로벌 확장 | 13~18개월 이후 | 검증된 HA·offline 기업 상품, connector 확대, 제한된 AI | 실제 수요와 지원 역량 확인, 별도 scale·attack 시험과 매출·비용 검증 |

calendar만 지나갔다고 다음 단계에 진입하지 않는다. 초기 3개 고객이라도 제각각 다른 요구를 모두 custom feature로 구현하면 일정이 무너진다. 구매 이유와 재사용성이 높은 작업부터 공통 제품으로 만든다.

## 우선 개발 항목

공수의 단위는 사람 주다. 한 명의 1주 작업량을 뜻하며 calendar 기간이 아니다. 각 항목은 구현·검증·문서화를 포함한 초기 추정이며 dependency·리스크는 4주차에 재산정한다. 높은 범위의 공수와 GA에 포함할 범위를 동시에 고정하면 일정이 늘어난다.

| ID | 우선순위 | 작업과 수용 기준 | 의존 | 주 책임 | 예상 공수 |
|---|---|---|---|---|---|
| C01 | P0 | 13개 제품군 중 작업별 후보의 lab 설치, 인터뷰, 위협 모델, benchmark 결과와 범위 결정 | 없음 | 제품·QA·SRE | 4~6 |
| C02 | P0 | 실제 연결 실패·DB 실패를 명시적 오류로 표시, mock·in memory fallback을 production에서 차단 | 없음 | backend | 3~5 |
| C03 | P0 | 모든 SSH connector의 host identity 검증과 회전 승인, legacy 예외 기한 | 없음 | agent·보안 | 3~5 |
| C04 | P0 | license 비대칭 서명, issuer 분리, offline·만료·회전 시험 | C01 | backend | 2~4 |
| C05 | P0 | 비특권 API와 최소 helper 분리, root endpoint 격리·typed action·설치 hardening | C01 | agent·SRE | 10~16 |
| C06 | P0 | users·membership·정책 저장 transaction, schema migration와 rollback·복원 검증 | C01 | backend | 6~10 |
| C07 | P0 | 중앙 action catalog·유효 권한 설명·기본 거부·legacy 이전, 모든 접근 경로 부정 시험 | C06 | backend·QA | 8~12 |
| C08 | P0 | 개인 identity와 credential reference, 만료·회전·실제 OS scope, 외부 CA·vault 연동 최소 버전 | C03·C07 | agent·backend | 6~10 |
| C09 | P0 | ticket 기반 WS, 세션 ownership·회수·재접속, 기본 기록과 replay·보존 최소 버전 | C07·C08 | frontend·agent | 6~10 |
| C10 | P0 | durable job·계획·승인·실행 직전 재인가·canary·UnknownResult·사후 검사 | C05·C06·C07 | backend·agent | 10~16 |
| C11 | P0 | 조직별 audit·actor·대상·plan·결과 연결, 보존과 export·저장 실패 처리 | C06·C07·C10 | backend·SRE | 6~10 |
| C12 | P1 | 작업 중심 navigation, 조직·환경 선택, 가입·회원·자원 흐름, 오류 구분·i18n·접근성 | C01·C07 | frontend·UX | 8~12 |
| C13 | P1 | tenant 인증·실행·자원·알림 설정의 최소 subset와 실제 집행·drift 표시 | C07·C10 | backend·frontend | 6~10 |
| C14 | P1 | metrics·logs·trace 연결과 데이터 신선도, connector별 health·알림 routing | C02·C10 | SRE·frontend | 5~8 |
| C15 | P0 | 실제 관리 데이터 backup·key 복원·tenant restore 권한, 복원 drill | C06·C08 | SRE·QA | 6~10 |
| C16 | P0 | SBOM·artifact 서명·provenance·CVE 검토·rollback, release gate 자동화 | C01 | SRE·QA | 5~8 |
| C17 | P1 | 지원 OS·설치·업데이트·offline bundle 최소 범위, 사전 조건 진단 | C05·C06·C16 | SRE | 4~6 |
| C18 | P1 | service account·API token 인가 연결, OpenAPI와 scope·만료·회수 E2E | C07 | backend | 4~6 |
| C19 | P0 | tenant·특권·부하·고장·외부 침투 시험과 수정 회귀 | C05~C18 | QA·외부 보안 | 10~16 |
| C20 | P1 | design partner 설치·운영·유료 pilot, 작업 비교·복구·비용·구매 이유 기록 | C01·P0 gate | 제품·SRE | 5~8 |
| C21 | P1 | 상품·지원·MSP 계약 범위·가격 실험·견적·license 발급 운영 | C01·C04 | 제품·사업 | 4~6 |
| C22 | P2 | PostgreSQL·관리 서버 HA·작업 fencing·gateway drain·scale topology | GA 기반 | backend·SRE | 12~20 |
| C23 | P2 | MSP 고객 import·inventory sync·Ansible·Rancher·Proxmox adapter 우선 subset | C18·고객 수요 | backend·agent | 8~14 |
| C24 | P2 | offline 기업 인증·업데이트·감사 보존과 상세 tenant 정책 확대 | GA 기반 | SRE·보안 | 8~14 |
| C25 | P2 | 서명된 확장 manifest·권한·version contract와 격리 PoC | C07·C16 | backend | 6~10 |
| C26 | P2 | AI 읽기 진단·계획, 민감 값 필터와 prompt injection·tenant 평가 | C11·C14·C19 | backend·QA | 8~14 |
| C27 | P2 | ARM64·지원 OS·추가 protocol 확대의 수요 검증과 자동 compatibility 시험 | C17 | agent·QA | 6~10 |
| C28 | P1 | support runbook·diagnostic bundle·incident·고객 상태·보안 통지 운영 | pilot | SRE·제품 | 4~6 |

P0는 보안·데이터 손실·운영 진실성·상용 권리 관련 출시 차단 작업이다. P1은 선택한 첫 고객에게 실제로 필요한 상품 작업이다. P2는 기반과 수요가 확인된 뒤의 확장이다. C19의 외부 진단 비용은 공수와 별도로 예산에 포함한다. 표의 모든 확장을 36주 GA에 넣지 않는다.

## 처음 네 주의 실행 순서

| 주 | 수행 내용 | 검토 가능한 산출물 |
|---|---|---|
| 1주 | 현재 설치 topology·자원·credentials·권한·실제 connector 조사. P0 재현 조건 작성. 고객 인터뷰 시작 | current state inventory, threat model, source findings, 고객 작업 목록 |
| 2주 | Cockpit·Webmin·Portainer·Teleport·AAP 등 필요한 edition PoC. privilege helper와 저장소·policy PoC | 버전 고정 lab, B01~B06 baseline, 선택지별 설치·운영 부담 |
| 3주 | 회원·tenant·자원·변경 UX prototype, API·action catalog·migration 설계, 가격·지원 가설 인터뷰 | user flow, 권한 표, API contract, 가격 실험 기록 |
| 4주 | 지원 범위·첫 유료 pilot 조건·공수 재산정. 보안·복원·benchmark gate 합의 | 확정 release scope, ADR, 소유자·dependency 있는 backlog, pilot 제안서 |

고객에게 새로운 메시지를 보내거나 인터뷰를 예약하는 것은 실제 고객 연락 권한과 대상이 정해진 뒤 별도 수행한다. 여기서는 인터뷰 설계와 작업 계획을 제공한다.

## 임계 경로와 범위 조정

인가와 저장소 이전(C06·C07)이 credential·세션·작업·감사 기능의 공통 선행 작업이다. 특권 분리(C05)가 안전한 서버 쓰기의 선행 작업이다. C15·C16·C19는 GA 직전의 장식이 아니라 이전 단계부터 진행해야 하는 release 기반이다.

일정이 밀리면 ARM64·RDP·GPU scheduling·SDK·AI를 줄인다. tenant 격리, 서버 identity 검증, 작업 결과 정직성, backup·restore, artifact 무결성은 축소하지 않는다. GA 범위는 실제 SSH·Linux 서비스 관리와 하나의 검증된 컨테이너 topology부터 제한할 수 있다.

## 인력 구성과 공수 해석

| 역할 | 인원 | 핵심 책임 |
|---|---|---|
| 제품·고객·사업 책임 | 1 | 범위, 인터뷰, pricing·계약·지원 정책 |
| Rust backend | 2 | 저장소·인가·jobs·audit·API |
| agent와 접속 보안 | 1 | privilege boundary·SSH·실행기·세션 |
| frontend | 1 | 작업 흐름·관리 화면·브라우저 안전성 |
| UX·디자인 | 1 | 신규·숙련 운영자 흐름, 접근성·i18n·사용성 시험 |
| SRE·배포 | 1 | lab·설치·backup·관측성·release·지원 |
| QA·품질 | 1 | 계약·부정·고장·부하·upgrade 시험 |

총 8명이며 전문 침투 시험과 계약 검토는 외부 도움을 전제한다. 6명 구성은 제품과 UX, QA와 일부 SRE 업무를 겸임하는 안이다. 그 경우 개발 범위·일정·고객 지원 처리량을 줄여야 한다.

GA까지 C01~C21·C28의 추정 합계는 125~200 사람 주다. C22~C27 확장은 추가 48~82 사람 주이며 고객 수요에 따라 선택한다. 36주 × 8명 × 70%는 약 202 사람 주다. 상한 200 사람 주는 이 처리량에 가까워 여유가 약 2 사람 주뿐이다. 선행 작업과 외부 대기, 추정 변동까지 고려하면 36주 완료를 보장할 수 없다. 최소 범위를 선택하거나 재산정 결과에 따라 GA를 연기한다.

8명 × 52주 = 416 사람 주다. 인터뷰·휴가·지원·재작업 등을 고려해 delivery 비율을 70%로 가정하면 약 291 사람 주다. backlog 공수와 기간을 맞출 때 여기에 무제한 병렬 개발을 가정하지 않는다. 같은 담당자의 여러 작업과 외부 검토 대기 시간을 calendar에 반영한다.

## 예산 시나리오

아래는 자체 계획을 위한 비용 가정이며 현재 인건비 시장 조사나 확정 견적이 아니다. 모든 금액은 원화이며 부가세·판매 수수료·법인 운영비 포함 여부는 별도로 확정한다. 매출과 투자 수익 예측은 제시하지 않는다.

| 항목 | 8명 12개월 가정 | 계산 |
|---|---|---|
| 인력 전체 비용 | 7.20~11.52억원 | 월 750~1,200만원 × 8명 × 12개월 |
| 외부 보안·계약·전문 검토 | 0.80~1.50억원 | 복수 견적 필요 |
| lab·cloud·도구 | 0.24~0.60억원 | 월 200~500만원 × 12개월 |
| 소계 | 8.24~13.62억원 | 위 항목 합계 |
| 예비비 | 소계의 15~20% | 각 범위 하한 15%, 상한 20% 적용 |
| 총 예산 가정 | 약 9.48~16.34억원 | 8.24×1.15 ~ 13.62×1.20 |

이 수치는 8명의 실제 투입 비용을 잡은 것이다. 창업자가 급여를 받지 않는다고 인적 비용이 사라지는 것은 아니다. 현금 지출·기회비용을 분리해 기록한다. 고객 획득·출장·채널 판매·24시간 지원은 별도 사업 예산으로 계산한다.

## 한두 명으로 추진하는 축소안

1~2명이라면 자체 설치형 단일 관리 서버, Linux·SSH, 기존 인증 플랫폼 연동, 최소 tenant 위임, 승인된 서비스 작업, audit, backup·restore, signed release에 집중한다. 별도 HA·SaaS·RDP·자체 CA·plugin marketplace·AI 자동 실행은 후순위다. 기존 제품을 연동할 때는 라이선스와 고객의 운영 부담을 확인한다.

2명 기준 첫 판매 가능한 좁은 제품을 9~12개월의 가설로 두되, 동일한 보안 gate를 통과하지 못하면 연기한다. 1명은 일정만 배로 늘려 계산하지 않는다. agent 보안·침투·UX·지원 같은 전문 역할의 외부 비용과 단일 장애 인력 위험이 커진다.

2명 12개월의 예산 예시는 월 인력 비용 가정을 동일하게 적용하면 1.80~2.88억원이다. 외부 검토 0.20~0.40억원, lab·도구 0.06~0.12억원을 더하고 예비비 15%를 적용하면 약 2.37~3.91억원이다. 시장 최고 수준의 넓은 제품을 이 범위로 완성한다는 의미는 아니다.

## 상품과 가격 실험

우선 자체 설치형 Team, Enterprise, MSP 계약을 제안한다. 기본 보안과 data export는 모든 상품에서 제공한다. Enterprise에는 검증된 HA·offline·장기 보존·지원, MSP에는 다고객 계약·위임·고객별 비용과 reporting을 추가한다.

과금 단위의 우선 가설은 관리 대상 host 또는 environment 수 + 지원 수준이다. 사람 계정마다 과금하면 고객이 계정을 공유할 유인이 생기는지 인터뷰한다. Kubernetes에서 cluster·node·VM·container를 중복 과금하지 않는 규칙이 필요하다. dormant·ephemeral·test 자원과 license grace도 명세한다.

첫 가격 실험 예시는 Team 20대 기준 월 20·40·60만원의 패키지 비교다. 이 금액은 확정 판매가나 경쟁 평균이 아니라 고객 인터뷰용 가설이다. 기능·지원 범위를 고정한 뒤 구매 이유·대안·허용 가능한 도입 비용을 확인한다. 실제 유료 pilot로 검증하기 전 매출 예측에 사용하지 않는다. 경쟁 제품의 과금 단위는 [비교 문서](benchmark.md)에 공식 출처를 연결했다.

원가에는 metrics·session recording 저장, egress, support 시간, 취약점 대응, offline 설치, upgrade 지원을 포함한다. 고객별 월 반복 비용과 도입 지원 시간을 재서 gross margin을 계산하고 최저 계약 규모를 정한다.

## 계약과 지원 운영

현재 저장소 LICENSE는 내부 사용 권한과 별도 SaaS 계약 필요를 명시한다. MSP 고객의 제3자 서버 관리 권리를 분명하게 정의해야 한다. 계약 검토는 적격 전문 검토를 거쳐 확정한다. [현재 LICENSE](../../LICENSE)

지원 상품에는 지원 시간대, severity, 최초 응답과 복구 목표의 차이, 지원 OS·topology, EOL, 고객 책임, 원격 지원 동의, 비밀 취급, 취약점 통지, backup 책임, 데이터 반환·계약 종료를 명시한다. 작은 팀에서 24시간 대응을 증명하지 못하면 24시간 SLA를 판매하지 않는다.

고객이 보내는 diagnostic bundle은 기본적으로 credential·token·개인정보를 제거하고 사용자가 내용을 검토하게 한다. support 직원의 고객 접근은 별도 일시 권한과 감사로 제한한다. support라는 이유로 모든 고객의 최고관리자 권한을 공유하지 않는다.

GS·ISMS·SOC 2·CC 같은 평가·인증은 대상 시장과 요구 계약이 확인된 뒤 별도 범위·비용·증거 작업으로 결정한다. 이 계획 또는 특정 표준을 참고했다는 사실만으로 인증을 받았다고 표시하지 않는다.

## 고객 검증과 사업 판단

첫 인터뷰는 MSP 6명, 기업 또는 중소기업 운영자 6명, 보안·구매 관계자 3명을 제안한다. 현재 도구, 최근 장애·권한 실수, 고객 위임 방식, 데이터 외부 전송 금지, 설치·업그레이드 부담, 구매 승인 과정, 대안·전환 비용을 묻는다. 요구 기능 목록보다 실제 작업과 도입 장애를 기록한다.

처음 3개 design partner의 목표는 같은 핵심 문제를 찾는 것이다. 유료 pilot에서는 명시된 성공 기준·복구 책임·지원 조건·종료 후 데이터 반환을 합의한다. 고객의 운영 데이터를 연구용 외부 저장소나 AI에 자동 전송하지 않는다.

| 검토 시점 | 판단 기준의 제안 | 기준을 못 맞추면 |
|---|---|---|
| 4주 | 서로 다른 고객 3곳에서 같은 핵심 작업 문제 확인 | 포지셔닝과 GA 기능 범위 재설정 |
| 12주 | P0 기반과 복원 가능성, 실제 connector 시험 통과 | 유료 pilot 범위 축소 또는 연기 |
| 24주 | 고객 3곳의 반복 사용·유료 의사·지원 비용 증거 | 신규 기능 확대 중지, 문제·가격·지원 구조 재검토 |
| GA | 독립 보안·격리·upgrade·restore·부하 gate 통과 | 출시 연기, severity 기준에 맞는 수정과 재시험 |
| 확장 | 안정 운영·재계약·unit cost·확장 구매 수요 확인 | HA·SaaS·AI 투자 순서 조정 |

“유료 전환 3곳”은 목표이며 현재 달성치가 아니다. CAC·회수 기간·churn·매출은 실제 cohort가 생긴 뒤 계산한다.

## 위험과 대응

| 위험 | 조기 징후 | 대응 |
|---|---|---|
| 초기 고객이 서로 다른 제품을 요구 | custom 요청이 공통 작업보다 많음 | 핵심 고객·지원 topology 제한 |
| 기존 root 구조와 권한 이전의 복잡성 | shadow 인가 결과 불일치, helper가 자유 셸 허용 | action 분리·least privilege·별도 검토 |
| 고객 격리와 실자원 권한 불일치 | UI 역할과 Unix 또는 Kubernetes 권한 다름 | 실제 credential mapping과 end to end 부정 시험 |
| demo 결과가 실데이터처럼 표시 | 연결 오류 뒤 fixture·0건 목록 표시 | production fail closed·source와 freshness 표시 |
| 작업 중복·부분 실패 | 재시도 후 중복 effect, 저장 실패 성공 표시 | durable state·fencing·UnknownResult·사후 조정 |
| 녹화·관측성 비용 증가 | data retention 비용과 support 시간이 급증 | 측정·tier 한도·sampling와 audit 분리 |
| 제품 개발보다 지원이 커짐 | 개발자 on call로 일정 지연 | 지원 범위·diagnostic·runbook·채널 partner |
| AI가 위험한 실행을 유도 | 로그 지시를 따라 tool 호출, 잘못된 대상 | identity·정책·typed action·승인·attack 평가 |
| 채택한 dependency의 배포 의무 | SBOM와 LICENSE 누락 | dependency별 license 검토와 NOTICE |
| 단일 개발자에 운영 지식 집중 | 특정 인력 부재 시 배포·복구 불가 | 문서·복구 drill·2인 교차 검토 |

## 완료 판단

최고 수준은 한 번의 큰 개편으로 완성되는 상태가 아니다. 특정 고객 작업의 성공·안전·복구·사용성에서 비교 우위를 입증하고, 그 수준을 릴리스마다 유지해야 한다. 범위와 목표는 고객 증거에 따라 바뀔 수 있지만 tenant 간 기본 격리와 데이터 보존·복구의 조건은 낮추지 않는다.
