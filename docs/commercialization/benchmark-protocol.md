# 나비맨 상용화 벤치마크 실행 명세

이 문서는 실측 시험의 명세다. 2026-10-06 Webmin 2.670과 배포된 나비맨의 로컬 cron 삭제 동작을 제한된 chroot에서 처음 비교했다. 범위와 한계는 [실행 기록](execution.md)의 Webmin cron 항목을 따른다. 전체 비교 실측과 고객 사용성 시험은 미완료다. 문서 조사 결과를 실제 우월성 주장으로 바꾸려면 아래 증거를 확보해야 한다.

## 공정한 비교 조건

제품군별로 같은 고객 작업을 지원하는 제품만 비교한다. Proxmox의 VM HA와 Cockpit의 단일 host 서비스 관리, Teleport의 접속 승인과 Plesk의 호스팅 account 생성 시간을 동일 점수로 묶지 않는다.

| 작업군 | 최소 비교 후보 |
|---|---|
| Linux 서버 등록·진단·서비스 변경 | 나비맨, Cockpit, Webmin |
| 고객 소속과 관리 위임 | 나비맨, Plesk 또는 cPanel, Portainer BE, Rancher |
| 원격 접속·일시 권한·기록 | 나비맨, Teleport의 해당 edition, Guacamole |
| 반복 작업·실행 승인 | 나비맨, AAP, Rundeck의 해당 edition |
| 컨테이너·namespace 운영 | 나비맨, Portainer, Rancher |
| 장애 조사와 관측 화면 | 나비맨, Netdata, Grafana에 같은 데이터 source 연결 |

라이선스가 필요한 제품은 정식 trial 또는 구매 권한이 확보된 lab에서만 시험한다. edition·지원·가격을 기록하고, 제품이 지원하지 않는 작업은 실패 점수 대신 N/A로 표시한다. 유료 기능을 무료판에서 찾지 못했다는 이유로 제품 전체를 낮게 평가하지 않는다.

## 테스트 환경

다음은 최초 lab 구성의 제안이다. 실제 지원 matrix는 계획 첫 4주에 확정한다.

- Linux LTS 두 계열, x86_64, 최소 권한 운영 계정과 root 복구 계정.
- OpenSSH 서버, 정상 host key·변경된 host key·만료 인증정보·별도 lab의 legacy SSH 대상.
- Docker environment와 Kubernetes cluster. Kubernetes는 CNI의 정책 집행과 metrics 지원을 기록한다.
- 고객 조직 10개, 사용자 100명, 다양한 역할과 만료·정지 상태를 가진 fixture.
- 서버 10개 → 100개 → 1,000개 규모. simulated inventory와 실제 연결 host 수를 별도로 기록한다.
- RTT 1·30·100·250ms, 패킷 손실 0·1%, 연결 끊김·NAT·HTTP proxy 조건.
- 브라우저 Chrome·Firefox·Safari의 고정 버전. 모바일은 핵심 조회·승인 작업을 별도 시험한다.
- 서버 spec, CPU governor, OS·kernel, image digest, 연결 topology, TLS, 정책, 수집 주기, 캐시 상태를 저장한다.

첫 GA lab class의 제안은 control plane 4 vCPU·8GiB RAM·100GiB SSD, 별도 측정한 실행 대상 host당 2 vCPU·2GiB RAM이다. CPU 모델·실제 할당·storage latency를 기록하며 모든 제품이 이 spec에서 같은 topology로 동작한다고 가정하지 않는다. 외부 DB·관측 저장소가 필요하면 별도 자원과 비용을 포함한다. fixture는 10개 조직·100명·100대 host·host당 20개 service이며 audit 10만 건·job 이력 1만 건을 초기값으로 한다. 저장 이벤트 크기와 실제 connector 부하는 기록한다.

API 부하는 로그인 후 100개 동시 사용자 session에서 시작하고, 사용자당 요청 사이 2초 간격을 둔 closed loop로 구성한다. 요청 비율의 제안은 inventory 목록 25%, resource 상세 25%, 최근 상태 20%, job 상태 15%, audit 조회 10%, 승인된 무해한 정책 변경 5%다. 실제 도달 request rate와 오류·queue 길이를 함께 기록하며 고정 50 requests/s가 보장된다고 가정하지 않는다. 실자원 변경·로그인·SSH·connector 수집은 별도 workload로 측정한다. open loop의 단계별 도착률 증가로 saturation과 backpressure도 별도 시험한다. warm up 동안 정해진 캐시를 만들고 cold cache 결과를 따로 보고한다.

control plane, DB, 실행기, 관측성 저장소의 CPU·메모리·disk·network를 모두 포함한다. 초기 비교는 동일한 측정 class에서 수행하되 서로 다른 제품의 공식 권장 topology가 필요하면 그 차이와 전체 비용을 함께 보고한다.

## 사용자 작업 시나리오

| ID | 작업 | 성공 판정과 수집할 증거 |
|---|---|---|
| B01 | 빈 VM에서 설치 후 첫 서버 등록 | 담당자의 명령 횟수, active 작업 시간, 전체 경과 시간, 첫 정상 데이터 확인 |
| B02 | SSH 접속 오류 해결 | DNS·TCP·KEX·host key·인증·sudo 오류를 구분해 정확한 원인을 찾는지 |
| B03 | 가입 신청 승인과 소속 관리자 지정 | 승인 전 로그인 차단, 승인 후 자기 자원만 표시, 잘못 배정한 자원 회수 |
| B04 | tenant 관리자에게 회원 관리 위임 | 승인된 하위 회원 관리, 자기·동급·최고관리자 권한 상승 거부 |
| B05 | 임시 서버 접근 부여와 회수 | 실제 접속 identity·TTL·회수 속도·기존 WS·refresh·재접속 결과 |
| B06 | 서비스 재시작 요청과 승인 | 영향 대상·사유·diff·승인자 표시, 계획 변경 시 승인 무효, 사후 상태 확인 |
| B07 | 여러 서버에 package 또는 설정 변경 | canary, 진행률, 부분 실패, 승인 범위와 실제 실행 일치, 복구 경로 |
| B08 | 잘못된 설정의 복구 | backup·검증·restore·복구 확인, 되돌릴 수 없는 작업의 정확한 표시 |
| B09 | tenant quota 또는 workload 변경 | 상위 한도·namespace·network·admission의 실제 집행 확인 |
| B10 | CPU 또는 디스크 장애 조사 | 지표·로그·변경 이력을 연결해 원인 후보를 찾는 시간과 오류 |
| B11 | 특정 사용자 작업의 감사 조사 | 사용자·자원·명령 또는 action·승인·실행·결과를 찾고 export |
| B12 | 관리 서버 upgrade와 복원 | 사용자·policy·credential reference·session ownership 유지, rollback·복원 성공 |
| B13 | 모바일과 키보드로 승인·상태 확인 | focus, label, 오류 안내, overflow, 접근성 critical 문제 |
| B14 | AI의 악성 로그 처리 | 잘못된 지시를 따라 비밀 반출·타 tenant 실행을 하지 않는지. 별도 공격 시험 |

사용성 시험은 3개 역할별 최소 5명씩 15명을 제안한다. 이 표본은 초기 문제 탐색용이며 세계 시장 전체 사용자의 통계적 대표 표본이 아니다. 제품 순서를 교차 배치해 학습 효과를 줄이고 익숙한 제품·경험을 기록한다. 성공률, 오류 수, 작업 시간의 median과 p90, 사용자의 설명을 남긴다.

## 성능과 규모 측정

아래 숫자는 출시 목표 초안이다. 현재 달성치가 아니다. 4주차에 lab baseline을 확보한 뒤 목표를 조정하되 완화 사유를 기록한다.

| 지표 | 제안 목표 | 측정 경계 |
|---|---|---|
| 설치와 첫 정상 데이터 | 지원 lab의 신규 운영자 median 15분 이하 | 다운로드 포함 전체 시간과 실제 작업 시간을 각각 기록 |
| tenant 격리 | 정의한 모든 부정 시험에서 유출·권한 상승 0건 | API·UI·stream·export·audit·job·backup·credential 경로 |
| 관리 목록 API | GA class 100 host·100 동시 사용자에서 p95 300ms 이하, p99 1초 이하 | 인증·인가 포함. 네트워크 왕복과 외부 connector 시간은 별도 표시 |
| 권한 회수 | 연결된 시스템 p99 5초 이내 | active session·새 요청·queue·retry 포함. 단절 agent는 신규 실행 거부 |
| 서버 상태 신선도 | 정상 수집 조건 p95 10초 이내 | 수집 주기와 last successful sample 기준 |
| 터미널 접속 | RTT 30ms 정상 credential 조건에서 p95 3초 이내 | 클릭부터 첫 prompt. MFA·사람 승인 시간은 별도 |
| 관리 시스템 RPO와 RTO | GA RPO 24시간·RTO 2시간, HA class RPO 5분·RTO 30분 이내 | 신규 환경으로 실제 복원. 고객 workload의 RPO와 구분 |
| 세션 동시성 | GA class 실제 SSH 세션 50개 1시간, Enterprise class 200개 별도 검증 | 출력·resize·재접속·권한 회수 부하 포함 |
| 서비스 SLO | 관리 서비스 GA 관측 목표 99.9%, 검증된 HA class 99.95% | 자체 설치형의 전체 가동 시간을 공급자가 무조건 보장하는 SLA가 아님 |
| 핵심 작업 사용성 | 첫 시도 성공률 90% 이상, 비교 대상 대비 median 시간 30% 개선 가설 | 작업별 결과와 표본·분산·학습 효과 제시 |
| AI | 읽기·진단 품질과 공격 성공률을 별도 평가 | 유한 시험에서 공격 0건이 보편적 안전의 증명은 아님 |

규모 class마다 30분 warm up, 60분 sustained load, 24시간 soak test를 제안한다. 동일 조건의 최소 5회 반복에서 error rate, p50·p95·p99, throughput, 자원 사용량, 데이터 신선도, reconnect와 작업 성공률을 수집한다. 바뀐 workload·TLS·인가 조건으로 얻은 수치를 같은 benchmark처럼 비교하지 않는다.

실제 1,000 host 구매 또는 운영 환경을 확보하지 못하면 simulated inventory와 제한된 실제 host 결과를 명확하게 분리한다. 그 결과만으로 “1,000대 운영 검증”을 주장하지 않는다.

## 장애와 보안 시험

| 장애 또는 공격 | 기대 동작 |
|---|---|
| kubeconfig 만료·connector credential 오류 | Expired 또는 Unauthorized 표시, fixture 자동 반환 금지 |
| DB 파일 접근 실패·disk full | 성공 응답 금지, 상태 손실·in memory 대체 금지, 운영자 경보 |
| job 저장 실패 | 작업을 수락했다는 성공 표시와 실제 실행 금지 |
| worker 종료·lease 만료·stale worker 복귀 | 복구 가능한 상태, 중복 변경 차단 또는 명시된 사후 조정 |
| 실행 성공 후 응답 손실 | UnknownResult와 사후 확인, 무조건 재실행 금지 |
| SSH host key 교체·MITM | 기본 차단, 승인된 회전만 수락 |
| 정책 변경 중 WS·SSE·재시도 | 기존 연결과 job도 새 정책에 따라 차단 |
| tenant ID 교체·검색·export | 다른 조직의 존재·내용·비밀·실행 결과 유출 금지 |
| 악성 symlink·경로 traversal·대상 바꾸기 | 실제 filesystem object와 허용 범위 검증 |
| 승인 후 인자·대상 변경 | plan hash 불일치로 실행 거부 |
| backup key 누락·다른 tenant restore | 복구 실패 이유 표시, 잘못된 조직에 복원 거부 |
| 감사 저장소 장애 | 보존 정책에 맞는 경고·고위험 변경 거부·손실 없는 복구 |
| 라이선스 만료·시계 변조·offline | 정의된 제한과 유예, 서비스·고객 데이터 임의 파괴 금지 |
| 악성 AI 도구 결과 | 정책 밖 실행·데이터 반출 거부, 감사 기록 |

이 시험은 오류 처리·입력 순서 시험의 중요성을 다룬 연구를 적용한 제안이다. [OSDI 2014](https://www.usenix.org/conference/osdi14/technical-sessions/presentation/yuan)

## 결과 형식과 판정

벤치마크 결과에는 `benchmark_id`, `product`, `edition`, `version`, `image_digest`, `test_commit`, `hardware`, `topology`, `dataset`, `policy`, `scenario`, `run_id`, `metric`, `value`, `unit`, `error_count`, `evidence_uri`, `measured_at`를 기록한다. 초안 CSV는 [측정 결과 양식](benchmark-results-template.csv)에 제공한다. 양식은 그대로 보존한다. 최초 cron 결과 18행과 원본은 `/workspace/nabiman/benchmarks/webmin-cron-20261006-v1/results.csv`에 별도로 저장했다.

성능·사용성의 비교 점수는 보안 gate를 통과한 제품 조합에만 계산한다. 가중치는 고객 인터뷰에서 정한다. 초기 제안은 안전한 위임 25%, 작업·복구 25%, 사용성 20%, 배포·지원 15%, 성능·관측성 15%다. N/A는 분모에서 제외하며 제외 범위를 함께 표시한다. 문서상 기능 존재 점수와 실제 성공 점수를 분리한다.

GA는 독립 검토에서 critical·high 미해결 위험이 없고, tenant 부정 시험·upgrade·restore·실제 connector 시험을 통과한 뒤 판정한다. 고객이 승인한 demo나 mock 환경에서 성공한 것만으로 production connector 시험을 통과했다고 처리하지 않는다.

비교 결과는 제품·버전·대상 고객·시험 조건을 붙여 공개한다. “세계 1위” 같은 일반 순위는 독립적이고 대표성 있는 자료가 확보되기 전 사용하지 않는다.
