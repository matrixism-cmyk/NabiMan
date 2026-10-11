# 나비맨 상용화 실행 기록

세계 최고 수준의 상용 제품 완성 목표는 진행 중이다. 조사·설계·개별 구현·운영 배포·고객 검증을 구분한다. 전체 완료 판단은 [종합 계획](README.md), [구조](architecture.md), [개발 항목](roadmap.md), [검증 명세](benchmark-protocol.md)의 요구 사항과 실제 증거를 함께 대조한다.

## 2026년 10월 5일 운영 데이터 신뢰성 개선

대상은 C02의 backend 실패 처리와 저장 기반이다. 이 단계는 상용화 전체 완료 또는 C02의 모든 화면·배포 완료를 의미하지 않는다.

| 변경 | 현재 결과 | 근거 |
|---|---|---|
| connector fallback | mock은 명시적 mock 모드에서만 사용. real·auto에서 초기화 실패 시 unavailable connector 사용 | [bootstrap](../../backend/src/mec/services/bootstrap.rs), [실패 connector](../../backend/src/mec/services/unavailable/mod.rs) |
| 일시적 Kubernetes 오류 | 구성된 실제 client를 유지하고 호출마다 실제 오류 반환. 인증·연결 복구 후 재시작 없이 정상 데이터 조회 가능 | [실패와 복구 시험](../../backend/src/mec/services/bootstrap_tests.rs) |
| 잘못된 mode | 허용 값 real·auto·mock 이외에는 시작 실패. 기본 auto는 demo를 허용하지 않음 | [mode parsing](../../backend/src/mec/services/bootstrap.rs) |
| DB 실패 | 파일 DB를 열지 못하면 서버 시작 중단. 빈 경로와 :memory: 지정 거부. auth 초기화와 background collector 시작 전에 검사 | [startup](../../backend/src/main.rs), [영구 저장과 재시작 시험](../../backend/src/mec/db/mod.rs) |
| 비밀 노출 제한 | 초기화 오류의 원문 대신 고정된 source와 복구 안내 반환. 테스트 process에는 운영 connector credential을 전달하지 않음 | [bootstrap](../../backend/src/mec/services/bootstrap.rs), [격리 smoke](../../scripts/smoke-commercial-integrity.py) |

검증 명령은 backend 디렉터리의 `cargo test --locked --no-fail-fast`, `cargo build --locked`, 저장소 루트의 `python3 scripts/smoke-commercial-integrity.py`다. 전체 backend 시험은 198 unit + 5 auth flow + 34 integration으로 총 237건 통과했다. 컴파일 경고는 발생하지 않았다. HTTP 503과 오류 envelope, disconnected health, demo 명시 모드, 실제 Kubernetes 검증용 API의 401 뒤 회복, 영구 DB 재열기·데이터 보존을 포함한다. 이는 외부 실제 cluster 부하·고객 침투 시험을 대신하지 않는다.

실행 바이너리의 격리 smoke는 real·auto 모두에서 잘못된 kubeconfig와 누락된 provider 설정을 사용했다. inventory 조회·node cordon·Harbor project 생성이 503으로 거부되고 fixture가 반환되지 않았다. 오류 DB·memory DB·잘못된 mode는 auth 저장소 생성 전에 시작을 중단했다. 기존 auth 데이터·실제 서버·운영 서비스는 변경하지 않았다.

`python3 scripts/smoke-membership.py`도 통과했다. 브라우저 가입·최고관리자 승인·로그아웃 terminal 격리·소속 관리자 화면과 하위 회원 관리, scope 정책·다른 tenant 차단·인원 한도·명령 제한·접근 만료·session 회수·지속 저장을 재검증했다.

운영 배포는 아직 수행하지 않았다. 배포 전에 exact release binary, 실제 데이터 backup·복원 가능성, 현재 connector 설정과 demo 의존 화면, 기존 session 보존, 재시작·rollback을 확인해야 한다. Kubernetes 설정 파일 자체가 누락되거나 잘못된 경우 수정 후 재시작이 필요하며, hot reload는 이번 구현에 포함하지 않았다.

## 2026년 10월 5일 AXGate 서버 신원 검증

C03의 AXGate connector부터 서버 공개키 SHA256 지문 검증을 구현했다. 설정 누락·잘못된 지문은 socket을 열기 전에 차단하고, handshake 키 불일치는 비밀번호 인증 전에 거부한다. 조회·health·VPN·NAT 변경 경로가 같은 접속 함수와 정책을 사용한다. 지문 불일치 오류에는 관측한 공개 지문과 독립 확인 후 교체 안내를 제공한다.

구형 SHA1 KEX와 ssh-rsa는 기본 추가에서 제거했다. 단일 AXGate host에만 만료가 최대 30일 이내인 RFC3339 예외를 적용할 수 있다. 잘못된 시각·만료된 예외를 거부하고 캐시 조회 전에도 유효성을 검사한다. 기본은 russh 기본 알고리즘이며 SSH host certificate·다중 key pin·전체 알고리즘 정책을 구현했다는 뜻은 아니다.

전체 `cargo test --locked --no-fail-fast`는 205 unit + 5 auth flow + 34 integration으로 총 244건 통과했고 컴파일 경고가 없었다. 추가 7건은 canonical 지문·만료·최대 기간, 정상 키 인증, 변경된 키에서 비밀번호 미전송, 설정 누락 시 socket 연결 없음, 구형 서버 기본 차단, 유효한 예외의 구형 서버 접속과 만료된 예외의 cache 반환 차단을 다룬다. 정상·변경 키와 알고리즘 협상은 실제 로컬 russh SSH 서버와 handshake를 사용했다. production AXGate의 접속·CLI 동작을 시험한 것은 아니다.

변경 후 `cargo build --locked`와 `python3 scripts/smoke-commercial-integrity.py`도 통과해 real·auto의 오류 응답 및 DB 시작 실패 처리를 재검증했다.

설정과 최초 등록·회전 절차는 [AXGate 서버 키 운영 문서](ssh-host-trust.md)에 기록했다. 키는 콘솔 또는 이미 검증된 관리 경로에서 독립 확인해야 하며 자동 keyscan 결과를 곧바로 신뢰하지 않는다. 현재는 관리자 배포 설정과 재시작으로 적용하며 통합 UI·승인 event·hot reload·live session 회수는 후속 범위다.

운영 배포는 아직 하지 않았다. 이 AXGate 단계 당시 일반 remote probe·terminal·SSH key 배포는 OpenSSH accept-new를 유지했다. 이후 일반 서버 신뢰 관리 구현은 아래 실행 기록을 참조한다. 따라서 C03 전체를 완료 처리하지 않는다. 배포 전 실제 장비의 승인된 지문과 필요 시 제한된 legacy 예외를 준비해야 한다.

## 2026년 10월 5일 일반 원격 서버 신뢰 관리

일반 서버 모델에 검증한 host public key·SHA256 fingerprint·확인자·UTC 시각·확인 사유를 추가했다. 기존 레코드는 미확인으로 읽는다. 최고관리자와 해당 서버의 소속 관리자만 PUT host-trust로 등록·교체·해제할 수 있고, 공개키와 독립 확인 지문이 일치해야 한다. 이전 지문과 확인 대상 host·port가 현재 값과 다르면 409로 거부해 변경 덮어쓰기와 대상 교체 경쟁을 방지한다.

관리 서버의 공개키 version별 0600 known_hosts 파일과 고유 HostKeyAlias를 사용한다. remote probe·exec·saved terminal·SSH key 배포·키 인증 시험에서 StrictHostKeyChecking=yes를 적용하고 기존 accept-new를 제거했다. 직접 host를 지정한 terminal도 등록된 일치 대상의 신뢰를 요구한다. host·port 변경과 신뢰 교체·해제는 이전 온라인 상태를 초기화하고 진행 중인 이전 probe 결과가 새 trust 상태를 덮지 않도록 비교한다.

원격 설정에 등록·교체·해제 화면을 추가하고 소속 관리자도 내 소속 서버에서 같은 기능을 사용한다. 신규 서버 저장 후 설정 화면으로 이동하며 신뢰를 확인하기 전에 비밀번호 검증 결과로 잘못 안내하지 않는다. 새 UI는 한국어·영어·일본어 label과 연결된 입력 label을 제공한다.

전체 backend 시험 209 unit + 5 auth flow + 34 integration, 총 248건 통과. 추가 4건은 잘못된 공개키·형식 차단, 신뢰 파일 권한·version·미확인 거부, 실제 OpenSSH의 비표준 port 정상 키 접속, 변경된 키의 인증 전 차단이다. frontend 대상 3 suites 15건, typecheck·build 통과. lint는 0 errors·기존 7 warnings다. backend build도 통과했다.

확장한 `scripts/smoke-membership.py`는 API 권한·교차 서버 차단·지문 불일치·이전 지문 및 대상 불일치·host 변경 신뢰 해제와 기존 가입·승인·소속 정책·회수 흐름을 검증했다. 실제 Chromium에서 소속 관리자가 key를 등록하고 reload 후 fingerprint를 확인한 뒤 해제하는 흐름도 통과했다. 화면은 `screenshots/host-trust-scope-admin.png`로 저장해 왼쪽 label·입력·설정 context를 확인했다.

아직 운영 배포 전이며 기존 서버의 key 확인·등록 migration이 필요하다. 기존 세션·진행 중 요청의 즉시 회수, append only key 교체 감사, 전체 legacy 예외 UI, version 파일 정리는 후속 작업이다. 수동 확인 기록만으로 실제 확인 행위의 진위를 시스템이 증명하거나 2인 승인 workflow를 완료했다고 주장하지 않는다. C03은 진행 중이다.

## 2026년 10월 5일 검증된 실행자와 작업 저장 경계

C10·C11의 기반을 구현했다. API middleware가 JWT 서명·session·활성 계정 확인 후 내부 VerifiedPrincipal을 설정한다. 보호된 handler는 이 context의 사용자·현재 계정 role을 사용하고 X-User 또는 검증되지 않은 JWT decode로 실행자를 만들지 않는다. MEC 변경·조회 감사, 회원 관리, terminal 설정·관리·접속, TOTP 설정, 감사 삭제 권한을 같은 인증 결과에 연결했다. public 인증 경로에는 incoming token만으로 principal을 설정하지 않는다.

감사 middleware를 인증 경계 바깥에 두고 response의 검증된 actor를 읽도록 변경했다. 인증 실패는 unauthenticated로, 활성 계정의 권한 거부는 검증된 사용자로 기록한다. 정상적인 고빈도 polling은 기존 skip을 유지하되 오류는 기록한다. 이 개선은 감사 저장 실패 처리·독립 보존·auth 성공 event의 세부 subject 모델까지 완료했다는 뜻은 아니다.

JobRunner의 create·start·단계·complete·fail은 저장 오류를 반환한다. 생성은 전체 UUID와 INSERT를 사용해 ID 충돌 시 기존 작업을 덮지 않는다. 존재하지 않는 job의 update를 성공으로 처리하지 않는다. 시작은 Pending에서만 허용하고 단계 저장은 Running에서만 허용한다. 단계 변경은 복사본을 DB에 저장한 뒤 memory와 event에 반영한다. unknown step과 미완료 단계가 있는 job의 완료도 거부한다.

완료·실패의 status·result·error·completed_at은 단일 UPDATE로 저장하고 terminal job을 뒤늦게 재시작·덮어쓰지 않는다. 저장 후에만 진행·종료 event를 발행한다. tenant orchestrator가 모든 checkpoint 오류를 전달하고 실패 이후 다음 인프라 호출을 진행하지 않는다. tenant upsert 오류도 무시하지 않는다. 삭제 중 Rancher·Kubernetes 오류를 성공 처리하지 않고 local tenant를 Failed 상태로 유지해 정합성 확인이 필요함을 표시한다.

전체 backend 시험 216 unit + 5 auth flow + 34 integration, 총 255건 통과했고 컴파일 경고는 없었다. 추가 7건은 서명된 identity와 실제 계정 role·위조 actor header·public request context, 읽기 전용 DB의 create 거부, checkpoint memory·DB·event 불변, 종료 UPDATE의 atomic 실패·재시작 거부, 중복 ID 보존, job 저장 실패의 namespace 생성 차단, 실제 upstream 삭제 실패의 tenant 및 failed job 보존을 다룬다. 기존 두 handler 보안 시험도 실제 AuthCheck와 유효 session을 거치도록 보강했다.

최신 `cargo build --locked`와 두 격리 smoke도 통과했다. `scripts/smoke-membership.py`는 위조 X-User에도 실제 회원이 감사 실행자로 남는 것과 잘못된 token의 401이 unauthenticated로 기록되는 것을 HTTP로 확인했다. 가입·최고관리자 승인·소속 관리자 화면·회원 정책·session 회수와 Chromium의 서버 key 등록·재조회·해제 흐름도 통과했다. `scripts/smoke-commercial-integrity.py`는 real·auto의 demo fallback 차단과 잘못된 mode·영구 DB의 시작 실패를 재검증했다. 실제 운영 서비스나 장비는 변경하지 않았다.

운영 배포는 아직 하지 않았다. 관리 DB와 job 상태는 남기지만 durable queue·worker lease·fencing·재시작 복구·실행 직전 정책 재인가·승인 plan·UnknownResult·partial recovery·tenant와 job의 최종 변경을 묶는 transaction은 후속 범위다. local audit의 저장 오류·외부 전송 실패·독립 검증도 남아 있다. C10·C11 전체 완료를 선언하지 않는다.

## 2026년 10월 5일 테넌트와 작업 완료의 원자적 저장

C10의 최종 DB 경계를 추가했다. 테넌트 생성의 profile 저장과 삭제의 profile 제거를 각각 job 완료 status·result·error·completed_at과 같은 SQLite IMMEDIATE transaction으로 묶는다. 저장된 job의 종류와 대상 tenant ID를 비교하고 Running 상태 및 모든 단계의 Completed 상태를 transaction 안에서 확인한다. 완료 event와 channel 제거는 commit 이후에만 수행한다. 생성 기록의 실행자는 저장된 job의 created_by를 사용한다.

일반 job 완료도 검증과 종료 UPDATE를 같은 transaction에서 수행한다. 저장된 kind·status·steps·result JSON과 시작·종료 시각이 손상된 경우 오류를 반환한다. 손상된 steps를 빈 목록으로, 잘못된 시각을 현재 시각으로 대체하던 처리를 제거했다. 조회 오류를 숨기지 않으므로 손상된 job이 있는 목록은 현재 오류를 반환한다. 개별 손상 레코드 격리·관리자 복구 UI는 후속 범위다.

전체 backend `cargo test --locked --no-fail-fast`는 222 unit + 5 auth flow + 34 integration, 총 261건 통과했다. 추가 6건은 생성·삭제의 job 완료 UPDATE 실패 시 전체 rollback·event 미발행, tenant INSERT 실패 시 job 미완료, 대상 및 종류 불일치 거부, 여섯 종류의 손상 필드 차단, 실제 파일 DB 재열기 뒤 commit·rollback 상태 보존을 다룬다. 이후 production에서 쓰지 않는 단독 tenant 삭제 helper를 test 전용으로 제한했고 최종 `cargo build --locked`는 경고 없이 통과했다.

새 `python3 scripts/smoke-tenant-transactions.py`도 통과했다. 격리 mock 서버에 HTTP로 요청하면서 별도 DB connection의 trigger로 완료 저장만 실패시켰다. 실패한 생성은 Active profile을 남기지 않고 failed job을 저장했다. 실패한 삭제는 resource ID와 Failed profile을 유지하고 failed job을 저장했다. 정상 생성·삭제의 201·204와 저장 상태도 확인했다. 기존 `scripts/smoke-commercial-integrity.py`의 real·auto 및 영구 DB 시작 경계도 재검증했다.

이 transaction은 local 관리 DB의 최종 변경에 적용된다. 외부 Kubernetes·Rancher 작업을 되돌리는 distributed transaction은 아니다. 생성 중 만들어진 외부 자원은 완료 저장 실패 후에도 남을 수 있으며, 삭제 후 local Failed profile이 남아도 실제 자원이 복구된 것은 아니다. 진행 중 생성의 자원 ID checkpoint·재시작 복구·실행 결과 불명확 상태·재조정·worker lease·승인은 계속 필요하다. 이번 변경은 운영에 배포하지 않았다.

## 2026년 10월 5일 생성 중 자원 추적과 테넌트 ID 예약

외부 namespace 생성 전에 Provisioning profile을 저장한다. SQLite IMMEDIATE transaction에서 기존 profile 유무를 확인하고 ID를 예약하므로, 기존 Active·Failed·Provisioning profile을 새 생성 요청이 덮어쓰지 않는다. namespace 사전 확인에서 이미 존재하는 대상을 발견하면 예약 없이 충돌을 반환한다. profile 예약이 실패하면 외부 자원 생성으로 진행하지 않는다.

Rancher project·user 생성 응답에서 받은 ID는 다음 단계 시작 전에 profile에 저장한다. 저장 실패 후 다음 인프라 호출을 진행하지 않는다. 실패한 생성은 known resource ID를 유지한 Failed profile과 failed job을 남긴다. profile 저장 자체가 실패한 경우에도 job 저장이 가능한 한 failed job result의 tenant snapshot에 확인된 ID를 보존한다. 생성 중·삭제 중 profile의 삭제 요청은 새 job을 만들기 전에 409로 거부한다. 정상 생성의 Active 전환은 이전 단계에서 구현한 tenant/job 최종 transaction을 사용한다.

전체 backend `cargo test --locked --no-fail-fast`는 228 unit + 5 auth flow + 34 integration, 총 267건 통과했다. 추가 6건은 project 이후·user 이후 checkpoint 실패의 known ID 보존, profile 예약 실패의 namespace 생성 차단, namespace가 사라진 기존 Failed profile 보존, Provisioning·Terminating 삭제 차단, 두 독립 DB connection의 동시 예약에서 단일 소유자 보존, profile checkpoint 저장 실패의 job snapshot 보존을 검증했다.

최종 `cargo build --locked`는 경고 없이 통과했다. 확장한 `scripts/smoke-tenant-transactions.py`는 HTTP 완료 저장 실패의 Failed profile·resource ID 보존, 정상 생성·삭제, 실제 server process 종료·재시작 뒤 인증된 tenant API로 두 실패 profile의 ID 재조회까지 검증했다. mock 인프라의 외부 자원 자체가 재시작 후 보존된다고 주장하는 시험은 아니다. `scripts/smoke-commercial-integrity.py`도 통과해 real·auto의 demo fallback 차단과 잘못된 영구 DB 시작 실패를 재검증했다.

외부 호출의 응답이 유실되거나 응답과 ID 저장 사이에 process가 중단되면 자원 존재가 불명확할 수 있다. 확인된 ID를 보존하는 이번 구현이 exactly once 실행·자동 재시도·분산 rollback·lease·fencing을 보장하지 않는다. 시작 시 orphan job 판별과 UnknownResult, 독립 외부 자원 확인·승인된 재조정, 실패한 profile의 안전한 삭제 및 재시도는 후속 범위다. 실제 생성 실패는 이제 profile이 전혀 없는 상태 대신 Failed profile을 남길 수 있으므로 관리 화면에서 이를 구분해야 한다. 운영 배포는 아직 하지 않았다.

## 2026년 10월 5일 재시작 시 중단 작업 판별과 이력 표시

서버 시작 시 auth 초기화·background collector·connector 연결보다 먼저 MEC DB 파일에 nonblocking exclusive advisory flock을 획득하고 process 수명 동안 유지한다. 같은 파일의 hard link를 포함한 다른 연결의 서버 시작은 소유권 오류로 중단한다. DB 자체의 SQLite transaction lock과 서버 process 생존 판별을 구분하기 위한 장치이며 HA worker lease·fencing을 대신하지 않는다. 기존 Rust 1.88 Docker 빌드와 호환되는 현재 nix dependency를 사용했다.

독점 소유권 확보 뒤 Pending·Running job을 단일 DB transaction에서 UnknownResult(`unknown_result`)로 변경한다. 감지 시각과 외부 자원 확인 안내를 status에 저장하고 steps·result·기존 완료 시각은 보존한다. Provisioning·Terminating profile은 같은 transaction에서 Failed로 변경하되 확인된 resource ID를 보존한다. 이미 완료·실패한 job은 변경하지 않으며 반복 시작이 새로운 재실행을 만들지 않는다. UnknownResult는 자동 실행 경로에 다시 들어가지 않는다. recovery 저장 오류나 손상된 active checkpoint는 auth 초기화 전에 서버 시작을 중단한다.

job SSE는 저장소 오류를 404로 숨기지 않는다. subscribe 뒤 snapshot을 읽고 저장된 status·steps·error를 전달한다. terminal snapshot은 닫힌 이력으로 응답한다. frontend는 완료·실패·결과 확인 필요 snapshot에서 단계와 상태를 복원하고 stream을 종료한다. 실패한 단계를 완료 진행률로 세지 않으며, 중단 당시 running 단계의 표시를 물음표로 바꾼다. job stream 인증은 Authorization header만 사용하고 URL에 JWT를 추가하지 않는다.

화면에 한국어·영어·일본어의 결과 확인 안내를 추가했다. 실시간 기록과 이력에서 같은 snapshot을 사용하며 job 변경 시 component를 다시 구성한다. 실제 어두운 테마 screenshot을 확인한 뒤 이력 card의 theme 색과 긴 오류·ID 줄바꿈도 보강했다.

전체 backend `cargo test --locked --no-fail-fast`는 232 unit + 5 auth flow + 34 integration, 총 271건 통과했다. 추가 4건은 interrupted job의 evidence 보존·terminal 변경 거부·반복 recovery, profile 저장 실패의 전체 recovery rollback, 손상 active job의 부분 변경 차단, 독립 연결 및 hard link 소유권 차단·guard 해제·신규 DB 0600을 검증했다. frontend history 시험 3건·typecheck·build 통과. lint는 0 errors·기존 7 warnings다. backend build는 경고 없이 통과했다.

확장한 `scripts/smoke-tenant-transactions.py`는 실행 중인 서버와 같은 DB로 두 번째 process를 시작해 auth 초기화 전 소유권 차단을 확인했다. Pending·Running 기록과 생성·삭제 중 profile을 넣고 실제 SIGKILL로 종료한 뒤 재시작하여 인증 API·SSE에서 UnknownResult와 resource ID·step 보존을 확인했다. 실제 Chromium의 작업 기록에서도 확인 필요 상태·저장된 단계·한국어 복구 안내·URL token 없음이 검증됐다. fixture는 격리 mock 인프라이므로 실 장비 응답 유실·외부 자원 복구나 실제 실행 중 단계의 fault injection을 완료했다고 주장하지 않는다.

`scripts/smoke-commercial-integrity.py`는 real·auto 및 영구 DB 시작 경계와 손상 active checkpoint의 auth 초기화 전 시작 실패를 검증했다. `scripts/smoke-membership.py`도 가입·승인·소속 정책·host trust·브라우저·session 회수 흐름을 통과했다.

운영 배포는 아직 하지 않았다. 이 소유권 방식은 협력하는 최신 서버와 지원되는 로컬 Linux filesystem을 전제로 하며, 이전 lock 미지원 서버를 먼저 종료해야 한다. live DB 파일을 교체·삭제하거나 network filesystem·다중 host 공유 DB로 운영하는 방식의 소유권을 보장하지 않는다. Pending을 포함한 중단 작업은 외부 자원 확인 전 재실행하지 않는다. 관리자 재조정·승인된 복구 실행·독립 감사·durable queue·worker lease·fencing·실행 직전 재인가와 규모 검증은 계속 필요하다. C10 전체 완료를 선언하지 않는다.

## 2026년 10월 5일 복구 전 외부 자원 확인

최고관리자 전용 `GET /api/mec/v1/jobs/{id}/recovery`와 MEC Jobs의 ‘자원 확인’을 추가했다. 실패·UnknownResult인 테넌트 생성·삭제 job만 대상으로 삼고 현재 profile 및 job snapshot의 namespace·project·user ID를 함께 보여준다. 두 기록의 ID가 충돌하면 표시하고, 가능한 경우 해당 job snapshot의 ID를 우선 조회한다. snapshot의 tenant ID 불일치와 손상된 기록은 조회 전에 거부한다. 복구 조회는 verified actor로 감사에 기록한다.

Kubernetes namespace 목록과 Rancher project·user 조회 결과를 present·absent·unknown·not_recorded·candidates로 구분한다. provider 조회 실패는 자원 부재로 바꾸지 않는다. ID 미기록 시 기존 생성 규칙에 따른 이름의 후보만 보여주고 후보를 소유 자원으로 자동 채택하지 않는다. namespace label·project name/cluster·username 비교는 identity hint이며 소유권 증명이 아니다. 등록 ID에 path/query 문자가 있으면 조회하지 않는다. 세 종류 조회는 병렬로 수행하고 각각 10초 상한을 적용한다. provider error 원문 대신 분류 code만 보고서에 포함한다.

화면에는 확인 시각·대상 ID·조회 상태·identity 비교·저장 기록 비교와 데모 표시를 추가했다. 실패한 새로고침에서는 이전 확인 결과를 숨기고, 요청 교체·화면 이탈 때 이전 요청을 취소한다. 다른 회원의 임의 role claim·actor header로 최고관리자 검사를 통과할 수 없음을 기존 verified principal 시험에 추가했다. 일반 회원의 화면에는 자원 확인 버튼을 제공하지 않는다.

기존 tenant 삭제 preflight도 namespace·pod 수·LB service·Rancher 조회 오류를 빈 목록·0·false로 숨기던 처리를 제거했다. 확인 실패 시 삭제 전 확인 결과를 발행하지 않고 해당 service error를 반환한다. Rancher의 명시적 NotFound만 project 부재로 처리한다.

전체 backend `cargo test --locked --no-fail-fast`는 238 unit + 5 auth flow + 34 integration, 총 277건 통과했다. 추가 6건은 정상 자원 identity 관찰 및 job/profile 불변, unavailable 관찰과 preflight 503, ID 미기록 후보의 소유권 미확정, 변경된 profile과 잘못된 identity 표시, 조회 timeout·path ID 차단·upstream secret 미포함, 기록 ID의 부재와 미기록 구분 및 완료 job 거부를 검증했다. 관련 frontend 2 suites 6건은 기존 history snapshot과 확인 실패·후보·demo·기록 충돌·새로고침 실패 후 오래된 증빙 제거를 검증했다. typecheck·build 통과, lint 0 errors·기존 7 warnings, backend build 경고 없음.

`scripts/smoke-tenant-transactions.py`는 강제 종료·재시작한 job의 recovery API 및 실제 Chromium 자원 확인 화면을 검증했다. mock resource가 재시작 뒤 사라진 경우 세 관찰이 absent이고 demo·수동 확인 요구가 표시되며 자동 재실행은 false였다. screenshot `screenshots/job-recovery.png`를 확인했다. `scripts/smoke-commercial-integrity.py`는 real·auto의 설정 누락으로 실패한 tenant 생성 job에서 세 관찰 모두 UNAVAILABLE/unknown으로 반환되어 absent와 구분됨을 검증했다. 운영 서비스나 실 장비는 변경하지 않았다.

아직 복구 실행·승인 기능은 아니다. 이 보고서는 저장된 승인 artifact가 아니며 외부 자원의 point in time snapshot·소유권·전체 quota/RBAC/network policy 정합성을 증명하지 않는다. 현재 생성 코드와 같은 c-local project 규칙을 사용하고 여러 cluster 선택은 후속 범위다. 승인 대상 plan의 영구 저장·digest·변경 비교·2인 승인·실행 직전 재확인·최소 권한·복구 실행 및 partial 결과 처리·독립 감사·실 장비 시험이 필요하다. 운영 배포 전이다.

## 2026년 10월 5일 고정 초기 비밀번호 제거와 접속 정보 전달

테넌트 자동 생성과 member create-if-missing의 고정 초기 비밀번호를 제거했다. 서버는 OS CSPRNG에서 48 byte를 받아 64문자 alphabet의 48자리 비밀번호를 생성하고, entropy 오류면 외부 생성으로 진행하지 않는다. 자동 생성의 초기 사용자 이름·비밀번호는 성공한 생성 요청에만 반환한다. Tenant 모델·profile·job spec/result·audit input에는 secret을 포함하지 않으며 생성 실패 snapshot도 public tenant 정보만 보존한다. 비밀번호를 반환하는 tenant/member 생성 응답에는 Cache-Control: no-store를 적용했다.

member 요청에서 비밀번호를 생략하면 새 사용자에 한해 생성하고, 지정하면 최소 15문자·최대 UTF-8 128 byte·control character 없음·일부 예측 가능한 문자열 차단 규칙을 적용한다. 기존 사용자에 member binding만 추가할 때에는 비밀번호를 바꾸거나 반환하지 않는다. direct Rancher 사용자 생성·reset에도 같은 검증을 적용한다. 사용자 목록 조회 실패를 사용자 부재로 처리하고 새 계정을 만들던 동작을 중단했다. 기본 project role은 project-member로 낮추고 project-owner·project-member·read-only 이외의 입력은 거부한다. 기존 role·password를 일괄 변경한 것은 아니다.

실제 Rancher create 요청에 mustChangePassword=true를 추가했다. 이 필드의 의미는 [Rancher user API 모델](https://github.com/rancher/rancher/blob/main/pkg/apis/management.cattle.io/v3/authn_types.go)에 근거한다. HTTP wire 시험은 이 flag와 생성 password를 확인했지만 실제 운영 Rancher 버전의 첫 로그인 및 강제 변경 UI를 검증한 것은 아니다. reset action 뒤 기존 사용자의 flag 재설정은 후속 범위다. credential create/reset의 provider 오류는 응답 body·URL 등 raw 메시지 대신 고정된 error class 안내로 바꿔 provider가 secret을 echo해도 job·audit·HTTP 오류에 남기지 않는다.

테넌트 wizard·member 추가·MEC 사용자 생성·reset에 임시 접속 정보 화면을 연결했다. 비밀번호는 기본 masked이고 사용자가 명시적으로 보기·보관 확인을 선택한다. 보관 확인 뒤 state와 UI에서 제거하며 application browser storage에 저장하지 않는다. 브라우저 생성기는 Math.random에서 crypto.getRandomValues로 변경했다. reset HTTP 오류에도 성공 toast와 새 비밀번호를 표시하던 기존 오류를 고쳐, 성공한 응답만 접속 정보 화면으로 연결한다. 이 기능이 browser extension·사용자 clipboard·메모리 dump까지 통제하거나 secret memory zeroization을 구현했다는 뜻은 아니다.

전체 backend `cargo test --locked --no-fail-fast`는 243 unit + 5 auth flow + 34 integration, 총 282건 통과했다. 추가 5건은 generated secret uniqueness·규칙, weak/unbounded input, provider echo 제거, 실제 로컬 HTTP의 Rancher flag 및 echo 거부 응답, 생성 secret 반환과 tenant/job/audit record 미포함을 검증했다. frontend 관련 2 suites 3건은 masked receipt·명시적 reveal·확인 뒤 제거, Web Crypto와 insecure fallback 없음, reset 403에서 credential 미표시를 검증했다. typecheck·build 통과, lint 0 errors·기존 7 warnings, backend build 경고 없음.

확장 `scripts/smoke-tenant-transactions.py`는 실제 API의 서로 다른 48자리 초기 secret, no-store, 재조회·SQLite dump에 secret 없음, 빈 member password 생성, weak input·임의 role 거부, 신규 계정의 must-change flag, reset weak input 차단을 확인했다. 실제 Chromium에서 MEC 사용자 생성과 masked 접속 정보·reveal·확인 뒤 제거도 통과했다. 기존 job 재시작·복구 조회 흐름과 `scripts/smoke-commercial-integrity.py` real·auto·DB 시작 경계도 통과했다. 운영 배포 전이다.

기존 계정의 알려진 초기 password 사용 여부와 회전은 운영 전 조사·migration이 필요하다. 응답 유실·account 생성 뒤 binding 실패 시에는 관리자 reset 또는 승인된 재조정이 필요하며 secret 재조회 API는 제공하지 않는다. 현재 생성·member 흐름의 request idempotency, credential vault와 개인별 자격 증명, breach password 검사, 조직 정책·MFA·JIT·최소 권한, 실제 Rancher 버전의 초기 로그인·password reset 검증 및 복구 승인은 후속 범위다. C07·C08 전체를 완료 처리하지 않는다.

## 2026년 10월 5일 상용 라이선스의 발급 권한 분리

서버 내장 HMAC 발급 비밀값과 unsigned LicenseInfo 저장을 제거하고 Ed25519 공개키 검증으로 바꿨다. token은 issuer key ID와 원문 claims bytes에 domain을 포함해 서명하며 server에는 발급 개인키나 기본 발급 공개키를 내장하지 않는다. 설정 파일의 공개키만 신뢰한다. `ed25519-dalek = 2.2.0`을 직접 사용하며 기존 lockfile의 동일한 transitive package를 재사용했다. 새 package version 변경은 없다.

저장 파일에는 signed_key만 포함하고 private storage로 atomic replacement·0600을 적용한다. 상태·feature 조회마다 서명·발급자·product·날짜·등급·한도·feature를 재검증한다. unsigned 기존 파일·변조·만료·공개키 누락·손상은 active=false와 기본 feature만 반환하고 체험으로 바꾸지 않는다. 잘못된 날짜는 영구 라이선스가 아니다. 체험 최초 기록은 create_new로 저장하고 미래·음수·손상 기록과 쓰기 실패에는 체험을 활성화하지 않는다.

활성화는 verified 최고관리자만 수행한다. JWT의 허위 admin claim·X-User header를 보낸 실제 operator는 기존 principal 시험에서 403이다. invalid token 400·공개키 확인 불가 503·저장 실패 500을 반환하며 확인 실패한 활성화는 기존 정상 파일을 교체하지 않는다.

오프라인 `tools/license-issuer.py`를 추가했다. OpenSSL Ed25519 key generation·서명과 server의 Rust 검증이 실제 API smoke에서 상호 운용된다. 개인키와 발급 output은 새 0600 파일로 작성하고 private key 덮어쓰기를 거부하며 stdout에 secret·token을 출력하지 않는다. 공개키 overlap·기존 issuer 제거를 지원하고 production 키는 만들지 않았다. 발급 작업은 오프라인 직렬 운영을 전제로 하며 HSM·동시 발급 서비스가 아니다.

라이선스 화면에서 성공 응답 object를 문자열로 렌더하던 오류와 network 실패 후 활성화 버튼이 잠기는 문제를 고쳤다. 검증 상태를 한국어·영어·일본어로 설명하고 누락된 화면 번역·textarea label을 추가했다. 비활성 라이선스를 무기한으로 표시하지 않는다.

전체 backend `cargo test --locked --no-fail-fast -- --test-threads=2`는 247 unit + 5 auth flow + 28 integration, 총 280건 통과했다. 라이선스 unit 13건은 실제 verifier의 정상 서명·claims 일치, payload/signature/key ID 변조·다른 issuer·version·기존 HMAC 거부, 공개키 overlap·제거·중복·weak key, 날짜·feature·한도·holder/product 검증, 저장 파일 재검증·unsigned/손상 파일·누락 root·체험 저장 경계를 시험한다. 예전 integration의 6건은 서버를 호출하지 않고 HMAC 구현과 feature 목록을 자체 복사한 시험이어서 제거했으며 raw 총 시험 수가 줄어든 이유도 여기에 있다.

첫 기본 병렬 전체 실행에서는 기존 legacy AXGate SSH 성공 시험 1건이 실패했다. 단독 재실행은 통과했고 동시 실행 수를 2로 제한한 전체 재실행도 통과했다. 실패 assertion이 연결 error를 출력하지 않아 원인을 timeout으로 확정하지 않는다. 기본 병렬 실행의 flakiness와 CI 자원/timeout 검증은 남아 있다. 이 실패를 숨기거나 모든 실행 방식의 통과로 확대하지 않는다.

frontend LicensePanel 1 suite 4건은 status object 성공 응답, 잘못된 signature 거부, network 실패 후 버튼 복구, 만료 안내 및 무기한 미표시를 검증했다. 최종 typecheck·build 통과, lint 0 errors·기존 7 warnings다. backend `cargo build --locked`도 경고 없이 통과했다. `scripts/smoke-signed-license.py`는 실제 OpenSSL 발급→Rust verifier→HTTP 활성화, unsigned·변조 저장 파일·등록 token 거부, 공개키 누락 503·복원·overlap·retirement, expired 발급 거부 및 기존 정상 token 보존, 0600·private key 덮어쓰기 차단을 통과했다. `scripts/smoke-commercial-integrity.py`의 real·auto 연결 실패·recovery unknown·손상 checkpoint·영구 DB 시작 경계도 통과했다.

[발급·wire format·rotation·기존 고객 이전 절차](license-signing.md)를 추가했다. 실제 API의 요금제 feature·동시 server limit 집행, 계약·갱신·개별 취소·clock rollback 대응·기기/organization binding·HSM·독립 감사·운영 migration은 아직 필요하다. 현재 root가 바꿀 수 있는 clock·설치 파일까지 방어했다는 뜻이 아니다. 운영 서비스는 변경하지 않았으며 C04 전체를 완료 처리하지 않는다.

## 2026년 10월 5일 계정 터미널 접속권·실시간 채널 회수

OWASP의 WebSocket·session 관리 지침과 Teleport의 짧은 credential·live session locking 설계를 다시 확인하고, 현재 로그인 session에 묶인 자체 admission을 구현했다. 비교 제품의 인증서·OS identity 격리나 성능을 재현했다는 뜻은 아니다. [근거·프로토콜·운영 이전·잔여 범위](terminal-admission.md)를 별도로 정리했다.

`POST /api/terminal/tickets`는 공통 JWT/session/현재 회원 인가를 통과한 최고관리자만 사용한다. 32-byte OS CSPRNG의 opaque ticket을 발급하고 hash만 cross-worker process memory에 저장한다. 원래 query·Origin·username·session ID를 묶고 최대 30초와 원래 access JWT 만료의 정확한 시간 상한을 적용한다. 1초 미만 남은 token은 401로 갱신을 유도한다. 유효 미사용 ticket은 session당 8개·전체 4096개로 제한하며 capacity 429·entropy 실패 503을 구분한다. 발급 응답은 no-store다.

WebSocket admission에서 원자적으로 ticket을 제거하고 binding·만료·재사용을 검사한다. 현재 session·회원 상태·실제 권한을 다시 확인한다. JWT role claim·X-User를 권한으로 쓰지 않는다. 기존 query JWT는 401이고 native Authorization header는 유지한다. 실제 upgrade를 검사한 뒤만 tmux 생성·attach로 진행하여 일반 HTTP 요청의 shell 생성도 차단했다.

계정 WsBridge는 각 수신 메시지 전과 유휴 1초 주기에 현재 session·회원·endpoint 권한을 검사한다. session revoke·logout·계정 정지/삭제·터미널 권한 회수 또는 7일 된 login session은 code 1008로 접속을 끊는다. dashboard·job SSE도 같은 grant로 실제 GET endpoint의 권한을 검사하고 각 전달 전 및 idle 주기에 확인하여 회수 후 stream을 닫는다. SSE에 터미널의 최고관리자 제한을 잘못 재사용하지 않으며 기존 읽기 권한은 보존한다. snapshot·terminal history와 heartbeat stream에도 같은 경계를 적용한다. SSE의 Cache-Control은 no-store로 변경했다.

frontend는 reconnect마다 header-authenticated refresh client로 새 ticket을 얻고 원래 query bytes를 유지한다. pending 발급·history 요청을 취소하고 늦은 응답·이전 socket callback·unmount 뒤 callback을 무시한다. history 조회 실패를 접속 자체의 실패로 만들지 않는다. code 1008은 무한 재연결하지 않고 한국어·영어·일본어로 session 종료/접근 회수를 안내한다. dashboard stream은 JWT query를 제거하고 fetchWithRefresh를 사용한다. file download는 window.open JWT query에서 같은 header client의 blob download로 교체했고 server 응답도 no-store로 바꿨다. 50MB 기존 한도는 유지한다.

최종 전체 backend `cargo test --locked --no-fail-fast -- --test-threads=2`는 259 unit + 5 auth flow + 28 integration, 총 292건 통과했다. 새 12 unit은 opaque·single use·target/origin binding·정확한 JWT expiry 상한·unknown/duplicate/credential query·global/session capacity·동시 소비 한 명·current role/status/session/age·실제 JWT middleware의 replay/revoke/spoof 거부·query JWT 거부 및 native header 유지·idle SSE 회수와 event 직전 정지·endpoint별 읽기 권한을 검증했다. 기존 AXGate debug legacy DH 성공 fixture가 5초 timeout으로 다시 실패하여 기능 시험의 상한을 15초로 늘리고 오류 내용을 assertion에 추가했다. 이는 인증 전 pin 검사·auth count 검증을 약화하거나 production timeout을 변경한 것이 아니다. 실제 release 성능 측정은 별도 gate다.

frontend 4 suites 15건은 URL에서 JWT 없음, 정확한 query byte 보존·connection마다 ticket 발급·invalid/rejected ticket·다른 origin 거부·익명 share 경로 유지, 기존 terminal windows 8건, header file download·403 거부, header dashboard stream을 검증했다. 최종 typecheck·build 통과, lint 0 errors·기존 7 warnings다. backend `cargo build --locked`는 경고 없이 통과했다.

`scripts/smoke-terminal-tickets.py`는 격리 DB·auth·명시적 mock provider·전용 TMUX_TMPDIR에서 실제 API query JWT 거부·invalid upgrade의 빈 session registry·잘못된 query·target 변경·replay·실제 31초 경과 만료·no-store·logout에 따른 admission 거부를 검증했다. 실제 dashboard HTTP stream은 logout 뒤 EOF에 도달했으며 socket timeout으로 열린 채널을 잘못 완료 처리하지 않았다. Chromium은 실제 로컬 셸 ticket admission과 분리 창의 새 admission, 파일 다운로드 내용·no-store·헤더 인증, dashboard 헤더 인증·active SSE, logout의 WebSocket 1008과 SSE reader EOF, page error 및 URL JWT 없음까지 통과했다. opaque 보조 document에 storage를 쓰던 test init은 동일 origin의 main frame으로 한정했고 Chrome requestfinished 대신 실제 reader EOF를 관찰했다. test browser timeout에는 process group을 정리하고 fixture가 기록한 tmux session만 정리한다.

분리 창에서 실제 `printf` 입력의 출력 marker를 확인하고 연결된 터미널 screenshot `screenshots/terminal-ticket.png`도 확인했다. signed license의 실제 OpenSSL→Rust→HTTP smoke도 새로운 principal metadata 뒤 통과했다. 기존 real/auto의 no-mock fallback·failed job recovery·손상 checkpoint·영구 DB 시작 경계 regression도 통과했다. 운영 서비스나 실 장비는 변경하지 않았다.

이는 접속 admission과 향후 전달/입력 회수다. 이미 OS로 보낸 명령·백그라운드 process·tmux 자체를 취소하지 않으며 검사와 실제 OS write/delivery 사이에 완전한 transaction cutoff가 없다. 주기 검사도 runtime scheduling의 영향을 받으며 1초 hard SLA가 아니다. proxy ticket query redaction·WSS deployment·공유 링크 cryptography/active 회수·OS 사용자별 실행·JIT/승인/MFA·session recording/retention·독립 감사·다중 replica의 atomic consumption 및 scale 검증은 후속 범위다. C09 전체 완료나 상용 GA로 표시하지 않는다.

## 2026년 10월 5일 tenant 회원 삭제의 실제 자원 경계

C07 중앙 인가 이전을 준비하면서 기존 `DELETE /api/mec/v1/tenants/{tenant}/members/{binding}`가 URL의 tenant와 Rancher binding 소속을 연결하지 않고 전역 binding ID를 삭제하는 결함을 확인했다. 현재 최고관리자만 변경할 수 있더라도 잘못된 tenant 선택과 향후 위임 관리자 확대에서 위험한 경계다.

삭제 전에 저장된 tenant 존재와 Rancher project 연결을 확인하고, 해당 project의 현재 binding 조회에서 **binding ID와 project ID가 모두 일치**해야 삭제하도록 변경했다. 없는 tenant·다른 tenant의 binding·이미 삭제된 binding은 404, project 미설정은 400이며 upstream 조회 실패에는 삭제를 실행하지 않는다. 다른 tenant의 binding 존재 여부를 별도 공개하지 않는다.

실제 Rancher adapter는 project filter를 HTTP query encoder로 전달한다. binding ID와 project ID는 길이가 제한된 ASCII resource identifier만 허용하여 경로나 query 삽입을 거부한다. project 조회가 foreign/missing project identity 또는 안전하지 않은 binding ID를 돌려주면 502로 실패하여 회원 목록 공개와 삭제 인가에 사용하지 않는다.

인증 middleware와 실제 삭제 handler·mock service를 연결한 HTTP 시험은 타 tenant ID 주입, tenant 미존재, project 미설정, binding 미존재, 정상 삭제, 재삭제를 확인하고 외부 tenant의 binding이 보존됨을 검증한다. 별도 loopback HTTP 시험은 실제 Rancher adapter의 query encoding과 정상·빈 목록·다른 project·project 누락·위험한 ID 응답을 확인한다. OS command나 실제 Rancher 운영 자원은 변경하지 않았다.

검증: `cargo test --locked --no-fail-fast -- --test-threads=2`에서 unit 262건, auth-flow 5건, integration 28건, 총 **295건**이 통과했다. 신규 경계 시험 3건이 포함되어 있다. `git diff --check`도 통과했다. 개발 소스 변경이며 운영 배포 전이다.

이 작업으로 tenant 관리자에게 Rancher 회원 변경 권한을 개방하지 않는다. 최고관리자의 가입 승인, 위임 상한, 승인된 나비맨 identity와 외부 Rancher user ID의 명시적 연결, 만료·회수의 실제 provider 반영, 변경 직전 재인가와 durable 감사가 이어져야 한다. 단순 username 일치만으로 두 시스템의 identity를 합치지 않는다. list와 delete 사이 upstream 변경은 transaction으로 묶이지 않으므로 immutable binding identity 가정과 provider 수준 조건부 삭제도 후속 검토 대상이다. page 밖 binding은 현재 조회로 증명되지 않으면 거부하며 전체 pagination 처리는 별도 구현해야 한다.

Rancher 공식 문서는 project membership이 실제 접근 범위를 결정하고 owner가 역할을 부여한다고 설명한다. 또한 기본 project-member/owner의 권한과 여러 project에 걸친 namespace 생성·삭제 영향도 명시하므로 나비맨의 viewer/operator/admin을 단순 문자열로 자동 변환해서는 안 된다. [Membership과 역할](https://ranchermanager.docs.rancher.com/v2.14/how-to-guides/new-user-guides/authentication-permissions-and-global-configuration/manage-role-based-access-control-rbac/cluster-and-project-roles), [project 회원 권한 주의사항](https://ranchermanager.docs.rancher.com/how-to-guides/new-user-guides/add-users-to-projects).

## 2026년 10월 5일 승인 회원과 Rancher identity의 명시적 연결

C07의 실제 자원 인가 기반으로 최고관리자 회원 화면에 **Rancher 계정 연결**을 추가했다. `GET/POST /api/users/{id}/external-identities/rancher`는 verified principal과 현재 활성 최고관리자만 허용한다. 일반 사용자·위임 관리자에게 전체 외부 identity를 공개하지 않는다. 가입 대기·반려·정지 회원은 연결 대상이 될 수 없다.

서버는 client가 선택한 realm을 현재 Rancher adapter의 canonical base URL과 비교한다. scheme·host·설치 path를 구분하고 userinfo·password·query·fragment가 들어간 주소는 identity realm으로 거부한다. mock은 별도 `mock://rancher` realm이며 unavailable adapter는 실제 realm을 만들어내지 않는다. remote user ID를 직접 조회하고 응답 ID 일치·명시적 enabled·username snapshot을 확인한다. 활성 상태가 응답에서 누락되면 활성으로 추정하지 않는다. username이 같다고 연결하지 않으며 이름이 다른 계정도 관리자 확인 후 ID로 연결할 수 있다.

연결은 SQLite의 `(local_user_id, realm)` primary key와 `(realm, remote_user_id)` unique constraint에 저장한다. 확인 시각·관리자·사유·외부 username snapshot을 보존한다. 동일 realm의 로컬·외부 계정 중복 연결 및 기존 연결 교체는 409다. 8개 동시 연결 시도에서도 1개만 저장된다. 성공 감사 로그는 연결과 같은 IMMEDIATE transaction에서 저장하며 감사 실패에는 두 기록을 함께 rollback한다. upstream 조회 이후 현재 계정 승인·최고관리자 역할·로그인 session·JWT expiry를 다시 확인하고 local commit 동안 회원/session store를 잠가 로컬 회수와의 경합을 줄인다. 조회와 연결 성공 응답은 no-store다.

화면은 realm·remote user ID·확인 사유를 명시하며 실패를 성공으로 표시하지 않는다. 연결 후에는 저장 당시 확인 시각과 관리자를 보여준다. 사용자에게 이 연결 자체가 project 역할 부여·회수를 뜻하지 않는다고 안내한다. 현재 화면의 문구는 기존 회원 화면과 같은 한국어이며 다국어 적용은 잔여 범위다.

설계 근거는 RFC 7643의 stable provider ID와 provisioning domain별 identity 구분 원칙이다. 이를 자체 mapping에 적용한 것이며 Rancher가 SCIM을 구현하거나 나비맨이 SCIM provisioning을 완성했다는 주장은 아니다. [RFC 7643, §3.1](https://www.rfc-editor.org/rfc/rfc7643.html#section-3.1).

외부 계정 연결은 실제 provider 권한 중앙화의 기반이다. 현재 기존 Rancher 회원 추가 API를 이 연결에 자동 연결하거나 위임 관리자에게 열지 않는다. project-role allowlist·권한 상한·기한·provider grant ownership·회수 queue·외부 권한 재조회·승인된 계정 이전이 이어져야 한다. 연결 제거·교체는 남아 있는 외부 권한을 조용히 잃어버리지 않도록 아직 제공하지 않는다. 로컬 계정 삭제 후에도 durable mapping과 감사는 남고 remote ID를 다른 회원에게 재할당하지 않는다. DB와 파일 기반 회원 저장의 완전한 한 트랜잭션 이전, provider instance의 재설치/ID 재사용 증명과 URL 변경 시 승인된 migration도 필요하다. 저장된 verified_at은 과거 확인이며 현재 활성 상태를 보증하지 않는다. 외부 provider 조회와 local commit이 분산 transaction인 것은 아니다.

검증: `cargo test --locked --no-fail-fast -- --test-threads=2`에서 unit 267건, auth-flow 5건, integration 28건, 총 **300건** 통과. 연결 API·원자 감사 rollback·재시작 보존·동시 중복·realm 정규화·활성 상태 누락 거부 시험이 포함된다. `cargo build --locked`, frontend 타입 검사·build가 통과했고 lint는 오류 0건·기존 경고 7건이다. 회원 화면과 새 editor의 2개 suite **7건**이 통과했다. `scripts/smoke-external-identities.py`의 실제 server+Playwright는 연결 201/no-store·reload 보존·교체 UI 없음·URL credential 없음·page error 없음을 확인한다. 화면 산출물은 `screenshots/external-identity.png`다.

운영 서비스와 실 Rancher는 변경하지 않았다. 실제 경쟁 제품 설치 비교·상용 GA·C07 전체 완료로 표시하지 않는다.

## 2026년 10월 5일 tenant 접근 차이 확인과 완전한 프로젝트 목록 조회

실제 권한 부여·회수의 선행 조건을 점검하면서 기존 Rancher adapter가 첫 페이지의 PRTB만 조회하는 문제를 확인했다. 이를 그대로 변경 작업에 사용하면 뒤쪽 페이지의 기존 연결을 놓치거나, 외부 접근을 회수했다고 잘못 판단할 수 있다. 이번 단계는 이 조회 경계와 회원별 차이 확인을 구현했다. durable grant ownership·권한 변경 실행기는 아직 다음 단계이며 C07 전체 완료가 아니다.

PRTB 조회는 요청 limit 1000과 next 링크를 따라 동일 origin·동일 collection path·동일 project filter를 유지한다. 상대 next도 지원하고 project filter가 빠진 링크에는 원래 값을 보존한다. 다른 host·scheme·path·userinfo·fragment·다른 project·중복 project filter는 요청 전에 거부한다. Rancher HTTP client의 자동 redirect도 끄고, proxy의 비정규 endpoint는 올바른 base URL로 설정해야 한다. 페이지 순환·binding ID 중복·scope 불일치·user/role ID 누락·잘못된 응답 shape는 성공 목록으로 반환하지 않는다. 전체 100페이지·10,000 binding·30초 deadline을 넘으면 부분 결과를 성공으로 반환하지 않는다. provider의 group/principal binding처럼 concrete user ID가 없는 형식은 현재 502로 보고하며 무시하지 않는다.

제공된 pagination.total은 페이지 간 일관성과 수집 건수를 검사한다. partial=true인데 다음 링크와 수집 total의 증명이 모두 없으면 불완전한 조회로 처리한다. 공식 문서의 pagination 설명과 실제 Norman 구현을 함께 확인했다. Norman query handler는 마지막 페이지에서도 startIndex>0이면 partial=true로 유지하고 total을 제공하므로 final-page partial만으로 실패 판정하지 않는다. 실제 loopback 3페이지 fixture도 이 형태를 재현한다. [Rancher v3 API](https://ranchermanager.docs.rancher.com/v2.14/api/v3-rancher-api-guide), [Norman query handler](https://github.com/rancher/norman/blob/master/api/handler/query.go), [pagination JSON fields](https://github.com/rancher/norman/blob/master/types/types.go). 제공된 전체 건수와 링크를 검증하는 것이 provider의 concurrent mutation을 완전히 방지하는 snapshot transaction은 아니다.

`GET /api/mec/v1/tenants/{tenant}/members/{local_user_id}/access-plan`은 현재 계정·session·tenant 관리 권한을 확인한다. scoped tenant 관리자는 자신의 scope의 승인된 하위 회원만 확인하며 자신·동급 관리자·최고관리자·다른 tenant의 회원을 조회할 수 없다. 최고관리자는 정지·만료·소속 해제 상태도 점검할 수 있다. linked realm과 정확한 remote user ID로 provider를 조회하고 해당 project의 해당 user 직접 binding만 응답한다. 다른 회원과 다른 project의 연결을 함께 반환하지 않는다. 전역 identity 연결의 확인 사유·관리자 정보도 scoped 보고서에서 제외한다.

보고서는 나비맨의 활성 회원·membership 활성화·만료, tenant lifecycle, 외부 계정 활성, 검토용 reference role, 직접 binding·역할 차이·검증되지 않은 접근 경로를 구분한다. 최고관리자 회원 화면과 scoped 회원 화면의 tenant 소속 항목에서 **실제 접근 확인**으로 연다. 나비맨 접근이 차단·만료됐는데 외부 연결이 남으면 이를 직접 표시한다. GET-only 조회이며 provider 권한을 부여하거나 회수하지 않는다. 직접 역할이 일치하거나 직접 연결이 없다고 총 유효 권한을 보증하지 않는다. global/cluster/group/other-project/custom role rules는 아직 별도 확인해야 한다. 기본 project-member/owner의 namespace 영향도 경고하며 reference role을 바로 적용 가능한 안전 정책으로 주장하지 않는다.

upstream I/O 뒤 현재 session·관리자 권한·대상 회원 상태/역할/membership·접근 만료·tenant project/status를 재검사하고 변동하면 보고서를 반환하지 않는다. 실제 Rancher HTTP fixture의 읽기 도중 관리 scope 또는 로그인 session을 회수하는 두 경합에서도 401 빈 응답을 검증한다. no-store 응답과 frontend AbortController를 적용하고 재확인 실패에는 과거 결과를 제거한다. 브라우저 확인에서 보고서와 회원 표의 이전 접근 상태가 충돌하는 문제를 발견해, 확인 완료 후 부모 회원 목록도 갱신하도록 수정하고 표의 실제 접근 차단 표시까지 smoke에 추가했다. 새 report 문구는 한국어·영어·일본어를 제공한다. 기존 회원 화면의 한국어 문구 국제화는 남아 있다.

인증 middleware를 포함한 API 시험은 scope·하위 회원 경계, 계정/identity/project 누락, 만료·비활성·정지 후에도 남는 외부 binding, unrelated user/project 제외, 공급자 오류, session 회수를 검증한다. 실제 adapter HTTP 시험은 절대·상대 next, project filter 보존, final partial+total, malformed/missing data, missing user identity, partial without proof, 외부 URL, 경로·query 삽입, 순환·중복 ID를 검증한다. 회원 화면은 direct-binding-only 의미, refresh 실패의 과거 결과 제거, 일본어 표시를 시험한다.

검증: `cargo test --locked --no-fail-fast -- --test-threads=2`에서 unit 272건·auth-flow 5건·integration 28건, 총 **305건** 통과. `cargo build --locked`, frontend build와 타입 검사 통과. 회원·identity·inspection 3개 suite **11건** 통과. lint는 오류 0건·기존 경고 7건이다. `scripts/smoke-tenant-access-inspection.py`는 실제 서버와 scoped manager의 Playwright 화면에서 direct role match, 최고관리자의 소속 차단 뒤 남아 있는 외부 binding 표시, global identity 편집 버튼 없음, no-store, URL JWT 없음과 page error 없음을 확인했다. mock 환경에만 자원을 만들었으며 screenshot은 `screenshots/tenant-access-inspection.png`다.

운영 서비스와 실제 Rancher는 변경하지 않았다. 나비맨 내부 소속 차단이 기존 외부 권한의 자동 회수가 되지는 않는다. 이를 해결하기 위한 managed binding ownership, 변경 intent의 원자 저장/감사, 실행 전 재인가·역할 규칙 검증, expiry/revoke queue, UnknownResult 재확인과 승인된 재조정은 원래 상용화 범위에 유지한다.

## 2026년 10월 5일 외부 프로젝트 회원 변경의 durable 기록과 연결 소유권

최고관리자의 기존 Rancher 회원 추가·삭제 경로에 변경 intent와 정확한 binding 소유권 기록을 도입했다. `membership_operations`에 realm·tenant·project·remote user·role·관리자·source IP·user agent를 기록한 뒤 외부 binding 변경을 실행한다. intent와 감사는 같은 SQLite IMMEDIATE transaction이며 저장 실패에는 binding 변경을 실행하지 않는다. 비밀번호·토큰·초기 credential은 기록하지 않는다. 신규 외부 계정 생성은 이 binding intent보다 앞선 별도 단계이므로 계정 생성까지 durable transaction으로 보호한다고 해석하면 안 된다.

생성 응답의 ID·project·user·role이 모두 맞아야 candidate ID를 저장한다. 전체 project inventory에서 정확한 연결을 다시 확인한 뒤에만 `owned_project_bindings`와 완료 상태·성공 감사를 원자적으로 commit한다. 삭제는 URL tenant의 현재 project 목록에서 정확한 연결을 찾고, 삭제 후 목록에서 ID 부재를 확인해야 완료한다. provider가 202를 반환했지만 연결이 남아 있으면 완료로 표시하지 않는다. 기존 외부 연결은 생성 소유권으로 자동 채택하지 않으며 최고관리자의 legacy 삭제는 현재 소속 조회에 근거한다.

응답 유실·identity 충돌·readback 실패·완료 감사 실패에는 결과 불명으로 남긴다. 동일 realm/project/user의 미완료 intent는 다른 role이나 tenant alias를 통한 재시도도 차단한다. 생성 후보 ID는 완료 감사 rollback 뒤에도 보존한다. confirmed ownership의 tuple과 다른 삭제 요청은 거부하며 terminal 상태를 다시 완료 처리하지 않는다. DB 최종 기록 자체가 실패하면 적용 중 기록이 남을 수 있다. 재시작에서는 중단된 applying을 결과 불명으로 전환하고 감사도 원자 저장하며 실패에는 시작을 중단한다. 자동 replay는 하지 않는다.

외부 변경 직전에 현재 활성 최고관리자·session의 사용자와 7일 수명·원 JWT 만료·tenant project를 재확인한다. 생성은 tenant의 활성 lifecycle도 다시 확인한다. provider 호출 중 관리자가 로그아웃하면 외부 결과와 소유권은 기록하되 해당 관리자에게 성공 receipt와 초기 credential을 전달하지 않는다. 로컬 검사와 원격 변경은 분산 transaction이나 HA fencing이 아니므로 검사 직후 권한 회수 경합을 완전히 제거했다고 주장하지 않는다.

최고관리자는 **MEC → 테넌트 → 외부 프로젝트 회원 → 회원 권한 변경 기록**에서 최신 100건의 상태·변경 ID·realm·binding ID·관리자·시각을 조회한다. GET은 no-store이며 scoped 관리자에게 이 전역 실행 기록은 403이다. 외부 회원 추가·삭제 버튼도 현재 최고관리자에게만 표시한다. 기록 조회 실패 때 과거 완료 결과를 감추며 실패한 변경 요청 뒤에도 기록을 다시 읽는다. 기록 문구와 tenant 탭은 한국어·영어·일본어를 제공하고 탭 색상은 현재 theme 변수를 사용한다. 기존 source 회원 관리와 외부 프로젝트 회원 변경은 별도 기능이다.

검증: `cargo test --locked --no-fail-fast -- --test-threads=2`에서 unit 279건·auth-flow 5건·integration 28건, 총 **312건** 통과. 신규 저장 시험은 원자 감사 rollback, 확정 소유권, 동시 요청 8개 중 단일 intent, 미완료 재시도 차단, 재시작과 recovery 감사 실패를 확인한다. 실제 loopback Rancher HTTP와 인증 middleware·회원 변경 handler 시험은 정상 생성/삭제, 생성 응답 유실, 다른 identity 응답, readback 실패, 완료 감사 실패, provider 호출 중 logout, accepted 삭제 뒤 연결 잔존의 7개 상황을 확인한다. 운영 provider 자원을 변경하지 않았다.

회원·identity·inspection·변경 기록의 4개 UI suite **14건**이 통과했다. backend/frontend build와 타입 검사 통과, lint 오류 0건·기존 경고 7건. `scripts/smoke-tenant-access-inspection.py`는 격리된 mock 서버와 실제 browser에서 최고관리자의 확정 변경 기록과 no-store, scoped 기록 조회 거부, scoped 회원 접근 차이 보고서와 URL credential 없음·page error 없음을 확인한다. 화면 산출물은 `screenshots/membership-operations.png`다. `scripts/smoke-commercial-integrity.py`는 real/auto upstream 미설정 시 가짜 데이터 fallback 없음과 잘못된 mode/DB 및 손상 checkpoint의 시작 중단을 재확인했다.

개발 소스이며 운영 배포 전이다. 이 기록은 foreground handler의 durable 추적 기반이며 분산 작업 queue를 완성한 것이 아니다. 승인된 UnknownResult 재조정 API, worker lease·fencing, 외부 user 생성 intent와 credential 복구, Rancher user 목록의 완전한 pagination, provider instance 재설치/ID 재사용 증명은 남아 있다. local user ID와 managed grant 정책 연결·최소 권한 role 규칙·tenant 관리자 쓰기 위임·만료/소속 차단의 실제 외부 자동 회수도 아직 구현되지 않았다. 직접 binding 확인은 group/global/cluster/다른 project의 유효 접근과 권한 전파 완료를 보증하지 않는다. C07·C10·C11 전체 완료나 경쟁 제품 실측 비교로 표시하지 않는다.

## 2026년 10월 5일 결과 불명인 회원 변경의 관리자 재확인

회원 변경의 결과 불명이 영구적으로 재시도를 막는 상태에 머물지 않도록 최고관리자의 `POST /api/mec/v1/tenants/{tenant}/membership-operations/{operation}/reconcile`과 기록 화면의 **재확인 후 기록 확정**을 구현했다. 현재 unknown_result이며 durable binding ID가 저장된 요청만 지원한다. 사용자는 정확한 요청 ID와 bounded 사유를 입력하고 조회 당시 binding ID·updated_at을 제출한다. applying 중인 실행·완료 기록·오래된 snapshot·다른 tenant의 요청은 확정하지 않는다. 원격 ID가 없는 응답 유실은 이름·user·role 일치만으로 생성 소유권을 채택하지 않고 미확정 상태를 유지한다.

서버는 현재 활성 최고관리자·session·원 JWT 만료를 검사하고 저장된 realm 및 tenant project가 현재 설정과 같아야 현재 전체 project inventory를 읽는다. 생성 요청은 durable ID와 project/user/role이 정확히 일치해야 하며 삭제 요청은 해당 ID의 부재가 확인되어야 한다. 아직 남은 삭제 대상, scope 불일치, 공급자 조회 실패에는 원래 기록을 유지한다. 이 endpoint는 provider GET만 수행하며 create/delete를 replay하지 않는다. upstream IO 뒤 관리자·session·project를 다시 검사하고 local transaction에서 snapshot과 state를 다시 비교한다. 최종 metadata commit 동안 회원·session store를 잠그고 활성 최고관리자·session 사용자와 7일 수명·원 JWT 만료를 다시 확인하여 로컬 회수가 그 commit에 끼어들지 않도록 한다. 이 잠금은 upstream IO를 포함하지 않는다.

ownership 생성/회수, completed 상태, 재확인 성공 감사는 같은 IMMEDIATE transaction에서 commit한다. 감사 실패에는 ownership과 상태 모두 rollback한다. 감사에는 재확인 관리자·사유·관측 시각·원 요청 관리자·이전 오류·직접 binding에 한정된 증거 범위와 provider 재실행 없음이 남는다. 원 요청의 actor는 바꾸지 않는다. 성공 receipt는 no-store이며 UI는 일치하는 operation ID·completed·provider_write_replayed=false receipt를 검증한 뒤 기록을 새로 조회한다. 화면 문구와 입력은 ko/en/ja를 지원하고 left alignment를 유지한다.

원격 POST의 결과가 유실되었다는 이유만으로 재시도하지 않는 방향은 [RFC 9110 §9.2.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-9.2.2)의 non-idempotent 요청 재시도 조건을 참고했다. 현재 구현은 새로운 원격 변경을 재시도하는 대신 저장된 identity로 현재 상태를 관측해 과거 요청의 결과만 확정하는 자체 절차다. 경쟁 제품 설치 비교나 provider idempotency API 구현을 뜻하지 않는다.

검증: 전체 backend unit 281건·auth-flow 5건·integration 28건, 총 **314건** 통과. 저장 시험은 reconciliation 감사 실패의 원자 rollback, candidate/version mismatch, terminal 재확정 거부, 원 실행자와 재확인자 분리, candidate 없는 요청과 applying 상태 거부를 포함한다. 실제 Rancher adapter HTTP·auth middleware·handler 시험은 오래된 snapshot·다른 tenant·조회 도중 logout 거부, candidate가 남은 create의 확정, 미확인 create ID 추정 금지, readback 실패, accepted delete가 나중에 사라진 후 확정, provider 쓰기 횟수 불변을 검증한다. UI 4개 suite **17건** 통과. backend/frontend build·타입 검사 통과, lint 오류 0건·기존 경고 7건.

`scripts/smoke-tenant-access-inspection.py`는 임시 mock DB에 완료 감사 실패를 주입하여 실제 외부 연결은 생성되었지만 ownership은 없는 결과 불명 요청을 만들었다. 실제 최고관리자 browser가 사유와 정확한 요청 ID를 입력하여 no-store receipt를 받고 completed와 같은 binding ID의 ownership이 저장됨을 확인한다. scoped manager의 기록 조회와 reconciliation POST는 모두 403이며 기존 접근 차이 화면도 검증했다. URL credential·page error가 없다. 재확인 입력 화면은 `screenshots/membership-reconciliation.png`, 확정 결과는 `screenshots/membership-operations.png`다. 모든 fixture와 fault는 격리된 임시 환경에 한정했다.

개발 소스이며 운영 서비스와 실 Rancher는 변경하지 않았다. 현재 관측은 provider의 변경을 막는 snapshot transaction이 아니고, 이 기록 확정이 기존 local 승인 정책·group/global/cluster 권한까지 보증하지 않는다. candidate 없는 생성의 승인된 identity 채택·권한 회수 실행 계획·local user와 managed grant 연결·만료 worker·HA fencing·독립 감사 저장·2인 승인 정책은 후속 범위다. C07·C10·C11 및 상용화 전체는 완료하지 않았다.

## 2026년 10월 5일 회원 소속과 소유 권한 연결 및 자동 회수

최고관리자의 **MEC → 테넌트 → 외부 프로젝트 회원 → 회원 정책과 외부 권한 연결**에 확정된 생성 요청과 승인 회원의 소속 정책을 연결하는 화면/API를 추가했다. `POST /api/mec/v1/tenants/{tenant}/membership-operations/{operation}/associate`는 현재 최고관리자·session·JWT, 정확한 생성 요청 ID, 명시적 자동 회수 동의와 사유를 요구한다. 연결은 외부 권한을 새로 만들지 않는다. 같은 username이 아닌 immutable external identity의 realm/account ID로 승인 회원과 현재 활성 owned binding의 user ID가 일치해야 한다.

현재 전체 provider project inventory에서 binding ID·project·user·role을 확인한다. source 회원은 승인·활성·해당 tenant 접근 가능 상태여야 하고 viewer는 read-only, operator는 read-only/project-member, tenant admin은 기존 허용 역할 중 하나의 상한을 적용한다. 이것은 기존 소유 연결의 관리 정책을 승인하는 단계이며 역할 문자열을 보고 새 provider 권한을 자동 부여하지 않는다. source role snapshot을 저장하고 역할 변경 때 기존 연결을 회수한다. 일반 project-member/owner의 광범위한 권한과 custom role 유효 권한의 검토는 잔여 범위다.

`managed_project_bindings`는 owned creation operation과 source local user ID를 immutable하게 연결한다. identity·활성 ownership·미완료 mutation 없음·현재 tenant project와 active lifecycle을 같은 IMMEDIATE transaction에서 재확인하고 association과 성공 감사를 원자 저장한다. upstream IO 뒤 source 활성/role·최고관리자·session을 다시 검사하고 로컬 commit 동안 회원/session store를 잠근다. 중복·재할당은 거부하고 legacy binding을 소유권으로 자동 채택하지 않는다. 조회 GET은 no-store이며 scoped 관리자에게 전역 association·inventory 권한을 개방하지 않는다.

독점 소유 MEC DB를 연 정상 서버에서 단일 주기 작업을 시작한다. 한 scan은 저장된 정책 100개를 순환 처리하고 완료 후 30초 기다린다. 회원 만료·소속 접근 비활성·계정 정지/삭제·소속 역할 변경이나 tenant 비활성을 감지하면 저장된 realm과 현재 tenant project, 해당 binding의 정확한 user/role을 읽고 변경 직전에 source 조건을 재확인한다. 연결이 이미 없으면 삭제하지 않고 부재를 재확인한다. 삭제 시에는 기존 durable membership intent·audit를 사용하고 재조회에서 ID 부재가 확인된 뒤에만 owned state를 revoked로 바꾼다. 오류나 accepted-but-present는 결과 불명으로 남기고 같은 대상의 삭제를 replay하지 않는다. 복구 후 최고관리자는 기존 reconciliation 절차로 부재를 확인한다. 계정이나 소속을 다시 활성화해도 provider 권한을 자동 재부여하지 않는다.

최소 권한과 기간이 제한된 승인 접근의 방향은 재확인한 [Teleport Enterprise Access Requests 문서](https://goteleport.com/docs/identity-governance/access-requests/)를 참고했다. 해당 문서는 reviewer와 임시 권한 수명을 설정하는 흐름을 설명한다. 나비맨은 이 방향을 기존 owned Rancher binding의 소속 만료/회수 정책에 적용한 자체 구현이며 Teleport의 certificate/JIT/dual authorization 기능을 동등하게 완성했거나 경쟁 제품을 설치해 실측했다는 의미는 아니다.

검증: 전체 unit 285건·auth-flow 5건·integration 28건, 총 **318건** 통과. 신규 시험은 association 원자 감사 rollback·명시적 identity·중복·role 상한, 만료·비활성·정지·삭제·역할 변경·invalid expiry 회수, unmanaged owned 권한과 다른 회원의 외부 binding 보존, provider identity 변경 시 삭제 금지, 재활성화 시 자동 재부여 없음이다. 실제 Rancher HTTP·auth middleware·handler 시험은 최고관리자 association, accepted 삭제 뒤 결과 불명 유지와 재삭제 없음, 나중에 부재가 확인된 뒤 관리자 reconciliation을 확인했다. 화면 5개 suite **19건** 통과, backend/frontend build와 타입 검사 통과, lint 오류 0건·기존 경고 7건.

격리된 실제 서버+Playwright 시험은 감사 fault로 만든 결과 불명 create의 확정, 최고관리자 policy association, scoped manager association/reconciliation/history 거부, source 비활성 뒤 실제 background scan의 owned revoked 저장, scoped access-plan의 source deny와 direct binding 0개, no-store·URL credential 없음·page error 없음을 검증했다. `scripts/smoke-commercial-integrity.py`도 real/auto의 upstream 미설정 fail-closed와 mode/DB/checkpoint 시작 경계를 재확인했다. 화면은 `screenshots/managed-membership-policy.png`와 `screenshots/managed-membership-revoked.png`다. 모든 자원과 SQL fault는 임시 mock 환경에만 만들었다.

개발 소스이며 운영 서비스·실 Rancher는 변경하지 않았다. scan 규모·upstream 응답 시간에 따라 회수 지연이 있으므로 30초 이내 회수나 즉시 접근 차단을 보증하지 않는다. preflight 실패·realm/project 변경·binding identity 충돌은 삭제하지 않고 다음 scan 또는 관리자 검토가 필요하다. scan 상태·차단 이유의 별도 경보, clock 신뢰, 확장 가능한 queue/worker lease/HA fencing와 회수 SLA 측정은 남아 있다. source 재활성화와 원격 삭제 사이 분산 경합을 완전히 fence하지 않으며 외부 권한 복원은 명시적 재승인이 필요하다. tenant profile 삭제 후 project mapping 부재는 자동 삭제하지 않는다. provider 재설치의 ID 재사용, local 승인과 durable policy의 완전한 transaction 이전, 독립 감사·2인 승인·custom role·scoped 쓰기 위임·통합 grant 생성도 잔여 범위다. tenant 초기 생성 orchestrator가 만든 기존/default binding 등 이 ledger 밖의 경로는 아직 managed association 대상으로 자동 편입하지 않는다. 직접 연결 부재는 group/global/cluster/다른 project의 전체 접근 회수를 보증하지 않는다. C07·C10·C11 및 상용화 전체를 완료 처리하지 않는다.

## 2026년 10월 5일 신규 외부 권한 부여와 회수 정책의 통합

최고관리자에게 `POST /api/mec/v1/tenants/{tenant}/managed-memberships`와 **회원 외부 권한 부여와 자동 회수** 화면을 추가했다. 승인된 local member ID를 선택하면 현재 설정의 Rancher realm·명시적으로 연결된 remote user ID·저장 당시 username을 먼저 보여준다. client가 검토한 realm/account ID를 제출하며 현재 realm/link와 다르면 새 요청을 거부한다. 이름으로 계정을 합치거나 외부 계정을 자동 생성하지 않는다. source 소속의 활성·만료·role 상한, 현재 활성 tenant project와 정확한 enabled remote account를 검증한다. 기존 direct binding이 하나라도 있으면 역할을 추가하거나 자동 채택하지 않고 409로 검토를 요구한다.

역할은 최고관리자가 명시적으로 선택한다. viewer에는 read-only, operator에는 read-only/project-member, tenant admin에는 현재 supported 3개 기본 역할의 상한을 적용한다. 자동 회수 동의와 provider 역할 범위 검토 확인이 모두 필요하다. [Rancher 공식 project 회원 문서](https://ranchermanager.docs.rancher.com/how-to-guides/new-user-guides/add-users-to-projects)는 기본 Member/Owner의 namespace 관리가 다른 project의 읽기 권한에도 영향을 줄 수 있음을 설명하므로 화면에 이 한계를 명시했다. role 이름만으로 tenant 격리를 보증하지 않는다. custom role 분석과 tenant 관리자에게 쓰기 위임하는 정책은 아직 별도 gate이며 이 경로는 현재 최고관리자만 허용한다.

외부 호출 전에 `requested_managed_grants`에 local user·source role·사유와 approval/automatic-revocation intent를 저장한다. membership intent·관리 승인 감사와 같은 IMMEDIATE transaction이다. 현재 explicit identity·활성 tenant project도 그 transaction에서 확인하며 저장 실패에는 원격 POST를 실행하지 않는다. provider가 정확한 binding identity를 반환하고 전체 project inventory에서 확인된 뒤에만 owned binding·managed association·completed·성공 감사를 같은 transaction으로 저장한다. 추가로 관리 정책을 연결하는 두 번째 수동 단계가 필요하지 않다. 기존 외부 연결에 대한 수동 association 경로도 유지한다.

완료 감사 실패에는 소유권·정책·완료 상태 모두 rollback하고 durable candidate/관리 의도를 결과 불명으로 유지한다. 최고관리자 reconciliation으로 candidate가 정확히 확인되면 ownership과 승인된 관리 정책이 함께 완성된다. 외부 호출 중 source 접근이 사라져도 실제 생성 결과와 관리 정책을 잃지 않으며 202 receipt로 현재 source deny를 표시하고 주기 회수의 결과 확인을 안내한다. caller가 정지/로그아웃된 경우 기록은 보존하지만 receipt를 전달하지 않는다. source 재활성화나 role 증가를 보고 remote grant를 자동 복원/승격하지 않는다.

화면은 한국어·영어·일본어를 제공한다. source 선택 변경의 이전 preview는 사용할 수 없고 늦은 요청은 AbortController로 무시한다. role 변경 후 scope 검토 동의를 다시 받는다. receipt의 operation/binding ID·local member·completed·automatic revocation·현재 source 허용 boolean을 검증한 뒤 결과를 표시하고 회원 목록·history·managed inventory를 새로 읽는다. 기존 username 기반 add/create-account UI는 **고급: 외부 연결만 추가 또는 계정 생성**으로 접고 자동 회수 정책과 연결되지 않는 경로임을 안내했다.

검증: 전체 backend unit 286건·auth-flow 5건·integration 28건, 총 **319건** 통과. 신규 primitive 시험은 managed intent/approval audit 실패의 원자 rollback과 중복 intent 차단을 확인한다. 실제 Rancher HTTP·인증 middleware·handler fixture는 managed 정상 생성, disabled remote account·명시적 identity 누락·source role 상한 초과 시 원격 쓰기 없음, 완료 감사 실패 시 ownership/policy rollback, root reconciliation 뒤 정책 복구, provider POST 응답 유실 때 candidate 추정·재생성 금지, 생성 중 source disable 또는 tenant 종료 상태 변경의 202와 후속 회수를 확인한다. 검토한 외부 identity 변경도 원격 쓰기 전에 거부한다. provider 쓰기 횟수도 확인하여 create replay가 없음을 검증한다. UI 6개 suite **23건** 통과, backend/frontend build·타입 검사 통과, lint 오류 0건·기존 경고 7건.

`scripts/smoke-tenant-access-inspection.py`의 실제 임시 서버+Playwright는 기존 결과 불명 확정/association/소속 비활성 회수 뒤, 회원을 짧은 만료 소속으로 다시 승인하고 신규 managed grant UI에서 명시적 realm·account ID preview와 두 검토 동의를 확인했다. 201/no-store와 ownership/policy의 동시 저장, 추가 수동 association 없이 만료 뒤 background 회수, unrelated project-owner binding 보존, scoped manager의 새 POST 403, URL credential 없음과 page error 없음을 확인한다. SQL 검증도 requested intent 1건·managed association 2건·active owned 0건을 확인한다. `scripts/smoke-commercial-integrity.py`는 real/auto fail-closed와 mode/DB/checkpoint startup 경계도 재확인했다. 화면 산출물은 `screenshots/new-managed-grant.png`다. 운영 자원은 변경하지 않았다.

개발 소스이며 운영 배포 전이다. managed intent가 곧 외부 생성 완료나 유효 권한을 의미하지 않는다. 특히 생성 응답 유실로 candidate ID가 없는 요청, inventory 확인 실패, 감사 저장 실패의 결과 불명은 자동 회수 대상 ownership으로 아직 완성되지 않을 수 있으며 관리자 재조정이 필요하다. 이 미확정 관리 요청의 자동 관측/경보·만료 cleanup queue와 승인된 identity 채택은 잔여 범위다. 로컬 approval과 remote 호출을 분산 transaction으로 묶거나 queue fencing·즉시 회수를 완성한 것은 아니다. source 정책의 파일 저장과 DB intent 사이 전체 저장 모델 이전, provider instance의 ID 재사용 증명·custom RBAC·2인 승인·tenant scoped 쓰기·원 orchestration/default grant 이전·외부 user 생성 및 credential intent도 계속 구현해야 한다. 전체 유효 접근 회수와 상용화 완료를 주장하지 않는다.

## 2026년 10월 5일 결과 불명 관리 요청의 보수적 자동 확인과 회수

신규 관리 요청의 완료 감사가 실패하여 소유권·회수 정책이 아직 확정되지 않았더라도, 사전에 승인된 `requested_managed_grants`와 저장된 candidate binding ID를 보존한다. source 회원 정책이 해제되었거나 tenant가 비활성인 경우에만 background scan이 현재 realm/project와 전체 외부 inventory의 정확한 binding ID·계정·역할을 대조한다. 일치하면 기존 snapshot CAS reconciliation으로 owned binding·managed policy·완료와 감사를 원자 저장하고 기존 회수 경로로 넘긴다. 새 외부 CREATE를 재실행하지 않는다. source가 조회 중 복원되면 이번 자동 확인을 중단한다. 감사 실패는 완료 기록을 rollback하므로 원격 삭제에도 진입하지 않는다.

candidate ID 없음·candidate 부재·다른 realm/project·계정/역할 불일치·조회 실패에는 생성 성공을 추정하거나 다른 연결을 채택하지 않는다. source가 계속 활성인 결과 불명 생성도 자동 확정하지 않는다. 이 요청은 최고관리자 검토 대상으로 남으며 기존 정확한 ID 기반 수동 reconciliation을 유지한다. 이미 승인된 자동 DELETE가 결과 불명인 경우에는 후속 전체 inventory에서 같은 binding ID의 부재가 확인되면 원격 DELETE를 반복하지 않고 감사와 revoked 상태만 확정한다. 이는 source가 다시 활성화되어도 이미 발생한 회수 결과의 기록이며 자동 재부여를 하지 않는다. 다른 actor/계정/역할의 미확정 삭제는 자동 처리하지 않는다.

최고관리자 전용 `GET /api/mec/v1/tenants/{tenant}/pending-managed-grants`와 **결과 불명 관리 요청** 화면을 추가했다. no-store로 local member·request·realm·project·remote account·candidate ID를 보여주며 미확정 상태를 회수 완료나 유효 접근으로 표시하지 않는다. 저장 ID 없는 경우의 외부 관리자 검토 필요를 별도로 안내한다. 한국어·영어·일본어와 조회 실패 시 과거 결과 숨김을 지원한다. 목록과 스캔의 snapshot 읽기 사이 이미 완료된 요청은 제외한다.

검증: backend 전체 unit 289건·auth-flow 5건·integration 28건, 총 **322건** 통과. 새 시험은 활성 source에는 자동 adoption 없음, source 해제 뒤 정확한 candidate 확인/회수와 다른 binding 보존, ID 누락·부재·realm/role 불일치에는 추정 금지, source 복원 후 미확정 자동 삭제의 부재만 확정한다. 실제 upstream HTTP·auth middleware·handler fixture는 새 inventory 응답과 no-store, 조회 중 source 복원으로 자동 확인 취소, 관리 완료 감사 실패 시 소유권·정책·삭제 모두 진행하지 않음, 감사 저장 복구 뒤 CREATE 1회·DELETE 1회로 최종 회수를 검증했다. 관련 UI와 i18n 7개 suite **25건** 통과, backend/frontend build·타입 검사 통과, lint 오류 0건·기존 경고 7건.

`scripts/smoke-managed-candidate-cleanup.py`의 격리된 실제 서버+Playwright도 통과했다. 완료 감사 fault로 만든 unknown managed create가 화면에 나타나는지, scoped manager 조회 403·no-store, source disable 후 background scan에서 정확한 candidate를 확정하고 회수하는지, 미확정 목록 해제와 unrelated project-owner 보존을 확인한다. 임시 SQLite에서 CREATE intent 1건·DELETE intent 1건·owned active 0건과 system confirmation 감사를 확인했다. 화면 산출물은 `screenshots/pending-managed-candidate.png`다. `scripts/smoke-commercial-integrity.py`의 real/auto fail-closed와 startup 경계도 통과했다. 운영 자원은 변경하지 않았다.

개발 소스이며 운영 배포 전이다. 모든 요청을 정리하는 범용 recovery queue가 아니다. candidate 없는 응답 유실, 실제 생성 부재의 판정, provider instance ID 재사용, 경보·진행 상태·차단 이유, durable worker lease/fencing, 분산 source 변경 경합과 clock 신뢰, 회수 SLA·확장 규모는 계속 남아 있다. 직접 연결 부재는 다른 project·cluster·global·group 권한의 전체 회수를 보증하지 않는다. 상용화 전체와 C07·C10·C11을 완료 처리하지 않는다.

## 2026년 10월 5일 최고관리자의 만료 위임과 tenant 관리자 권한 부여

[Teleport 공식 접근 요청 설정](https://goteleport.com/docs/identity-governance/access-requests/access-request-configuration/)의 reviewer 역할·조건과 [Rancher 공식 cluster/project 역할](https://ranchermanager.docs.rancher.com/v2.14/how-to-guides/new-user-guides/authentication-permissions-and-global-configuration/manage-role-based-access-control-rbac/cluster-and-project-roles)의 별도 회원 관리 권한을 비교했다. 이는 공식 문서 조사이며 유사 솔루션을 설치해 기능·성능을 직접 측정한 결과가 아니다. 나비맨에는 전체 provider 회원 관리 권한을 그대로 위임하기보다 최고관리자가 지정한 local manager ID·tenant·현재 realm/project·허용 역할·만료로 승인 범위를 구성했다. 2인 검토·JIT access request workflow·custom role 엔진 전체는 아직 구현한 것이 아니다.

최고관리자 전용 `GET/PUT /api/mec/v1/tenants/{tenant}/access-delegation`와 **최고관리자: 외부 권한 부여 위임** 화면을 추가했다. 현재 활성 tenant 관리자를 최대 32명 지정하며 read-only와 선택적 project-member만 허용한다. project-owner와 최고관리자 권한은 위임하지 않는다. 활성 위임은 최대 30일 내 만료가 필요하다. 변경은 확인한 tenant ID·realm/project·기존 version·bounded reason과 공식 역할 범위 검토를 요구한다. policy upsert와 변경 감사는 동일 IMMEDIATE transaction이며 audit 실패에는 저장하지 않는다. tenant project/lifecycle을 transaction에서 다시 확인한다. 오래된 version은 409로 거부하고, 단순 checkbox 활성 여부를 현재 유효 권한으로 표시하지 않는다.

tenant 관리자는 **MEC → 내 소속 tenant → 외부 권한 부여**에서 자기 소속의 현재 활성·접근 가능 하위 회원만 확인한다. `GET /tenants/{tenant}/delegated-access-context`가 현재 지정된 manager·scope·role·expiry·realm/project를 확인하고 scoped candidate와 최고관리자가 승인한 외부 identity만 제공한다. 자기 자신, tenant 동급 admin, global 최고관리자와 다른 tenant는 제외한다. manager가 외부 identity를 새로 연결하거나 비밀번호·외부 계정을 생성하지 못한다. source viewer는 read-only, operator는 승인된 read-only/project-member 안에서만 부여할 수 있다. project-member의 namespace 권한이 다른 project에 영향을 줄 수 있다는 공식 경고를 계속 표시하며 role 이름만으로 실제 격리를 보증하지 않는다.

기존 managed POST에 검토한 `expected_delegation_version`을 추가했다. 실제 caller의 현재 JWT·SID·session age·계정과 요청 method의 권한을 확인한다. 연결 계정 조회와 provider inventory 전에 caller/target/delegation을 확인하고, 원격 IO 뒤 User/Session lock 아래에서 다시 인가한다. intent 저장 transaction에서는 현재 delegation version·manager·realm/project·role·expiry를 재확인한다. `requested_delegated_grants`에 manager ID와 승인 version을 intent·managed policy·감사와 원자 저장한다. 전역 username이나 provider owner 역할에서 권한을 추정하지 않는다. outcome 추적·정확한 readback·owned/managed 저장·불명 결과 처리와 회원 source 정책 회수를 기존 경로로 이어 쓴다. 실행 중 caller 권한이 사라져도 이미 생성된 연결의 추적 기록을 잃지 않고 receipt를 전달하지 않는다.

위임 해제·만료·대상 manager 변경은 미래의 권한 부여를 차단한다. 이미 승인된 회원의 외부 권한을 자동으로 전부 취소하는 의미는 아니며 화면과 감사에 이 조건을 명시했다. 기존 연결의 회수는 해당 source 회원 정책을 해제하거나 최고관리자가 외부 권한을 검토하여 진행한다. 위임과 source grant의 수명을 혼동하지 않는다. UI는 한국어·영어·일본어, 저장된 정책의 초기 form 값, candidate 조회 오류 때 이전 form 숨김을 지원한다. background policy version, 외부 identity, source role 또는 허용 역할 변경에는 검토 동의를 다시 요구한다. 승인 receipt가 불완전하면 성공으로 표시하거나 CREATE를 재실행하지 않는다.

검증: backend unit 292건·auth-flow 5건·integration 28건, 총 **325건** 통과. 새 primitive 시험은 위임 expiry/CAS/audit rollback, 자기 자신·동급·foreign tenant/realm/project·owner role·만료 manager 거부, stale delegation intent 전체 rollback과 manager/version 저장이다. 실제 Rancher HTTP·auth middleware·handler fixture는 지정된 tenant manager의 정상 생성과 scoped context/no-store, delegation 부재·만료·stale version·다른 manager·self/peer·없는/다른 tenant target·조회 중 delegation/manager 회수·intent audit failure에서 원격 쓰기 없음, 최고관리자 policy 조회 거부를 검증했다. 관련 UI·i18n 10개 suite **33건** 통과, backend/frontend build·타입 검사 통과. lint 오류 0건·기존 경고 7건.

`scripts/smoke-access-delegation.py`의 실제 격리 서버+Playwright 시험도 통과했다. 최고관리자 화면의 위임 활성화·manager 지정·expiry·tenant 확인·원자 저장 receipt/no-store, 실제 scoped routing의 **내 소속 tenant** 진입점, scoped candidate 1명과 역할 상한, tenant 관리자 UI의 정상 managed grant를 확인했다. 이후 highest-only policy/history/identity 조회 403, root의 위임 해제 뒤 context/POST 차단과 기존 source grant 유지, source 해제 뒤 실제 background revoked 저장, unrelated project-owner 보존을 검증했다. 임시 DB에서 CREATE 1건·DELETE 1건·requested delegated grant 1건·owned active 0건·disabled delegation을 확인했다. 화면은 `screenshots/tenant-access-delegation.png`와 `screenshots/delegated-managed-grant.png`다. `scripts/smoke-commercial-integrity.py`의 real/auto fail-closed와 startup 경계도 통과했다. 최신 빌드의 `scripts/smoke-tenant-access-inspection.py`도 다시 통과하여 기존 최고관리자 unknown confirmation/association, 신규 managed grant, 자동 만료 회수와 위임 미설정 scoped write 거부의 회귀를 확인했다. 모든 자원·fault는 격리 fixture이며 실제 Rancher와 운영 서비스는 변경하지 않았다.

개발 소스이며 운영 배포 전이다. 현재 위임 대상은 새 managed project grant이고, raw binding CRUD·외부 account/identity/credential 관리·불명 결과 reconciliation·전역 history는 최고관리자 전용이다. 조직/team/custom 역할·typed action별 한도·2인 승인·위임 감사의 scoped 조회·member별 실제 유효 권한 전체 분석·clock 신뢰·source 저장의 transaction 이전·provider fencing·role dispatch 이후 분산 경합·회수 SLA·실 장비/고객 pilot은 남아 있다. 전체 상용화와 C07·C10·C11을 완료 처리하지 않는다.

## 2026년 10월 5일 scoped 관리 연결 목록과 별도 승인 회수

최고관리자의 tenant 위임에 `allowed_actions`를 추가해 `grant`와 `revoke`를 독립적으로 승인한다. 기존 정책은 새 action table을 처음 만드는 migration에서 grant만 부여한다. 이미 존재하는 action row가 사라지거나 policy version과 불일치하면 권한을 추정하지 않고 거부한다. schema 생성과 migration은 하나의 transaction이며 실패 시 rollback한다. 최고관리자 화면에서도 기존 정책의 회수 checkbox는 기본 해제하고 변경 후 범위 확인을 다시 요구한다. 회수만 위임하는 정책에서도 관리 목록은 열리며 신규 부여 context는 차단한다.

**MEC → 내 소속 tenant → 외부 권한 관리**에 `GET /tenants/{tenant}/scoped-managed-memberships` 기반 목록을 추가했다. 현재 자기 하위 회원의 owned+managed 연결만 보이며 자기 자신·동급 tenant 관리자·global 최고관리자·다른 tenant·소속이 제거된 회원은 제외한다. 비활성·정지된 회원도 하위 소속이 유지되면 기존 연결 정리를 위해 조회·회수할 수 있다. 소유권의 active/revoked, 이 승인 연결의 현재 source 정책, 현재 realm/project 일치, 미확정 요청과 최신 변경 상태를 표시한다. 이는 저장된 관리 기록이며 다른 global/cluster/group 권한이나 실제 유효 접근 전체의 조회가 아니다. 최고관리자 identity 연결 사유·전역 감사·IP·user-agent·민감 오류 내용을 scoped 응답에 넣지 않는다. 응답은 no-store다.

`POST /tenants/{tenant}/managed-memberships/{creation_operation}/revoke`는 creation 요청 ID·binding ID·ownership updated_at·delegation version·사유·직접 연결 범위 동의를 요구한다. 위임 manager, action, role, expiry와 현재 하위 소속을 확인한다. 현재 canonical realm/project의 전체 provider 목록에서 정확한 project/user/role/binding 일치를 검증하고, 원격 IO 후 현재 계정·SID·manager 정책을 다시 검사한다. intent transaction에서 현재 owned+managed tuple·version·회수 승인 snapshot·감사를 원자 저장한다. managed ledger에 없는 raw binding이나 외부 계정·source 소속 정책을 이 경로로 변경하지 않는다.

이미 정확한 연결이 없으면 DELETE 없이 부재를 확정할 수 있다. 있는 연결은 한 번만 DELETE하고 전체 재조회에서 부재가 확인된 뒤 owned revoked·완료 감사·operation completed를 저장한다. 응답 유실·202 후 연결 잔존·재조회 실패·완료 감사 실패는 unknown_result로 남긴다. 미확정 동일 회원 변경은 provider 조회 이전에 차단하며 DELETE를 재실행하지 않는다. 최고관리자의 기존 read-only reconciliation으로 나중에 정확한 부재를 확인할 수 있다. 실행 중 caller 권한이 사라져도 변경 추적은 보존하고 성공 receipt를 전달하지 않는다. 직접 연결 부재의 receipt는 `all_external_access_revoked: false`를 명시한다. source 정책과 unrelated binding은 유지하며 source가 활성 상태여도 자동 재부여하지 않는다.

한국어·영어·일본어 목록과 확인 form을 추가했다. 정책 version·binding timestamp 변경 때 동의를 초기화하고 실패 receipt 뒤 자동 재삭제를 하지 않는다. 공통 `mecList/useMecList`도 구조화된 404/error envelope와 data array 누락을 실패로 처리하도록 수정했다. 오류를 빈 inventory로 바꾸지 않으며 실패한 갱신에서 이전 목록을 비우고 오류를 표시한다. 성공한 빈 목록은 정상적으로 허용한다.

검증: backend unit **296건**, auth-flow **5건**, integration **28건**, 총 **329건** 통과. 실제 Rancher HTTP·auth middleware·handler fixture는 정상 회수, grant-only 거부, stale/expired/disabled 위임, peer/root/foreign/removed target 거부, 비활성·정지 회원 정리, project/binding/ownership 불일치, 조회 중 manager/policy 회수, intent·완료 감사 실패, 삭제 응답 유실·202·readback 실패·logout, 정확한 부재 재확정과 중복 삭제 없음 등을 검증했다. migration의 grant-only 이전·누락 action fail-closed·실패 rollback도 통과했다. 전체 frontend **30개 suite·113건** 통과했으며 목록 오류·receipt·별도 action 동의 시험을 포함한다. backend/frontend build·타입 검사 통과, lint 오류 0건·기존 경고 7건이다.

최신 frontend 빌드에서 `scripts/smoke-scoped-managed-revocation.py`의 격리 서버+Playwright가 통과했다. 부여만 승인한 manager의 회수 UI/API 차단, 최고관리자의 명시적 회수 승인, 오래된 version 409, 실제 scoped UI의 정확한 연결 회수·receipt·기록 갱신, source 정책과 unrelated project-owner 보존, DELETE 재실행 거부를 확인했다. 임시 DB에서 CREATE 1건·DELETE 1건·manager 회수 승인 기록 1건·owned active 0건을 검증했다. 화면은 `screenshots/scoped-managed-revocation.png`와 `screenshots/scoped-managed-ledger.png`다. 기존 `scripts/smoke-access-delegation.py`와 `scripts/smoke-commercial-integrity.py`도 통과했다. 실제 Rancher나 운영 서비스는 변경하지 않았다.

`scripts/smoke-tenant-access-inspection.py`도 최신 빌드에서 통과했다. 기존 최고관리자 불명 결과 확인·association·자동 회수, 신규 managed grant와 만료 회수, unrelated binding 보존과 위임 없는 scoped write 거부의 회귀를 확인했다.

개발 소스이며 운영 배포 전이다. 공식 문서 조사에서 확인한 [Rancher project 역할](https://ranchermanager.docs.rancher.com/v2.14/how-to-guides/new-user-guides/authentication-permissions-and-global-configuration/manage-role-based-access-control-rbac/cluster-and-project-roles)과 [Teleport 접근 요청 설정](https://goteleport.com/docs/identity-governance/access-requests/access-request-configuration/)을 참고한 자체 구현이며 경쟁 제품 실측 결과가 아니다. 관리 목록의 pagination·대규모 부하 검증, 전체 실제 유효 권한 분석, 삭제된 소속의 scoped recovery, scoped 감사 상세·경보, queue/lease/HA fencing·clock 신뢰·회수 SLA와 고객 pilot은 남아 있다. 범용 action/2인 승인 엔진과 전체 상용화 완료를 의미하지 않는다.

## 2026년 10월 5일 공유 터미널의 수명·진행 중 회수·저장·감사 보강

기존 공유는 접속 시작 때만 링크를 검사하고 이미 열린 `WsBridge`에는 grant가 없었다. JSON 읽기·쓰기 실패를 빈 상태 또는 성공으로 처리했고 worker 사이 read-modify-write도 직렬화하지 않았다. [OWASP WebSocket 지침](https://github.com/OWASP/CheatSheetSeries/blob/master/cheatsheets/WebSocket_Security_Cheat_Sheet.md)의 장기 연결 재인가·만료 종료·비밀 로그 제외와 [Session Management 지침](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)의 서버 측 수명 집행을 확인하고, 자체 개발 경계를 보강했다. 경쟁 제품 설치 실측이나 인증 결과가 아니다.

새 링크는 OS CSPRNG의 256-bit token·발급 최고관리자의 불변 local ID·정확한 terminal tuple에 연결한다. 최대 30일의 유한 수명이며 API/UI 기본이 읽기 전용이다. 기존 ID 없는 링크는 owner username으로 추정하지 않고 재발급을 요구한다. 소유자 목록에서 expired/legacy 링크도 사용 불가로 표시하고 복사를 막으며 삭제할 수 있다. public metadata와 password 확인에서도 현재 owner 상태를 확인한다. 실제 upgrade 전 tmux attach를 하지 않는다.

공유 `ShareGrant`를 `WsBridge`에 연결했다. 수신 메시지·output frame·history/geometry·시작/유휴 검사에서 현재 링크와 snapshot·owner ID/username/현재 최고관리자 권한·session owner/tmux_name을 확인한다. 취소·만료·owner 삭제/정지/역할 회수·대상 변경에는 1008로 attachment를 종료한다. 한 output poll을 128KiB로 제한해 연속 출력이 검사를 무한 점유하지 않도록 했다. 브라우저는 policy close에 자동 재접속하지 않는다. 원래 shell과 다른 링크는 유지한다. 기존 계정 연결도 각 output frame에 같은 인가 검사를 적용했다. 이미 실행한 명령이나 브라우저에 전달한 화면의 소급 회수, OS 프로세스 종료와 원자 cutoff를 구현한 것은 아니다.

공유 registry는 worker 공유 mutex와 안정적인 companion inode의 process exclusive flock을 사용한다. 0600 임시 파일·file fsync·atomic rename·parent directory fsync 뒤 memory를 갱신한다. corrupt/unavailable 파일의 빈 registry fallback을 제거하고 auth 초기화 전에 검증한다. 변경 저장 실패는 503이며 registry 전체를 unavailable로 만들고 기존 grant도 거부한다. old cache를 계속 제공하거나 성공 삭제 receipt를 반환하지 않는다. 최대 4096건·16MiB 저장 상한을 둔다. last-used는 informational memory 갱신이며 독립 durable session audit가 아니다.

추가 점검에서 공통 감사가 share bearer를 path에 그대로 기록하는 것을 확인했다. public 공유·소유자 취소 path는 이제 bearer 대신 SHA-256의 128-bit correlation ID를 남긴다. 성공/실패와 서로 다른 endpoint에서 같은 capability를 추적할 수 있고 원문 token·query·unknown suffix를 저장하지 않는다. 기존 hash-chain 이벤트를 다시 쓰지는 않았다. reverse proxy와 APM의 URL redaction은 별도 운영 범위다.

검증: backend unit **309건**·auth-flow **5건**·integration **28건**, 총 **342건** 통과. 신규 시험은 live grant 취소·expiry 경계·시간 역행·owner 정지/강등/삭제·같은 username 재생성·password/read-only/target 변경, 저장 후 재시작·exclusive owner·0600·병렬 mutation 보존·publication failure quarantine·corrupt JSON 거부, 실제 auth middleware의 finite/default/upgrade/no-store/503 계약, 감사 correlation/redaction이다. 전체 frontend **30개 suite·114건** 통과, build·타입 검사 통과, lint 오류 0건·기존 경고 7건이다.

최신 빌드의 `scripts/smoke-shared-revocation.py`는 실제 격리 서버·tmux·Playwright에서 owner UI의 read-only 기본 발급과 암호 gate, 정상 writable 입력과 readonly 차단, 동일 링크의 두 live attachment 취소와 1008, 새 화면 전송·자동 재접속 없음, 만료 종료, unrelated 공유·원래 shell 보존, owner 역할 회수 뒤 남은 공유 종료를 확인했다. 임시 파일의 owner ID·유한 수명·0600과 실제 audit의 bearer 비노출도 검사했다. 화면은 `screenshots/shared-terminal-revoked.png`다. `scripts/smoke-terminal-tickets.py`의 계정 접속권·파일 다운로드·로그아웃 WebSocket/SSE 회귀와 `scripts/smoke-commercial-integrity.py`의 real/auto fail-closed·startup 경계도 통과했다. 후자는 corrupt share registry가 auth 생성 이전에 시작을 중단하고 원문 손상을 보존하는 실제 process 시험을 추가했다.

운영 서비스·실제 서버 세션은 변경하지 않았다. 이전 server를 중지하고 frontend/backend를 함께 교체하는 migration이 필요하며 legacy 링크 재발급을 안내해야 한다. rollback으로 새 보안 경계가 사라지는 위험과 복구 절차는 [공유 운영 문서](shared-terminal-access.md)에 기록했다. 기존 12시간 재사용 password digest ticket의 표준 cryptography·일회용 소비·Origin binding, owner 권한 version의 영구 전달, rate/connection limit·JIT/MFA·개인 OS identity·독립 recording/audit·HA·신뢰 clock·규모/SLA·실제 proxy rollout은 남아 있다. C09와 전체 상용화를 완료 처리하지 않는다.

## 2026년 10월 5일 공유 터미널 일회용 접속권과 자원 제한

앞 단계에 남아 있던 12시간 재사용 proof를 제거했다. 공유 인증은 OS 난수 256-bit opaque 접속권을 발급하고 서버에는 digest와 승인 snapshot만 보관한다. 접속권은 최대 30초이며 링크의 남은 수명보다 길지 않다. 정확한 링크·query·Origin에 연결하고 소비를 원자 처리한다. 불일치 시에도 소비하며 replay·기존 signed 형식·접속권 없는 접속을 거부한다. 계정 터미널과 공유 접속권 저장소를 분리했다. Origin은 발급과 접속의 일치 검사이며 전역 허용 목록이나 기기 신원 증명은 아니다. 권한·링크 변경은 접속권 snapshot 검사와 기존 live grant 검사로 거부한다.

비밀번호 없는 공유도 접속권을 요구한다. 비밀번호 확인은 blocking 작업으로 분리하며 최대 4개 동시 작업을 허용한다. 대기 중 링크·소유자 권한 변경을 다시 확인하고 bcrypt 입력을 72 UTF-8 bytes로 제한한다. 기존 더 긴 비밀번호는 재발급이 필요하다. 인증 시도는 고정 60초 창에서 링크별 30회·실제 TCP peer별 60회·process 전체 240회다. 위조 X-Forwarded-For를 신뢰하지 않는다. reverse proxy 뒤 여러 이용자가 같은 peer 상한을 공유할 수 있어 운영 구성 검증이 필요하다. 미사용 접속권은 링크별 8개·전체 4096개, live 공유 연결은 링크별 8개·전체 64개다. 원자 reservation과 RAII 반환으로 경쟁·실패·종료 시 한도를 유지한다. 이 수치는 단일 process 안전 기본값이며 상품별 SLA나 분산 제한이 아니다.

브라우저는 매 재접속에 새 접속권을 받는다. 공유 HTTP는 control-plane JWT·refresh·cookie를 사용하지 않는다. 비밀번호는 입력 검증 후 화면에서 지우고 재접속용 메모리에만 유지하며 문서 종료 시 참조를 지운다. 비밀번호와 일회용 접속권을 브라우저 영구 저장소에 기록하지 않는다. 접속권은 WebSocket query에 전달하므로 proxy/APM redaction은 여전히 필요하다. 인증 실패에 자동 반복을 하지 않으며 링크 변경 때 이전 인증 상태를 버린다. 메모리 zeroization이나 XSS 방어 완료를 주장하지 않는다.

검증: backend unit **323건**·auth-flow **5건**·integration **28건**, 총 **356건** 통과. 전체 frontend **32개 suite·129건** 통과. backend/frontend build·타입 검사 통과, lint 오류 0건·기존 경고 7건이다. 시험에는 단일 소비 경쟁, expiry·snapshot·Origin/query/target 불일치, pending/live 상한과 반환, 비밀번호 반복 시도, blocking 검사 중 owner 권한 변경, malformed 응답과 frontend 재접속이 포함된다.

최신 `scripts/smoke-shared-revocation.py`의 격리 서버·실제 tmux·Playwright 검증이 통과했다. 재접속마다 다른 proof, 재사용·Origin/query/target 불일치·기존 형식 거부, 실제 30초 만료, 공유/계정 접속권 상호 교환 거부, 비밀번호 반복 시도 429, 아홉 번째 live 연결 제한과 종료 후 자리 반환을 확인했다. public HTTP에 control-plane JWT가 없고 브라우저 저장소에 비밀번호·접속권이 없는 것도 검사했다. 기존 live 취소·만료·owner 강등·read-only·원래 shell 보존·private registry·감사 redaction 검증을 유지했다. 계정 터미널의 logout/WebSocket/SSE와 commercial-integrity startup 회귀도 통과했다. 화면은 `screenshots/shared-terminal-revoked.png`다.

[OWASP WebSocket 지침](https://cheatsheetseries.owasp.org/cheatsheets/WebSocket_Security_Cheat_Sheet.html)의 인증·Origin·장기 연결 인가·자원 제한을 검토한 자체 구현이다. 경쟁 제품 부하 실측이나 인증 결과는 아니다. 운영에는 배포하지 않았다. 기존 server 중지와 frontend/backend 동시 교체, 오래된 탭 갱신, legacy 링크·긴 비밀번호 재발급이 필요하다. [공유 운영 문서](shared-terminal-access.md)에 API·migration·제한을 기록했다. 소유자 권한 회수 뒤 재승격으로 기존 링크가 다시 활성화되는 것을 막는 영구 권한 version, JIT/MFA·OS identity·recording·독립 감사·HA·신뢰 clock·규모 검증과 proxy rollout은 남아 있다. C09와 전체 상용화 목표는 계속 진행 중이다.

## 2026년 10월 5일 공유 소유자의 승인 version과 재승격 차단

앞 단계에서 현재 계정의 역할만 확인하면 강등 후 재승격으로 이전 링크가 다시 허용되는 한계가 있었다. `NabimanUser.share_authorization_version`을 서버 생성 UUID로 저장하고 링크·접속권 승인 snapshot·live grant에 연결했다. username·role·status·password hash·TOTP 활성 상태/secret 변경은 중앙 계정 commit에서 version을 교체한다. 마지막 로그인·소속 bookkeeping만 변경하면 version을 유지한다. 비밀번호 변경과 MFA mutation도 같은 commit 경로를 사용한다. 저장 실패에는 이전 role·credential·version을 함께 복구한다. 사용자 API에는 version을 노출하지 않는다.

새 링크 발급은 저장 직전 현재 계정과 version을 재확인하고 account mutex를 publication까지 유지한다. 현재 활성 최고관리자여도 version이 다른 기존 링크는 metadata·password 확인·접속권·live attachment에서 거부하며 소유자 목록은 사용 불가로 표시한다. 재승격으로 자동 갱신하지 않고 검토한 새 발급만 허용한다. 시작 시 기존 계정의 누락 version은 생성해 저장하지만 기존 링크의 누락 version은 자동 채우지 않는다. 기존 owner ID가 있는 링크도 version이 없으면 재발급 대상이다.

검증: backend unit **327건**·auth-flow **5건**·integration **28건**, 총 **360건** 통과, backend build 통과. 새 시험은 역할 강등/재승격·계정 정지·password/MFA 변경·last-login 유지·디스크 roundtrip/API 비노출, 실제 파일 저장 실패의 계정/version rollback과 성공 publication, 이전 grant snapshot과 legacy generation 거부를 확인했다. frontend 제품 코드는 이번 단계에서 변경하지 않았으며 앞 단계의 **32개 suite·129건** 결과를 유지한다.

확장한 `scripts/smoke-shared-revocation.py`는 실제 격리 서버·tmux·Playwright에서 강등 시 live 종료, 재승격 후 오래된 metadata/auth 거부, 소유자 목록의 사용 불가 표시, 새 로그인과 새 승인 링크의 정상 접속을 검증했다. 실제 서버를 재시작한 뒤 저장 version 유지·오래된 링크 계속 거부·새 승인 링크 허용도 확인했다. 앞 단계의 일회용/재사용/Origin/query/target/namespace/30초 만료·시도/연결 제한·원래 shell 보존 시험을 그대로 통과했다. `scripts/smoke-commercial-integrity.py`도 통과했다. 운영 서비스와 실제 서버 세션은 변경하지 않았다.

[OWASP Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)의 권한 변경 시 이전 승인 식별자를 폐기하는 원칙을 검토한 공유 capability 구현이다. 계정 JWT 전체의 새 session lifecycle이나 경쟁 제품 실측을 완료했다는 의미는 아니다. 사용 계약·version migration·비밀번호/MFA 변경 시 영향은 [공유 운영 문서](shared-terminal-access.md)에 기록했다. 기존 users.json parent fsync는 best effort이므로 전원 장애 내구성 강화, transactional 중앙 계정 저장, 오래된 백업 복구 시 승인 폐기, HA·mixed-version writer와 분산 전달은 남아 있다. 이번 정상 변경/재시작 증거로 전체 장애 모델·C09·상용화를 완료 처리하지 않는다.

## 2026년 10월 5일 계정 publication의 불명 결과와 재시작 경계

users.json의 rename 뒤 directory fsync 실패를 무시하던 계약을 제거했다. 계정과 공유 승인 version은 file fsync·atomic rename·필수 directory fsync 뒤에만 성공한다. publication 이전 실패는 기존 계정/version으로 rollback한다. rename 이후 실패는 디스크의 후보 상태와 메모리의 이전 권한 중 어느 것을 안전하게 수용할지 확정할 수 없으므로, rollback이나 성공 receipt 대신 server process 전체를 exit 78로 종료한다. worker·HTTP/WebSocket 연결과 실행 중 작업이 끊어질 수 있다. 이미 OS로 전달한 작업이나 tmux shell은 종료하지 않는다. availability를 희생하는 단일 process fail-closed 경계이며 고가용성 저장 엔진은 아니다.

시작 시 users file을 nonblocking으로 열고 regular file인지 검사한다. file과 parent directory를 sync한 뒤 내용을 읽어 기존 형식·빈 목록 검사와 승인 version migration을 수행한다. 최초 open의 NotFound만 초기 bootstrap으로 처리한다. 후속 validation/sync/read 오류를 NotFound bootstrap으로 해석하지 않는다. 지속적인 fsync 장애에는 관측 파일이 있어도 서비스를 시작하지 않는다. 이전 version이나 root 계정을 자동 복원하지 않는다.

검증: backend unit **330건**·auth-flow **5건**·integration **28건**, 총 **363건** 통과. post-rename 오류의 후보 파일 보존·불명 결과 분류, test subprocess exit 78와 정상 반환 없음, directory/FIFO 거부, 기존 pre-publication rollback을 검증했다. backend build 통과. frontend 제품 코드는 변경하지 않았으며 이전 **32개 suite·129건** 검증 결과를 유지한다.

`scripts/smoke-user-publication.py`는 격리 서버에만 임시 LD_PRELOAD shim을 적용해 auth directory의 fsync를 실패시킨다. 실제 HTTP의 root→viewer 변경에서 성공 receipt 없이 연결이 종료되고 exit 78, 후보 계정의 role/version 저장, 지속 장애의 재시작 거부, 장애 제거 후 이전 관리자 쓰기 권한과 전역 회원 조회가 복원되지 않음을 확인한다. 시스템/운영 환경에 shim을 설치하지 않는다. shared-revocation의 실제 브라우저·일회용 접속권·회수·재승격·재시작 회귀와 commercial-integrity startup 회귀도 통과했다.

[Linux fsync(2)](https://www.man7.org/linux/man-pages/man2/fsync.2.html)의 file sync만으로 directory entry 내구성을 확정할 수 없다는 계약을 확인했다. fault injection은 소프트웨어의 오류 처리를 검증하며 실제 전원 차단·장치 cache·파일시스템의 물리 내구성 시험이 아니다. [공유 운영 문서](shared-terminal-access.md)에 결과 불명 mutation의 자동 재시도 금지와 디스크/version 확인을 기록했다. rename 뒤 fatal 종료는 완료 감사 이벤트를 남기지 못할 수 있다. transaction 계정/감사 저장·관리자 복구 workflow·상위 directory 생성 내구성·백업 rollback 인가·HA와 고객 storage 시험은 남아 있다. 운영에는 배포하지 않았으며 전체 목표를 완료 처리하지 않는다.

## 2026년 10월 5일 원격 서버 신뢰의 승인 generation과 변경 충돌

진행 중 원격 접속 회수의 선행 경계를 점검했다. 기존 trust는 지문으로만 변경 충돌을 검사해 같은 키 재승인과 등록→해제 뒤 과거 빈 지문의 요청을 구분하지 못했다. `RemoteServer.host_trust_version`을 추가해 새 서버 생성·키 등록·같은 키 재승인·교체·해제·host/port 변경에 UUID를 새로 저장한다. 신뢰가 없는 record도 마지막 generation을 보존한다. 표시 이름·memo와 일반 상태 갱신은 generation을 유지한다.

관리 form은 조회한 `previous_version`을 이전 지문과 함께 제출한다. 기존 server write mutex에서 현재 endpoint·fingerprint·generation을 비교하고 불일치하면 409로 변경을 거부한다. version 없이 요청하는 구버전 client는 version이 저장된 record를 변경할 수 없다. legacy JSON의 누락 generation은 빈 값으로 읽고 현재 빈 값인 경우 첫 명시적 변경만 허용한다. 자동 TOFU나 이미 검증된 키의 자동 해제를 추가하지 않았다. async 상태 점검도 승인 generation이 같을 때만 결과를 병합하므로 과거 승인의 online 결과가 같은 키의 새 승인 상태를 덮어쓰지 못한다.

검증: backend unit **331건**·auth-flow **5건**·integration **28건**, 총 **364건** 통과. 기존 OpenSSH 실제 loopback fixture의 matching key 접속과 mismatched key 인증 전 차단도 전체 suite에 포함된다. 새로운 probe 시험은 endpoint와 키가 같아도 이전 승인 결과를 거부하고, 기존 메모/name 변경 보존 시험은 같은 generation을 유지하는 snapshot으로 수정했다. 전체 frontend **32개 suite·129건**, backend/frontend build·타입 검사 통과, lint 오류 0건·기존 경고 7건이다.

최신 빌드의 `scripts/smoke-membership.py`는 실제 handler에서 같은 키 재승인마다 version 변경, stale version의 revoke 409, 해제 후 과거 빈 지문의 요청 409와 명시적 현재 version 승인, host/port 변경의 신뢰 해제를 검증했다. 실제 scoped 관리자 UI의 등록·새로고침·해제도 통과했다. 기존 가입/최고관리자 승인·회원/서버/tenant 격리·위임 관리·권한 상승 차단·scope 정책·계정 회수·감사 identity 회귀를 유지했다. 화면은 `screenshots/host-trust-scope-admin.png`다. 운영 서버와 실제 원격 장비에는 배포하거나 접속하지 않았다.

[RFC 9110의 조건부 변경](https://www.rfc-editor.org/rfc/rfc9110.html#section-13.1.1)에서 확인한 lost-update 방지 목적의 application CAS다. HTTP If-Match/ETag 구현이나 경쟁 제품 실측 결과가 아니다. [서버 키 운영 문서](ssh-host-trust.md)에 API·legacy·동시 frontend/backend 교체 계약을 추가했다. 이미 열린 SSH/tmux는 여전히 처음 검증한 trust를 유지한다. 다음 단계는 이 generation과 정확한 saved-server tuple을 session metadata·live input/output·reconnect·shared attachment에 연결하고 legacy session을 명시적으로 처리하는 것이다. C03/C09·독립 변경 감사·HA·운영 rollout과 전체 상용화 목표는 완료하지 않았다.

## 2026년 10월 5일 원격 터미널의 승인 snapshot과 진행 중 회수

앞 단계의 remote host approval generation을 terminal admission에 연결했다. 새 SSH command를 만들 때 읽은 saved server record에서 server ID·host/port·SSH user·auth method·canonical key/fingerprint·generation의 `RemoteBinding`을 함께 만든다. 명령 생성 뒤 tmux 생성 전 계정 grant와 현재 승인 일치를 재확인한다. generation 없는 legacy 승인으로 새 terminal을 열지 않으며 명시적 키 재검토를 요구한다.

binding을 TmuxSession metadata에 저장한다. 계정 WsBridge는 현재 session/tmux tuple와 binding을, 공유 ShareGrant는 기존 owner/link/session 검사와 같은 binding을 검사한다. 현재 record 조회 오류·삭제·trust 해제·교체·같은 키 재승인·endpoint/user/auth method 변경은 기존 attachment의 input/output·history/geometry·idle 검사에서 1008로 종료한다. Local attachment는 remote registry 오류에 의존하지 않는다. 단일 서버 파일 조회를 매 검사에 수행하므로 대규모 처리량과 revocation latency SLA는 검증 전이다.

기존 원격 session 재접속은 저장된 generation을 새 값으로 추정하지 않는다. 현재 승인이 다르면 종료 안내만 제공하고 tmux attach 또는 새로운 local shell로 fallback하지 않는다. session 목록 preview와 REST scrollback도 현재 remote admission을 검사한다. 공유 목록의 usable과 public metadata의 alive/grid에도 검사한다. public auth가 일회용 입장권을 발급하더라도 마지막 binding 검사에서 무효 대상의 WS attach를 거부한다. binding 없는 legacy session은 ssh_cmd가 없고 label이 local인 경우만 local로 수용하며 나머지는 새로 열어야 한다.

검증: backend unit **333건**·auth-flow **5건**·integration **28건**, 총 **366건** 통과, backend build·fmt·diff 검사 통과. 신규 시험은 정확한 target tuple·generation·key 변경 거부, 이름/memo 유지, binding serialization roundtrip, legacy approval와 remote session 거부다. frontend 제품 코드는 변경하지 않았으며 이전 **32개 suite·129건** 결과를 유지한다.

`scripts/smoke-remote-binding.py`는 임시 Paramiko SSH server의 실제 loopback protocol과 OpenSSH·tmux·Playwright를 사용한다. 소유자와 공유 guest의 실제 SSH input/output, 같은 key 재승인 뒤 두 attachment 1008, old session 재접속/preview/scrollback 거부, 새 승인 session 접속, unrelated local shell 보존을 확인했다. 실제 Nabiman process 재시작 후 old generation 거부·current generation의 원래 session attach, 이어 saved target 삭제로 current attachment 1008도 통과했다. metadata에서 정확한 server ID와 두 distinct generation을 확인했다. 이는 검증용 SSH shell protocol fixture이며 실제 운영 장비나 원격 OS workload 실측은 아니다. 초기 시험 도구가 binary WS frame을 Blob 문자열로 읽던 오류를 고쳐 실제 SSH 화면 bytes를 decode한 최종 실행이 통과했다.

최신 빌드에서 shared-revocation의 실제 UI·일회용 소비·입력 제한·live 회수·재승격·재시작과 terminal-tickets의 계정 WS/HTTP download·logout/WebSocket/SSE 회귀도 통과했다. 운영 서버·실제 원격 장비는 변경하지 않았다. [OWASP WebSocket 지침](https://github.com/OWASP/CheatSheetSeries/blob/master/cheatsheets/WebSocket_Security_Cheat_Sheet.md)의 장기 연결 재인가 원칙을 saved-target 승인에 적용한 자체 구현이다. [서버 키 운영 문서](ssh-host-trust.md)에 migration과 영향 범위를 기록했다.

이 작업은 browser attachment 회수이며 기존 SSH process·원격 명령·tmux shell 자체를 죽이거나 이미 실행한 작업을 되돌리지 않는다. 검사와 OS IO 사이 원자 cutoff·네트워크 queued frame 회수·general probe/exec/JIT job 재인가·transaction terminal registry·독립 감사·scoped OS identity·HA·규모·복구 인가는 남아 있다. C03/C09와 전체 상용화 목표를 완료 처리하지 않는다.

## 2026년 10월 5일 터미널 metadata의 원자 저장과 정확한 상태 확인

`terminal_sessions.json`의 직접 truncate와 저장 오류 무시를 제거했다. 단일 writer flock, private temporary file, file fsync·rename·parent-directory fsync를 적용하고 startup에서 일반 파일·16 MiB·4,096개 한도·중복 ID와 tmux 이름·필드 유효성을 확인한다. 기존 lock이 있는 저장소의 JSON 유실·손상·조회 오류는 빈 목록으로 바꾸지 않고 password·JWT·users 초기화 전에 시작을 거부한다. 초기 공유 registry 생성은 앞서 일어날 수 있다. 저장 실패는 exit 79로 서버 전체를 중단하며 성공 응답을 제공하지 않는다. 이미 발생한 OS 작업을 되돌리거나 감사와 원자 commit하는 기능은 아니다.

tmux 조회는 정확한 대상의 명시적 부재와 확인 불가를 구분한다. socket·권한·도구 오류가 있으면 복원·정리에서 metadata를 보존하고 종료 요청은 확인된 종료/부재가 아니면 503이다. 목록에 alive·ended·unknown·blocked 상태를 제공한다. 화면은 확인 불가·승인 차단의 preview·접속·새 공유를 비활성화하며 변경 응답 유실은 결과 확인 안내를 표시하고 자동 재실행하지 않는다. 한국어·영어·일본어 문구를 추가했다.

최종 점검에서 tmux의 기본 prefix/pattern 선택을 확인했다. 접속·존재 검사·종료는 정확한 `=name`, pane 조회·변경은 `=name:`으로 지정해 승인된 대상이 없을 때 유사 이름으로 넘어가지 않게 했다. [tmux 공식 target 문법](https://man.openbsd.org/tmux)에 따른 자체 구현이며 경쟁 제품 실측이 아니다. 실제 격리 tmux에서 missing target과 비슷한 이름의 살아 있는 session을 만들고 조회·capture·kill이 그 이웃으로 넘어가지 않는 것을 확인했다.

검증: backend unit **338건**·auth-flow **5건**·integration **28건**, 총 **371건** 통과. frontend **33개 suite·133건**, frontend/backend build와 타입 검사 통과, lint 오류 0건·기존 경고 7건이다. 새로운 UI 시험은 unknown 표시, stale alive가 있어도 blocked 입장 거부, 정상 입장과 응답 유실 안내·자동 재시도 없음이다.

최종 빌드의 `scripts/smoke-terminal-registry.py`는 실제 HTTP 저장 장애의 EOF·exit 79·검토된 원본 보존·잘못된 registry 재시작 차단·명시적 fixture 복구 뒤 원래 shell 재접속, tmux 도구 조회 불가의 unknown·종료 503·metadata 보존·도구 복귀를 확인했다. `smoke-commercial-integrity.py`의 corrupt terminal/share checkpoint와 real/auto 서비스·job 복구 회귀도 통과했다. 최신 `smoke-remote-binding.py`는 실제 OpenSSH·tmux·loopback SSH protocol의 owner/shared input-output, 재승인 회수·재접속 거부·재시작·대상 삭제를 통과했다. `smoke-shared-revocation.py`와 `smoke-terminal-tickets.py`의 실제 UI·일회용 입장·expiry·owner 회수·재시작·로그아웃 WebSocket/SSE 종료 회귀도 통과했다. 모두 임시 fixture이며 운영 서버·실제 장비를 변경하지 않았다.

[운영 계약과 복구 제약](terminal-registry.md)을 추가했다. 저장 실패 시 서버 전체 중단과 요청 유실, 사전 capacity admission·긴 외부 identity의 제한 검증, parent-directory 생성 전체의 crash durability, OS 작업·metadata·감사 원자성, 특권 분리·HA·복구 인가와 규모 검증은 남아 있다. C03·C09·C10과 전체 상용화 목표를 완료하지 않는다.

## 2026년 10월 5일 터미널 생성 전 용량 검증과 metadata 여유 공간

앞 단계의 저장소 제한은 저장 시점에 검사하므로 새 tmux를 만든 뒤 한도 초과로 exit 79가 발생할 수 있었다. 새 session 후보와 전체 snapshot을 OS 생성 전에 검사하도록 변경했다. 중복 ID·이름, 4,096개 수 제한, 필드 유효성과 16 MiB 크기를 검사하고 모든 session의 제목·timestamp·timeout·boolean이 나중에 커질 공간도 보수적으로 예약한다. 제목 1,024 bytes의 JSON escape 최악 상한을 사용하므로 실제 수용 session 수는 수 제한보다 작을 수 있다. 이는 상품 quota나 성능 벤치마크가 아니다.

store mutex를 검사부터 tmux 생성·metadata 발행까지 유지해 동시 요청의 중복 공간 소비를 막는다. 용량이나 owner 등 필드 제한에 걸리면 기존 화면 notice/END 계약으로 종료하고 새 tmux·metadata 변경·프로세스 종료를 수행하지 않는다. 실제 저장 장치 오류는 exit 79 정책을 유지한다. 생성된 shell은 attach 전에 기록하므로 attach 오류가 나더라도 session 목록으로 관리할 수 있다. 새 tmux 이름의 사용자 부분은 UTF-8 문자 경계를 보존해 최대 128 bytes로 제한하며 소유권은 원래 username으로 판정한다. owner가 128 bytes를 넘는 계정을 지원 확대했다고 주장하지 않으며 현재는 생성 전에 거부한다.

검증: backend unit **340건**·auth-flow **5건**·integration **28건**, 총 **373건**과 backend build·fmt·diff 검사 통과. 신규 unit 시험은 중복·invalid owner·개수 초과를 원본 변경 없이 거부하고 현재 serialized 크기가 한도 안이어도 미래 필드 공간이 부족하면 거부하는 것이다. 제품 frontend는 바꾸지 않았으며 앞 단계의 **33개 suite·133건**, 타입·빌드 결과가 해당 코드의 최신 검증이다.

최신 `scripts/smoke-terminal-registry.py`는 실제 저장 실패·조회 불가·복구 회귀에 더해 유효한 대형 legacy snapshot에서 실제 WebSocket 동시 입장 두 건을 시험했다. 두 요청 모두 capacity 안내와 END를 받고 tmux 목록·metadata bytes가 바뀌지 않았으며 서버가 유지됐다. 명시적 fixture snapshot 복구 뒤 기존 shell도 살아 있었다. 실제 OpenSSH·tmux·loopback SSH의 `smoke-remote-binding.py`, account WS·HTTP download·logout/SSE의 `smoke-terminal-tickets.py`, 실제 shared UI·입장권·회수·재시작의 `smoke-shared-revocation.py`도 최신 빌드에서 통과했다. 운영 서버와 실제 장비를 변경하지 않았다.

[저장·입장 운영 계약](terminal-registry.md)을 갱신했다. 이미 여유 공간이 부족한 legacy metadata의 변경 처리, 긴 외부 identity 지원, 사용자·tenant별 한도, 전역 mutex 대기·OS 명령 timeout·부하 측정은 남아 있다. 저장 실패 중단, OS 생성과 snapshot 사이 crash window, 작업·metadata·감사의 원자성, 특권 분리·HA·복구 인가는 여전히 후속 범위다. 상용화 전체 목표를 완료하지 않는다.

## 2026년 10월 5일 기존 터미널 변경의 사전 검증과 EOF 오류 보존

이전 단계는 새 session에 여유 공간을 예약했지만 이미 가득 찬 legacy snapshot의 REST 변경은 메모리를 수정한 뒤 저장 제한에 걸려 exit 79로 이어질 수 있었다. 제목·공유·timeout·keepalive 변경을 `prepare_update`와 공통 commit helper로 이전했다. store mutex 안에서 전체 후보의 필드와 실제 serialized 크기를 확인하고, 실패하면 HTTP 409로 거부하며 기존 메모리를 유지한다. 성공 후보만 저장하므로 실제 디스크 저장 실패의 exit 79 정책은 유지한다. 새 입장 예약량을 변경에 강제하지 않아 크기를 줄이는 작업을 허용한다. 제목 거부 시 tmux 창 이름 변경도 수행하지 않는다. keepalive 해제 시 legacy preference timeout을 30일 상한으로 제한했다.

별도 검토에서 PTY EOF의 `finish`가 boolean 조회 실패를 shell 부재로 판단해 metadata를 지우는 잔여 경로를 확인했다. 정확한 대상의 명시적 부재인 `Ok(false)`만 제거와 END 발행으로 처리한다. 확인 불가이면 연결만 닫고 metadata·원래 shell을 보존한다. 정상 종료와 진행 중 계정/공유 회수 계약은 유지한다.

검증: backend unit **341건**·auth-flow **5건**·integration **28건**, 총 **374건**, backend build·fmt·diff·Python syntax 검사 통과. 새 unit 시험은 정확히 16 MiB인 map의 제목 증가·invalid timeout·누락 target 거부와 원본 보존, 크기를 줄이는 변경 성공이다. 제품 frontend 변경은 없으며 해당 코드의 최신 검증은 앞 단계의 **33개 suite·133건**·타입·빌드 결과다.

최신 `scripts/smoke-terminal-registry.py`는 실제 HTTP에서 정확히 16 MiB인 legacy snapshot의 제목·timeout·keepalive 증가를 409로 거부하고 메모리·disk bytes·tmux window name이 동일함을 확인했다. shared boolean을 줄이고 다시 원래 크기로 돌리는 변경은 성공했다. 기존 저장 장애·조회 불가·복구·동시 입장 거부 회귀도 통과했다. 이어 임시 wrapper로 backend tmux probe만 실패시키고 해당 fixture namespace의 유일한 client PID를 종료했다. 실제 PTY EOF에서 WS가 END 없이 종료되고 metadata·shell은 보존됐으며 연결 수가 0으로 내려갔다. wrapper 복구 후 alive도 확인했다. 실제 tmux server·운영 session을 종료하지 않았다.

최신 빌드의 `smoke-remote-binding.py` 실제 OpenSSH·tmux·loopback 승인/회수/재시작, `smoke-terminal-tickets.py` 실제 계정 UI·일회용 입장·HTTP download·logout WS/SSE, `smoke-shared-revocation.py` 실제 공유 UI·입장권·live 회수·재시작 회귀가 통과했다. 운영 배포는 하지 않았다. [터미널 저장 운영 계약](terminal-registry.md)을 갱신했다.

legacy snapshot의 attach/detach timestamp 증가, settings 저장의 별도 durability, 긴 identity 지원·사용자/tenant quota·OS 명령 timeout·mutex 부하, OS 작업·metadata·감사 트랜잭션·특권 분리·HA·복구 인가는 남아 있다. C03·C09·C10과 상용화 전체를 완료 처리하지 않는다.

## 2026년 10월 5일 터미널 환경설정의 원자 저장과 감사 append 직렬화

`terminal_settings.json`의 read/parse 실패를 기본값으로 숨기는 경로와 직접 덮어쓰기를 제거했다. startup에서 별도 단일 writer flock·일반 파일·1 MiB·4,096개 계정·중복 username·필드 범위를 확인하고 password·JWT·users 초기화 전에 거부한다. 초기 공유/session 파일은 앞서 생성될 수 있다. 기존 lock에 JSON이 없으면 초기 설치로 간주하지 않는다. 정상 registry에 해당 user record가 없을 때만 기본 설정을 반환한다. 누락된 legacy 필드의 serde 기본값은 유지하되 잘못된 값은 덮어쓰지 않는다.

환경설정은 shared memory mutex 아래 전체 후보를 검증하고 0600 temporary file·file fsync·rename·parent-directory fsync 뒤 발행한다. 여러 계정의 동시 저장이 다른 계정을 덮어쓰지 않는다. 후보 검증 실패는 기존 상태를 유지하며, 파일 발행 실패는 preferences registry를 격리한다. 이후 GET/PUT·keepalive 변경은 503, 새 terminal은 안내/END로 거부한다. 기존 shell·server·session registry 자체를 종료하지 않는다. 파일만 고쳐도 live 격리를 해제하지 않으며 검토된 snapshot과 저장 장애 복구 뒤 재시작을 요구한다. 기존 session 재접속의 계정·원격 승인 검사는 유지한다.

frontend는 환경설정 reload의 HTTP/네트워크 실패를 modal에 표시하고 save의 실패·응답 유실을 처리한다. 확인된 저장 응답 없이 state/localStorage cache를 바꾸거나 자동 재시도하지 않는다. 한국어·영어·일본어 안내를 추가했다. cache는 화면 표시용이며 server 설정 확인을 대신하지 않는다.

첫 최신 shared-revocation 실행은 실제 UI·live 회수·재시작 검증을 통과했지만 audit JSONL 전체 읽기에서 `JSONDecodeError: Extra data`로 실패했다. 동시 요청의 hash 계산·file append·rotation이 store mutex 밖에 있었고 `writeln!`의 JSON/개행 쓰기가 다른 요청과 섞일 수 있는 경로를 확인했다. 실패를 무시하거나 읽기에서 손상 줄을 제외하지 않았다. 이 작업들을 같은 AuditLog mutex 안에서 직렬화하고 JSON+개행을 하나의 buffer로 `write_all`한다. memory order도 같은 잠금에 두며 외부 forwarding은 잠금 밖에 유지한다. 단일 프로세스 append 경쟁을 수정한 것이며 독립 감사·손상 복구·다중 writer·fsync·쓰기 실패 정책·clear API 원자성·rotation chain 검증·외부 불변 저장을 완성한 것은 아니다.

최종 검증: backend unit **345건**·auth-flow **5건**·integration **28건**, 총 **378건**, frontend **34개 suite·136건**, backend/frontend build·타입·fmt·diff·Python syntax 검사 통과. lint 오류 0건·기존 경고 7건이다. settings unit 시험은 12개 계정 실제 동시 저장·private file·배타 writer·재시작, 손상/중복/유실/잘못된 값 거부, 후보 validation 유지와 실제 rename 실패 후 quarantine다. frontend 3건은 load 실패 표시, 실패 응답·network EOF 저장에서 confirmed state/cache 보존과 자동 재시도 없음이다. audit 신규 시험은 barrier로 16개 thread를 동시에 시작해 128건의 개별 JSON line·단일 hash chain·memory order를 확인한다.

`scripts/smoke-commercial-integrity.py`는 손상된 preferences JSON을 그대로 보존하며 users/JWT 초기화를 거부했다. 최신 최종 빌드의 `smoke-terminal-registry.py`는 실제 HTTP preferences 저장·재시작 유지, rename 실패 503·원본 backup 보존·조회/새 입장/keepalive 차단, 기존 두 shell alive·server 유지, 파일 복구 후 live 격리 유지, 검토된 복구 뒤 재시작과 같은 설정/원래 shell 복원을 통과했다. 저장소 용량·session 변경·실제 EOF·복구 회귀도 유지했다. `smoke-terminal-tickets.py`의 실제 account UI·download·logout WS/SSE, `smoke-remote-binding.py`의 실제 OpenSSH·tmux·loopback trust/회수/재시작, 수정 후 `smoke-shared-revocation.py`의 실제 공유 UI·입장권·live 회수·재시작·JSONL 전체 파싱과 capability redaction도 통과했다. 처음 실패한 감사 검사까지 포함해 최종 재실행이 통과했다.

운영 서버에 배포하거나 실제 원격 장비를 변경하지 않았다. [터미널 저장·환경설정 운영 계약](terminal-registry.md)을 갱신했다. username 기반 preferences의 rename/재사용 이전·브라우저 cache 계정 분리, tenant 강제 정책·상품 quota, parent-directory 전체 crash durability·HA·규모, session OS 작업·metadata·독립 감사 원자성은 남아 있다. C03·C09·C10·C11과 전체 상용화 목표를 완료하지 않는다.

## 2026년 10월 5일 불변 회원 ID에 따른 terminal 소유권과 account 공유 회수

terminal 소유권의 username 비교를 불변 local ID로 이전했다. 인증 middleware의 VerifiedPrincipal에 현재 검증된 user ID를 유지하고 ConnectionGrant가 그 ID를 snapshot으로 보관한다. 현재 username의 회원 ID가 바뀌거나 ID가 비어 있으면 열린 terminal/SSE grant를 유지하지 않는다. JWT·ticket wire format의 UID migration을 구현한 것은 아니며 기존 SID와 현재 회원 검사 계약은 유지한다.

새 TmuxSession에 `owner_id`를 저장한다. owner 이름은 생성 당시 표시 metadata다. account 재접속·목록·scrollback·제목/공유/timeout/keepalive·종료·새 share-link는 ID를 비교한다. ID가 없는 legacy와 다른 ID의 session 요청은 안내/END이며 join 여부와 관계없이 새 local shell로 fallback하지 않는다. shared flag도 현재 owner ID의 terminal 정책이 유효할 때만 사용할 수 있다. owner 삭제·정지·권한 상실 때 다른 계정이 붙어 있는 account shared attachment까지 input/output·idle 재검사에서 1008로 회수한다. OS shell·이미 실행한 명령 자체를 취소하지 않는다.

ShareGrant와 public metadata·owner link 목록·WS의 마지막 입장 검사도 terminal owner_id와 link owner_id를 연결한다. 표시 이름이 같은 다른 terminal이나 owner_id 없는 legacy terminal에는 기존 유효 링크도 붙일 수 없다. 링크 목록·회수는 불변 owner ID를 기준으로 하며 기존 legacy link의 이름에 따른 사용 불가 목록/삭제만 유지한다. username 변경은 기존 owner version/live-link 정책에 따라 과거 링크 입장을 거부한다. terminal의 표시 이름만 달라졌다고 소유권을 바꾸지 않는다.

검증: backend unit **347건**·auth-flow **5건**·integration **28건**, 총 **380건**, backend build·fmt·diff·새 fixture의 Node/Python syntax 검사 통과. 신규 ownership unit은 표시/로그인 이름 변경에 따른 ID 유지, 같은 이름의 다른 ID와 삭제된 owner의 account shared 거부, legacy blank ID serialization이다. share unit은 정상 발급 fixture의 ID를 명시하고 누락/다른 terminal ID 거부를 추가했다. ConnectionGrant 기존 시험에도 빈 ID와 같은 username의 replacement ID 거부를 추가했다. 제품 frontend 변경은 없으며 해당 코드의 최신 검증은 앞 단계 **34개 suite·136건**·타입·build·lint 오류 0건·기존 경고 7건이다.

신규 `scripts/smoke-terminal-owner.py`와 실제 Playwright WebSocket fixture는 계정 생성·로그인·실제 tmux 화면·account 공유·public link 발급 뒤 실제 owner DELETE와 같은 username의 새 계정 생성을 수행했다. old/new 회원 ID가 다르고 owner/account-shared 두 연결이 1008이며 unrelated Root shell은 살아 있음을 확인했다. 새 ID로 old session 목록·join/일반 reconnect·이름/공유/timeout/keepalive·share-link 발급·scrollback·종료가 거부되고 old public link도 무효였다. metadata에는 같은 표시 이름에 distinct owner IDs가 저장됐고 old shell 자체는 그대로 남았다.

fixture operator가 별도의 정상 session에서 owner_id만 제거하고 실제 서버를 재시작했다. current ID의 원래 session attach는 복원됐고 deleted ID 및 missing-ID legacy는 이름으로 채택하지 않았다. legacy shell·metadata도 유지됐다. 이는 테스트용 offline legacy 전환이며 제품에 rename·자동 재배정 UI를 추가하거나 운영 데이터를 바꾼 것이 아니다. 최신 빌드의 `smoke-membership.py` 회원가입·최고관리자 승인·server/tenant 격리·위임 관리·role/trust/scope 정책·session revoke, `smoke-remote-binding.py` 실제 OpenSSH·tmux·loopback 승인/회수/재시작, `smoke-terminal-tickets.py`, `smoke-shared-revocation.py`, `smoke-terminal-registry.py`의 실제 UI·입장·SSE/download·저장/용량/EOF/환경설정 장애·복구 회귀도 통과했다.

CI에 `terminal-access-smoke` job을 추가해 Chromium·tmux와 실제 fixture server를 준비하고 ownership·registry·account tickets·shared revocation을 순차 실행하도록 했다. backend-test에도 실제 tmux/OpenSSH unit 의존성을 설치한다. YAML을 파싱하고 같은 smoke 명령의 로컬 실행을 확인했으나 원격 workflow는 실행하지 않았다. 새 browser 설치는 [Playwright 공식 CI 지침](https://playwright.dev/docs/ci)을 참고했다. 기존 frontend job의 400행 규칙은 유지했으며 현재 작업 트리에서 **26개 파일이 초과**함을 확인했다. 원격 CI 전체 통과나 release 준비 완료를 주장하지 않는다. 해당 구조 분리·CI 검증이 후속 작업이다.

[OWASP의 user ID/username 구분](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html#user-ids)을 확인해 나비맨의 기존 random local ID에 session ownership을 연결했다. 경쟁 제품 실측 또는 인증 완료가 아니다. [계정 입장](terminal-admission.md)·[공유 입장](shared-terminal-access.md)·[저장/legacy 이전](terminal-registry.md) 계약을 갱신했다. orphan의 명시적 승인 복구/재배정·독립 감사, 일반 JWT/SID 모델 전체 ID 이전, settings/cache 계정 ID 이전, JIT·OS identity·분산/backup 복구 인가와 규모는 남아 있다. 운영 배포·실제 원격 장비 변경은 하지 않았으며 C03·C09·C10·C11과 전체 상용화 목표를 완료 처리하지 않는다.

## 2026년 10월 5일 책임별 소스 분리와 CI 400행 규칙 복구

이전 기록에서 확인한 400행 초과 파일 26개를 책임별 모듈로 분리했다. 인증 bootstrap·암호 저장, 원격 서버 작업, 암호화 backup, vhost 설정, IP 차단 parser, terminal SSH command·PTY spawn·tmux 작업·환경설정 handler·share 발급/입장, 회원 추가·membership 조정·관리 grant/association/inventory·위임 저장과 durable intent를 분리했다. 부모 모듈의 HTTP handler 이름과 route, 저장 format·원자 발행·회수 정책은 유지한다. 소스 include나 CI 예외로 규칙을 우회하지 않았다.

Rust 단위 시험은 이름을 유지한 별도 module로 옮겼다. 실제 provider를 사용하는 큰 membership 시험은 HTTP fixture·설정·managed/delete 검증으로 나눴으며 기존 30개 grant 시나리오와 28개 scoped revoke 시나리오를 그대로 실행한다. 실제 HTTP dispatch와 DB·provider write·회수/재조정 assertion도 유지한다. provider thread guard는 각 시나리오가 끝날 때까지 살아 있다.

frontend TerminalView의 공개 type은 별도 파일에 두고 기존 import 경로에서 re-export한다. terminal 이력 controls·non-passive wheel handler와 외부 pane 조작 hook을 분리했다. reconnect·ticket·권한 회수 처리는 기존 pane에 유지하며 imperative handle의 ref 의존성을 명시했다.

검증: backend unit **347건**·auth-flow **5건**·integration **28건**, 총 **380건**, frontend **34개 suite·136건** 통과. backend debug/release build·Rust test compile·fmt, frontend typecheck·build, diff 검사 통과. backend compile 경고는 없고 lint는 오류 0건·기존 경고 7건이다. 기존 CI의 동일한 find/wc/awk 명령으로 backend/src·frontend/src의 모든 Rust/TypeScript source **455개**를 검사해 **400행 초과 0개**를 확인했다. 파일 이동 때문에 시나리오를 줄이거나 assertion을 생략하지 않았다.

최신 debug binary와 새 UI build의 `smoke-membership.py`, `smoke-terminal-owner.py`, `smoke-remote-binding.py`, `smoke-commercial-integrity.py`, `smoke-terminal-registry.py`, `smoke-terminal-tickets.py`, `smoke-shared-revocation.py`, `smoke-user-publication.py`가 통과했다. 임시 auth/DB·고유 tmux namespace의 별도 fixture로 실제 `smoke-terminal-history.js`도 실행해 wheel·track click·영역 밖 drag·keyboard End·live shell input과 reconnect 없는 redraw/이력 위치 유지를 확인했다. 운영 서버·실제 원격 장비·운영 tmux session을 변경하지 않았다.

같은 최신 release binary에서도 `smoke-commercial-integrity.py`의 실제 startup·손상 저장·오류 connector 거부와 `smoke-user-publication.py`의 directory fsync fault·exit 78·회수된 권한 및 새 generation 보존을 통과했다.

원격 GitHub workflow는 실행하지 않았다. 로컬 검사 결과를 원격 CI 전체 통과나 상용 출시 완료로 해석하지 않는다. 이 단계는 유지보수 구조와 이미 구현한 동작의 회귀 검증이며, 특권 분리·독립 감사·HA·실측 벤치마킹·상품 한도 집행·고객 pilot 등 상용화 잔여 범위는 유지한다.

## 2026년 10월 5일 서비스 제어의 제한된 특권 경계와 로그인 전환

`/api/services/action`을 비특권 API와 별도 `nabiman-privileged-helper`로 분리했다. helper는 root가 승인한 정확한 service/action만 받으며 shell·프로그램·임의 argv를 protocol에 넣지 않는다. 양쪽 kernel UID 검사와 root supervisor가 발행한 API PID/start ticks/boot ID pin으로 같은 UID의 다른 process를 거부한다. API process의 dumpability도 해제한다. API가 여전히 root인 기존 설치에서는 변경을 503으로 거부하므로, 전체 legacy 기능·비특권 설치·데이터 이전이 완료되기 전 이 작업 트리를 상용 upgrade로 배포하지 않는다.

Root 전용 SQLite ledger에 policy snapshot/hash와 실행 intent를 먼저 commit한다. DELETE/EXTRA와 private single writer를 검사하고, 동일 UUID 재생과 결과 불명 unit의 새 key 변경을 차단한다. intent 저장 실패는 실행 전 중단하며 결과 저장 실패는 성공 응답 없이 helper를 중단한다. 재시작은 running을 unknown으로 이전한다. 현재 계정의 기록만 조회하며 systemctl exit 0을 workload 건강 증명으로 표시하지 않는다. UI는 제출 전 ID 저장, 중복 클릭 방지, 실패/결과 불명 구분, 응답 유실·reload 뒤 read-only 조회와 inventory 오류 중에도 복구 경로를 제공한다. [상세 계약과 한계](privileged-service-control.md)를 추가했다.

실제 non-root API/root helper/Unix socket/systemctl/SQLite/browser fixture가 debug와 release binary에서 통과했다. 대상은 존재하지 않는 고유 fixture unit이며 실제 workload를 변경한 검증은 아니다. root UID·unsafe policy·손상/유실 ledger·중복 writer·잘못된 frame·잘못된 peer·허용되지 않은 unit/action·중복 ID·다른 actor의 기록 접근을 거부했다. 실제 SQL insert/update fault, helper 중단과 재시작, 결과 불명 상태의 새 key 차단을 확인했다. SQLite shared reader로 intent commit을 막은 사이 root가 승인 process pin을 바꾸면 `not_dispatched`를 기록하고 실행하지 않았다. 최초 `/proc/PID` 디렉터리 소유권 assertion 실패는 Linux 명세와 별도 실제 process 실험으로 오류를 확인하고 `/proc/PID/mem` 소유권 및 실제 open 거부 검사로 수정했다.

추가 검토에서 공통 refresh client가 이전 요청의 401을 새 계정의 credential로 갱신/재시도할 수 있는 race를 발견했다. login generation과 refresh credential에 갱신 작업을 묶고, 늦은 응답이 새 token을 덮어쓰거나 새 계정을 logout하거나 변경을 재생하는 경로를 차단했다. 같은 session의 갱신은 공유하고 retry는 한 번이며 원래 method/body/idempotency key를 유지한다. JSON 완료 후에도 확인하고 서비스 화면은 계정 전환/unmount 뒤 오래된 receipt를 버린다. [계정 입장 문서](terminal-admission.md)에 범위와 raw stream·외부 adapter·서버 identity의 잔여 작업을 기록했다.

신규 refresh unit 11건과 서비스 화면의 계정 전환 시험이 통과했다. 실제 Chromium에서 별도 최고관리자 session의 서비스 401/refresh 성공/refresh 실패를 지연시키고 logout 뒤 viewer로 로그인했다. 세 경우 모두 viewer credential과 실제 `/api/users/me` identity를 유지하며 변경 POST는 한 건이고 helper ledger에 추가 명령이 없었다. 이는 응답 처리의 격리이며 이미 실행한 서버 작업을 취소하거나 rollback했다는 뜻은 아니다.

검증: backend helper unit **14건**·서버 unit **347건**·auth-flow **5건**·integration **28건**, 총 **394건** 통과. frontend **36개 suite·152건**, typecheck·build·lint 오류 0건/기존 경고 7건 통과. Rust debug/release build·fmt·diff와 CI YAML 파싱도 통과했다. Rust/TypeScript source **473개**에서 400행 초과는 0개다. 회원가입·최고관리자 승인·scoped 관리/권한/회수, terminal owner/registry/SSH binding/ticket/shared, 사용자 저장 fault fixture의 회귀를 확인했다. archive는 두 binary와 검토용 packaging template을 포함하며 checksum과 실제 UI/assets/login/protected API smoke를 통과했다. CI의 기존 terminal-access job에 helper와 browser race fixture를 연결했지만 원격 workflow를 실행한 것은 아니다.

공통 client 변경 후 membership·ticket smoke는 통과했으나 shared browser의 첫 guest 화면 로딩은 UI rebuild와 겹친 실행에서 timeout이 났다. 관측된 시간 중첩만으로 제품 원인을 단정하지 않았다. shared fixture가 완성된 frontend build를 임시 static directory에 복사해 모든 guest와 서버 재시작에서 같은 snapshot을 제공하도록 바꾸고, 새 build가 완료된 뒤 재실행해 password·일회용 접속·expiry·회수·owner generation·재시작 시험을 모두 통과했다. 마지막 client guard까지 포함한 build로 release helper/browser race와 archive smoke도 통과했다. 실제 운영의 원자적 static rollout·이전 asset 유지 검증은 C02/C16의 잔여 범위다.

이 단계는 C05 일부와 client session 경계의 개발 구현이다. 현재 PID 1은 systemd가 아닌 supervisor이며 실제 systemd unit sandbox·지원 OS 설치 matrix를 검증하지 않았다. helper의 정상 workload 변경·서비스 건강·unit 참조 파일의 승인/변경 감지, local terminal의 별도 OS identity, 다른 legacy root endpoint·installer/Docker 이전, independent audit·승인·복구 실행·HA와 고객 pilot은 남아 있다. 운영 배포·실제 서버 서비스 변경·GitHub release 발행은 하지 않았다. 전체 목표를 완료 처리하지 않는다.

## 2026년 10월 5일 23:48 KST 운영 배포와 실제 접속 확인

사용자가 배포까지 완료하도록 명시적으로 요청해 검증한 backend와 frontend를 **https://man.bebo.kr/**에 반영했다. 대상은 supervisor의 `nabiman` program 한 개이며 다른 service를 재시작하지 않았다. 배포 ID는 `commercial-foundation-20261005-v1`이다. 현재 release binary SHA256은 `6ee77a972c91e06753d753c851ebe404ed23a689186128d83571a60cc2d0c7ca`, 당시 소스 archive SHA256은 `7c1876b1787013c3d103cb54941af0925672026860ce19cc80ca7f9432893042`다. GitHub release/tag 발행과 상용 GA 완료는 별개다.

운영 auth 파일·원자 SQLite backup·binary·static·실행 wrapper·tmux 목록을 root 전용 배포 directory에 보관했다. `scripts/deploy-reviewed-runtime.py`는 준비 → 명시적 plan digest 검토 → 운영 데이터 복사본 rehearsal → 전환 → HTTP/HTTPS 확인 순서로 실행하며 전환 실패 시 backup으로 복구하도록 구성했다. rollback 데이터는 `/workspace/nabiman/deployments/commercial-foundation-20261005-v1/cutover.before`이고 최종 증거는 같은 배포 directory의 `manifest.json`, `browser-verification.json`, `ssh-verification.json`, `evidence/`에 있다. backup을 준비한 것이 전체 고객 disaster-recovery drill 완료를 뜻하지 않는다.

기존 회원은 admin 1명이고 과거 배포 backup과 현재의 불변 ID·계정 생성 시각이 같음을 확인했다. terminal 16개는 이 ID, 등록 tmux 이름, SSH command의 정확한 target/port, 원격 서버 ID를 검토한 계획으로 이전했다. runtime의 이름 기반 자동 adoption을 허용하지 않았다. 서버 6개는 root known_hosts의 기존 키를 사용했다. 5개 host의 keyscan과 syc의 StrictHostKeyChecking handshake가 기존 키와 일치했다. `host_trust`, 승인 version, terminal의 exact target binding을 함께 이전했으며 암호화 password를 변경하지 않았다. 기존 공유 링크 9개는 이미 모두 만료돼 있었고 그대로 보존했다.

새 바이너리를 별도 port·auth/DB 복사본·독립 tmux namespace에서 먼저 실행했다. 첫 rehearsal은 검증 script의 잘못된 remote API 경로 때문에 실패했다. 실제 route로 수정한 뒤 회원/소속·원격 신뢰·16개 terminal의 current ownership과 alive 상태를 확인했다. 이 단계의 shell은 복사 metadata에 대응하는 별도 fixture shell이며 운영 shell에 명령을 보내지 않았다. 마지막 전환에서는 새 backup을 받고 identity/target의 변경 여부를 재검사한 뒤 binary를 원자 교체하고 version directory의 static으로 연결했다. 기존 hash asset도 남겨 이미 열린 페이지의 chunk 로딩을 보호했다.

전환 후 로컬과 공개 HTTPS에서 실제 기존 관리자 로그인, 현재 회원·회원 목록·소속 설정·등록 서버·terminal 목록, 비인증 회원 조회의 401을 확인했다. 공개 HTML과 5개 JS/CSS asset의 SHA256이 검토한 후보와 일치했다. 실제 Chromium에서 로그인 화면의 가입 신청 버튼, `/signup` 직접 접근/새로고침, 입력의 left 정렬, 로그아웃 상태의 terminal UI/요청 부재, 최고관리자 회원 화면과 tenant·서버 설정을 검증했다. 별도 임시 terminal 하나에서 무해한 printf를 입력하고 실제 출력이 도착했으며 해당 probe session만 API로 삭제했다. 브라우저 page error는 0건이었다.

`syc.insnetkorea.com`에 새 host trust 경로와 기존 저장 credential로 실제 SSH 인증·printf 확인 명령이 성공했다. 기존 Root tmux session은 전환 전/후 **17개**, 유실 **0개**이며 앱에 등록된 **16개** 모두 alive로 조회됐다. 기존 회원 ID와 password hash를 보존했고 SQLite `quick_check`는 ok였다. 재로그인은 필요하지만 기존 tmux 작업을 중단하거나 새 shell로 대체하지 않았다.

현재 호스트는 PID 1이 supervisor이며 기존 systemd inventory도 동작하지 않는 환경임을 사전에 확인했다. 따라서 helper binary는 준비하되 daemon으로 활성화하지 않았다. root API의 `/api/services/action` 거부를 유지하며 일반 Root API/local shell과 다른 legacy 관리 기능의 OS 권한 분리는 후속 C05 작업이다. backend의 자동 mock 전환 차단도 반영됐으므로 미설정 MEC connector는 실제 연결 불가로 처리한다. 이 배포는 실제 운영 반영·한 호스트의 이전과 접속 확인이며 전체 설치 matrix·독립 보안·HA·상품 한도·상용 GA를 완료 처리하지 않는다.

## 다음 실행 항목

- 소스 400행 규칙의 로컬 통과를 바탕으로 원격 CI 전체와 신규 terminal smoke gate를 실제 runner에서 검증한다.

- C02 잔여: UI의 source·신선도·오류 상태 구분, 정확한 release와 운영 rollout 검증.
- C03 잔여: trust version과 진행 중 접속 회수, 교체 감사·승인 절차, 전체 legacy 예외 관리, 실제 장비 rollout 검증.
- C10 잔여: durable queue·worker lease·fencing·승인·실행 직전 재인가·재시작 복구·UnknownResult·partial recovery와 응답 유실 시 자원 확인·관리자 재조정과 승인된 복구 실행.
- C05·C06·C07: 특권 프로세스 분리, transaction 저장, 조직과 실제 자원의 중앙 인가 이전.
- C05 다음: local terminal과 API의 OS identity/credential 분리, legacy root 실행 경로별 typed operation 이전, unit 참조 파일의 승인·변경 감지와 정상 workload 검증, systemd sandbox·installer/Docker·기존 데이터 이전 시험. 현재 service helper 일부 구현만으로 이 gate를 닫지 않는다.
- C04 잔여: 실제 feature·한도 집행, 발급 운영·rotation·고객 이전·계약·갱신·취소·clock 신뢰와 signed installer.
- C09 잔여: 소유자 version의 분산 전달·백업 복구 인가, proxy redaction·WSS rollout·OS identity·JIT/승인/MFA·recording·다중 replica·규모 검증.
- C11·C15·C16: 독립 감사, 실제 복구, signed release와 공급망 검증.

회원·tenant 기능 확대, UX·국제화·접근성, 관측성·연동, 상품·지원, HA·offline·규모·AI 평가도 원래 범위에 유지한다. 개별 시험 통과만으로 전체 상용화 목표를 완료 처리하지 않는다.

### 2026-10-06 — 실행 버전 식별과 반복 배포 검증

- 인증된 진단 API와 화면에 실제 실행 파일의 SHA-256, 시작 시각, 인스턴스 ID를 추가했다. 시작 시 `/proc/self/exe`를 읽으므로 설치 경로를 교체해도 기존 프로세스를 새 바이너리로 오인하지 않는다. 응답은 `Cache-Control: no-store`이며 갱신 실패 시 화면에서 이전 지문을 숨긴다.
- 배포 도구의 `preserve` 경로는 승인된 터미널 소유자 ID, SSH 서버 신뢰 키와 승인 버전을 다시 부여하지 않는다. 검토 이후 대상·소유자·승인 버전이 변경되면 중단한다. 현재 검증기는 기존 단일 최고관리자가 모든 등록 터미널을 소유한 이 설치에 한정되며, 다중 소유자 설치를 자동 이전하는 범용 도구가 아니다.
- 배포 리허설은 운영 데이터 복사본과 별도 tmux namespace에서 실행한다. 활성화 후 내부 HTTP와 외부 HTTPS의 실행 지문·인스턴스가 일치해야 한다. 실제 브라우저 증거와 회원 권한·서버 신뢰·기존 터미널 보존 검증을 배포 디렉터리에 저장한다.
- 이 기능은 런타임 식별이며, 독립적인 원격 attestation, 서명된 provenance, SBOM, 전체 C16 완료를 의미하지 않는다. 운영 활성화 결과는 해당 배포의 activated.json 및 browser-verification.json으로 확인한다.

운영 반영 완료: 2026-10-06 00:28 KST, 배포 디렉터리 `/workspace/nabiman/deployments/runtime-identity-20261006-v1`.
운영 바이너리 SHA-256은 `3420bf1ac9f8714d4ab363163eeded5f558afb709747aa83f47cae12484a7abb`, 배포 시점 소스 아카이브는 `4c648657f52625320fef08fa3664386c846f713b796c22b0c6f7412936499a2a`이다.
내부 HTTP·외부 HTTPS가 동일한 실행 인스턴스를 보고했으며, 공개 화면의 실행 지문과 배포 후보가 일치했다.
실제 브라우저에서 가입 신청 왼쪽 정렬, 로그아웃 시 터미널 UI·요청 없음, 회원/소속 관리, 버전 표시·갱신, 별도 검증용 터미널 입출력·삭제를 확인했다. 브라우저 오류는 0건이다.
기존 tmux 17개, 등록 세션 16개, 회원 권한·비밀번호 해시, 서버 6개의 신뢰 키·승인 버전을 보존했다. SQLite quick_check도 통과했다.
검증: 백엔드 394개, 프런트엔드 기존 152개 및 신규 오류 표시 1개, 배포 보존 3개, debug/release 실제 프로세스 교체·재시작, 릴리스 패키지 HTTP 시험 통과. 프런트엔드 타입 검사·빌드 통과, lint 오류 0개·기존 경고 7개.
최종 증거는 배포 디렉터리의 `manifest.json`, `activated.json`, `browser-verification.json`, `evidence/`에 저장했다. 이전 바이너리·정적 파일·데이터 백업은 `cutover.before/`에 보존했다. root API 분리, systemd helper 실제 운영, signed release 등 기존 상용화 잔여 범위는 유지한다.

### 2026-10-06 — 회원 ID 기반 로그인 세션과 본인 세션 관리

[OWASP Session Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)의 서버 측 세션 무효화와 계정·권한 연결 원칙을 대조했다. 이번 작업은 해당 원칙의 일부 구현이며 인증 전체 적합성 판정은 아니다.

- 로그인 세션 저장소에 변경 불가능한 회원 ID를 연결했다. JWT의 표시용 사용자 이름이 같더라도 세션에 연결된 ID와 현재 회원 ID가 다르면 일반 요청·토큰 갱신을 거부한다. 연결 유지 검사와 upstream I/O 이후 재인가도 검증된 회원 ID를 사용한다.
- 비밀번호·MFA·OAuth·LDAP 로그인은 인증 당시 회원 상태와 등록 직전 상태를 비교하고, 회원 잠금을 유지한 채 세션을 등록한다. 삭제·재가입, 비밀번호·MFA·권한 변경 이후 늦게 완료된 인증 결과가 새 세션을 발급하는 것을 막는다.
- 본인 비밀번호/MFA 수정과 회원 관리의 호출자 판정도 검증된 ID에 연결했다. 비밀번호 변경은 검증한 비밀번호 해시가 아직 현재 값인지 쓰기 직전에 확인한다.
- 최고관리자/소속 관리의 로그인 세션 화면을 추가했다. 일반 회원과 tenant 관리자는 자기 계정 세션만 조회·종료하고, 최고관리자는 전체를 관리한다. 타인 세션과 없는 세션의 종료 요청은 동일한 404를 반환한다. 본인의 다른 로그인 일괄 종료는 현재 로그인 및 다른 회원을 유지한다.
- API는 민감한 목록에 no-store를 적용한다. 최근 시각은 로그인·토큰 갱신 시각으로 표시하며 실제 매 요청 활동 시각인 것처럼 표현하지 않는다. 세션 ID는 로그인 권한을 대신하는 bearer credential이 아니다.
- 새 메뉴는 OS 로그인 세션 관리와 별개이며, 로그인 종료는 서버 tmux 작업을 죽이지 않는다. 기존 로그인 저장소는 메모리 기반이므로 서버 재시작 시 재로그인이 필요하다. 분산 세션 저장, refresh rotation/reuse detection, MFA 정책·복구 절차, 독립 감사는 후속 범위로 유지한다.
- 실행 파일 지문 도입 후 큰 debug 실행 파일을 해싱하는 초기화 지연을 줄이기 위해 개발 빌드의 SHA-256 압축도 최적화했다. 운영 release 최적화 설정은 유지한다.

추가 보강: 서명·세션이 유효한 refresh JWT라도 일반 API의 access credential로 사용할 수 없도록 토큰 용도를 검사한다. 기존 access JWT 형식과 정상 refresh API는 유지한다. 만료 시간이 긴 갱신 토큰으로 access 토큰의 짧은 만료 정책을 우회하는 경로를 차단하는 회귀 테스트를 추가했다.
토큰 용도 분리의 검토 근거는 [RFC 8725 §3.12](https://www.rfc-editor.org/rfc/rfc8725.html#section-3.12)이며, 서로 다른 JWT 종류에 배타적인 검증 규칙을 적용하는 부분을 이번 수정에 반영했다. issuer/audience·키 분리·회전 등을 포함한 RFC 전체 준수 여부는 별도 검증 범위다.

운영 배포 완료: 2026-10-06 00:50 KST, `/workspace/nabiman/deployments/account-sessions-20261006-v1`.
바이너리 SHA-256 `5ff0f58e3875185000574be3db381609ab953fd55644fdf4e6f879fa17844e95`, 소스 아카이브 SHA-256 `5e7f6ccf0f85a8e915c6917bcc9730db6f534c9c45355843c11f000e8bcf703f`.
백엔드 398개, 프런트엔드 기존 153개와 신규 오류 표시 1개, 타입 검사·빌드 통과. lint 오류 0개·기존 경고 7개.
격리한 실제 debug/release 서버와 브라우저에서 본인 세션 목록, 다른 로그인 일괄 종료, 타인 세션 종료 거부, 삭제·재가입 후 이전 access/refresh 차단, refresh JWT의 일반 API 사용 차단과 정상 갱신을 확인했다. 기존 회원 승인·소속 격리·서버 신뢰·터미널 소유권 및 logout 후 WebSocket 1008/SSE EOF 시험도 통과했다.
운영 HTTPS에서 새 메뉴를 열고 별도로 생성한 검증용 로그인만 UI로 종료한 뒤 access와 refresh가 모두 401인 것을 확인했다. 갱신 토큰을 Authorization에 넣은 일반 요청도 401이었다. 가입 신청·회원/소속 관리·실행 버전 표시·실제 터미널 입출력 검증 통과, 브라우저 오류 0건.
회원 ID·권한·비밀번호 해시, 서버 6개의 SSH 신뢰 키·승인 버전, 등록 터미널 16개와 root tmux 17개 보존, SQLite quick_check 통과. 백업과 증거는 배포 디렉터리의 cutover.before/, manifest.json, browser-verification.json, evidence/에 보존했다. 전체 상용화 목표는 진행 중이다.

### 2026-10-06 — 갱신 토큰 교체와 재사용 차단

검토 근거: [RFC 9700 §4.14.2](https://www.rfc-editor.org/rfc/rfc9700.html#section-4.14.2), [Auth0 Refresh Token Rotation](https://auth0.com/docs/secure/tokens/refresh-tokens/refresh-token-rotation). Auth0 문서의 매 교환 시 새 토큰 발급·이전 토큰 재사용 시 token family 무효화 방식을 비교했다. 제품 전체 동등성이나 OAuth 서버 적합성을 주장하지 않는다.

- 로컬 로그인 refresh JWT에 세대를 넣고 서버 세션의 현재 세대와 대조한다. 회원→세션 잠금 순서를 유지한 상태로 검증·발급·세대 증가를 수행한다. 동일 토큰의 동시 교환은 한 번만 성공하고, 재사용을 감지하면 그 로그인 세션을 제거한다. 다른 로그인은 유지한다.
- 교체된 refresh 토큰도 최초 만료 시각을 유지한다. 만료 허용 오차로 7일을 연장하지 않으며, 실패한 서명의 토큰으로 정상 세션을 폐기할 수 없다. 재사용 후 access·refresh 요청과 해당 로그인으로 열린 연결을 거부한다.
- 프런트엔드는 교체된 access/refresh를 함께 저장하고 로그인 교체와 토큰 교체를 구분한다. 같은 페이지의 동시 갱신은 하나를 공유하며, 지연된 401이 이미 소비한 토큰을 다시 쓰지 않는다. access 문자열이 같은 초 안에서 동일해도 갱신 세대로 판정한다. 로그아웃·다른 계정 로그인 후 늦게 온 응답은 반영하지 않는다.
- 갱신 실패 시 자격 증명을 제거하고 로그인 화면에 재로그인 안내를 표시한다. 원본 refresh 토큰을 자동 재시도하거나 재사용을 허용하는 유예 창은 두지 않는다. 응답 유실이나 복제된 탭의 동일 자격 증명 동시 사용도 재로그인이 필요할 수 있다. 탭 간 조정, sender constraint, 분산 세션 저장, 재사용 전용 독립 감사 이벤트는 후속 범위다.
- 백엔드 400개·프런트엔드 156개, 타입·빌드 검사 통과. 실제 debug/release 서버에서 브라우저 토큰 교체·다음 갱신·재로그인 안내, 동시 교환 한 건 성공 후 family 폐기, 다른 로그인 유지, 연결의 WebSocket 1008 종료와 tmux 작업 보존을 검증했다. 새 테스트가 초기 빈 문서에 자격 증명을 주입하지 않도록 origin을 제한했다.

운영 배포 완료: 2026-10-06 01:07 KST, `/workspace/nabiman/deployments/refresh-rotation-20261006-v1`.
바이너리 SHA-256 `daa3f57f556246fee935ec4880cc66372a2211b7ce76765f1b74f772b22c2347`, 소스 아카이브 SHA-256 `d39ef66c3d6e3aab1c631b45868f4ad2bdc74e600edbb9391621b2bf55b11981`.
실제 helper·계정 전환 검증도 통과했다. 지연된 갱신 성공/실패 응답이 새 로그인 자격 증명을 덮거나 작업을 재전송하지 않았다.
운영 HTTPS에서 별도로 만든 검증용 로그인 토큰의 교체와 이전 토큰 재사용 후 access/refresh 모두 401을 확인했다. 별도 로그인은 200을 유지했다. 검증용 로그인만 대상으로 실행했으며 증거에는 토큰을 저장하지 않았다.
가입 신청·회원/소속 관리·로그인 종료·실행 지문 표시·터미널 입출력 브라우저 검증 통과, 브라우저 오류 0건. 기존 회원 ID·권한·비밀번호 해시, 서버 6개의 신뢰 정보, 등록 터미널 16개와 tmux 17개 보존, SQLite quick_check 통과.
백업·증거는 cutover.before/, manifest.json, browser-verification.json, evidence/에 보존했다. 타입·빌드·lint 오류 없음(기존 lint 경고 7개). 전체 상용화 목표와 기존 미완료 범위는 유지한다.

### 2026-10-06 — 로컬 로그인 MFA 등록·복구·해제

[OWASP Multifactor Authentication Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html)의 factor 변경 시 재인증, 복구 수단, OTP 수명·일회성 원칙을 대조했다. 인증 앱 설정 화면이 없던 기존 API를 실제 회원용 흐름으로 연결했다.

- 최고관리자와 소속 회원 모두 자기 계정의 MFA 상태를 조회하고 설정한다. 등록에는 현재 비밀번호를 요구하며, QR/설정 키와 128-bit 무작위 복구 코드 10개를 한 번 표시한다. 클라이언트 저장소에는 기록하지 않고 사용자가 직접 파일로 보관한다. 등록 대기는 10분이며 저장 확인과 앱의 인증 번호가 있어야 활성화한다.
- 복구 코드는 SHA-256 지문만 기존 private 회원 파일에 저장한다. 인증 앱 키·복구 해시·사용한 시간 단계는 회원 API에서 제외한다. 민감한 설정·코드 응답은 no-store다. secret 자체는 기존 private 회원 파일에 저장되므로 외부 KMS/HSM 보호를 달성한 것은 아니다.
- TOTP 시간 단계와 복구 코드 사용을 회원 저장 잠금 아래 먼저 영속화하고, 성공한 경우에만 로그인 세션을 발급한다. 같은 코드의 재사용과 재시작 후 재사용을 거부한다. 저장 실패 시 사용 상태를 복원하고 토큰을 발급하지 않는다.
- 등록·해제 시 해당 회원의 모든 로그인을 종료한다. 해제와 복구 코드 재발급에는 비밀번호 및 유효한 인증 번호/복구 코드가 필요하다. 재발급은 이전 코드를 전부 폐기하고 다른 로그인을 종료하며, 현재 로그인은 새 코드를 저장할 수 있도록 유지한다.
- 비밀번호/MFA 입력 경로의 기존 시도 제한을 설정·확인·해제·재발급에도 적용했다. 현재 제한은 프로세스·peer IP 기준이며 신뢰 proxy 처리, 계정별·분산 제한은 별도 후속 범위다.
- 기존 회원은 기본값이 비어 있는 private mfa 상태로 읽으며 ID·권한·비밀번호·기존 TOTP 설정을 보존한다. 배포 검증은 이 명시적인 기본값만 정규화하고, 이미 저장된 MFA 상태는 그대로 비교한다. 운영 관리자의 MFA는 자동으로 켜거나 끄지 않는다.
- API: GET /api/auth/2fa/status; POST setup {password}; POST verify-setup {code,recovery_saved}; POST disable 및 recovery-codes {password,code}; 기존 verify 로그인은 {username,password,code}에서 앱 번호 또는 복구 코드를 받는다.
- 범위: 로컬 계정의 선택적 TOTP MFA다. WebAuthn/passkey, tenant별 강제 정책, 외부 IdP MFA 검증, MFA 분실 시 관리자의 별도 신원 확인·복구 승인, 독립 감사와 KMS는 남아 있다. 인증 앱 OTP는 phishing-resistant MFA로 주장하지 않는다.

운영 배포 완료: 2026-10-06 01:33 KST, `/workspace/nabiman/deployments/mfa-enrollment-20261006-v1`.
바이너리 SHA-256 `8f2430f8a85405effc64d92addc4a8f49436af4c56ad4e3db12dc476cc5168ab`, 소스 아카이브 SHA-256 `afe33c981d1dde72be20d4319640d345f3c53c4fc16e12a0ff10990136b6a6a9`.
백엔드 402개·프런트엔드 156개, 타입·빌드·배포 패키지 검증 통과. lint 오류 0개·기존 경고 7개.
격리한 실제 debug/release 서버와 브라우저에서 소속 회원 QR 등록, 복구 코드 파일 다운로드, 저장 확인, 로그인 종료, 일회성 코드 사용, 서버 재시작 후 재사용 거부, 코드 재발급과 인증 해제, 잘못된 비밀번호 입력 제한을 확인했다. 회원·갱신 토큰·privileged helper 회귀 검증도 통과했다.
운영 HTTPS에서 2단계 인증 화면과 상태를 읽기 전용으로 확인했다. 운영 관리자 MFA는 변경하지 않았다. 가입 신청 왼쪽 정렬·로그아웃 화면 분리·회원/소속 관리·로그인 종료·실행 지문·실제 터미널 입출력·갱신 토큰 교체 및 재사용 차단 모두 통과했고 브라우저 오류는 0건이다.
기존 회원 ID·권한·비밀번호 해시·MFA 설정과 서버 6개의 SSH 신뢰 정보, 등록 터미널 16개와 tmux 17개를 보존했다. 기존 회원 직렬화에 추가된 빈 private mfa 기본값만 명시적으로 정규화해 비교했다. SQLite quick_check 통과, supervisor 정상 실행을 확인했다.
배포 디렉터리에 cutover.before/, manifest.json, browser-verification.json, evidence/를 보존했다. 실제 실행 파일 지문과 공개 HTTPS가 제공한 지문이 일치한다. root API 분리, tenant별 MFA 강제 정책, passkey, 독립 감사·HA/DR 등을 포함한 전체 상용화 목표는 계속 진행 중이다.

### 2026-10-06 — tenant·서버별 MFA 필수 정책

벤치마킹 근거: [Teleport per-session MFA](https://goteleport.com/docs/zero-trust-access/authentication/per-session-mfa/)는 로그인 MFA와 자원 접근 시 MFA를 구분한다. [authentik authenticator validation](https://github.com/goauthentik/authentik/blob/main/website/docs/add-secure-apps/flows-stages/stages/authenticator_validate/index.md)은 미등록 사용자에게 등록 단계를 연결한다. 이번 구현은 이 두 구분을 참고한 로컬 로그인 보안 정책이며, Teleport의 명령별 재인증·하드웨어 키 보증과 동등하다고 주장하지 않는다.

- 소속 운영 설정에 require_mfa를 추가했다. 기존 파일은 false로 읽으며, 이 필드가 없는 기존 클라이언트의 저장 요청은 현재 MFA 정책을 유지한다. 명시적 false만 필수 정책을 해제한다.
- 활성·미만료 소속 중 하나라도 필수이면 해당 회원의 서비스 접근 전체에 적용한다. 소속별 정책을 섞어 우회하지 못하게 한다. 비활성·만료 소속은 요구 조건에 포함하지 않는다. 최고관리자는 명시적 복구 권한으로 예외이며, 소속 관리자가 최고관리자를 만들 수는 없다.
- 회원의 totp_enabled와 로그인 시 실제로 factor를 확인했다는 서버 세션의 mfa_verified를 별도로 검사한다. OAuth/LDAP 및 비밀번호만으로 발급한 세션은 이 검증 사실을 갖지 않는다. 토큰 갱신은 기존 세션의 검증 사실을 보존하며 새 검증 사실을 만들지 않는다.
- 미등록/미검증 회원은 본인 정보·MFA 등록·비밀번호 변경·본인 로그인 종료 등 정해진 복구 경로만 사용할 수 있다. 기존 로그인도 다음 보호 API 요청에서 제한하며, 연결 권한의 주기적 검사와 입력 검사에서도 정책을 다시 평가한다. 이미 시작한 모든 외부 작업을 소급 취소한다는 의미는 아니다.
- 화면은 정책 차단 시 인증 등록으로 안내한다. 정책 변경으로 받은 403도 본인 상태를 다시 읽어 안내 화면으로 전환한다. 이전 계정의 지연 응답은 새 계정 화면을 전환하지 않는다. 백엔드 권한 검사가 실제 접근을 강제한다.
- 소속 관리자는 자신의 MFA 등록과 재로그인을 마친 뒤 정책을 활성화할 수 있다. 저장 시 현재 회원 ID·관리 권한·MFA 조건을 다시 확인한다. 필수 정책 아래에서는 MFA 해제를 거부하고 이미 소비하려던 코드도 되돌린다.
- 설정 파일을 해석하지 못하면 소속 회원의 보호 접근을 거부하고 상태 조회에서 503을 반환한다. 로그아웃 등 복구 경로와 최고관리자의 접근은 남긴다. 없는 파일의 초기 기본값은 선택적 MFA다. OS 파일 삭제를 독립적으로 탐지하는 저장소 무결성 보증은 후속 범위다.
- 외부 SSO·LDAP MFA assurance는 아직 검증하지 않는다. 따라서 필수 정책 대상 외부 계정은 접근할 수 없음을 설정 화면과 차단 화면에 명시했다. 외부 MFA를 검증한 것처럼 처리하지 않는다. WebAuthn/passkey, 재인증 신선도, 독립 정책 변경 감사·승인, 전역 최고관리자 강제 정책과 별도 break-glass 계정은 후속 범위다.

별도 프런트엔드 origin의 실제 브라우저 시험에서 기존 CORS 처리 순서가 인증 앞의 OPTIONS 사전 요청을 막는 문제를 발견했다. CORS를 인증 검사 바깥으로 옮겨 사전 요청과 거부 응답에 적용하고 MFA 정책 안내 헤더를 노출했다. 실제 API 요청의 인증·정책 검사는 유지하며, 허용 origin 설정을 확장하지 않는다.

운영 배포 완료: 2026-10-06 01:56 KST, `/workspace/nabiman/deployments/mfa-policy-20261006-v1`.
바이너리 SHA-256 `859835b642321c6201b3fe76a8d95db05ea85854411c033debf4821bfdc0e1da`, 소스 아카이브 SHA-256 `4608b27c80301b15c3e0450db7f6de9b84431447c6c16f035f4f68960d46fe20`.
백엔드 404개·프런트엔드 158개, 타입·빌드·배포 패키지 검증 통과. lint 오류 0개·기존 경고 7개. MFA 등록·복구, 회원 승인·소속 격리·SSH 신뢰, 갱신 토큰, 실제 privileged helper 회귀 시험과 배포 보존 테스트 3개도 통과했다.
실제 debug/release 서버와 브라우저에서 소속 관리자의 정책 저장, 이미 로그인한 회원의 다음 요청 후 등록 안내, 필수 인증 완료 후 접근 복구, 다중 소속 정책 결합, 필수 상태 해제 거부와 코드 미소비, 이전 클라이언트의 MFA 정책 보존, 재시작 후 정책 유지, 손상 파일의 기본 거부를 확인했다. 최종 release에서는 토큰 갱신 후 미등록 제한과 MFA 검증 사실 보존, 별도 허용 origin에서 401·403 및 안내 헤더 수신, 미허용 origin의 응답 공유 제한·사전 요청 거부도 검증했다. CORS는 브라우저 응답 공유 정책이며 bearer 인증을 대신하는 접근 인가로 취급하지 않는다.
운영 HTTPS에서는 새 필수 정책 제어와 MFA 상태를 읽기 전용으로 확인했다. 실제 운영 회원의 MFA나 소속 정책은 변경하지 않았다. 가입 신청 왼쪽 정렬·로그아웃 화면 분리·회원/소속 관리·로그인 종료·실행 지문·실제 터미널 입출력·갱신 토큰 재사용 차단 검증 통과, 브라우저 오류 0건.
기존 정책 파일 바이트, 회원 ID·권한·비밀번호·MFA 상태, 서버 신뢰 정보 6개, 등록 터미널 16개와 tmux 17개를 보존했다. SQLite quick_check 정상, supervisor 정상 실행을 확인했다. 공개 HTTPS와 실제 실행 파일의 SHA-256이 일치하며 백업·manifest·브라우저 증거는 배포 디렉터리에 보관했다. 전체 상용화와 C05·C06·C07·C11·C13의 남은 범위는 계속 진행 중이다.

### 2026-10-06 — 소속 정책 충돌 방지와 최근 변경 이력

[RFC 9110 조건부 변경](https://www.rfc-editor.org/rfc/rfc9110.html#section-13.1.1)의 lost-update 방지 목적을 참고했다. HTTP If-Match/ETag 전체 구현이 아니라 JSON expected_revision을 비교하는 응용 계층 CAS다.

- 소속 정책 GET에 revision을 제공하고 PUT은 expected_revision·change_reason을 요구한다. 버전 누락은 428, 현재 버전 불일치는 409와 scope_policy_conflict 코드다. 변경 사유는 제어 문자를 제외한 3~500자다. 동일한 값으로 저장해도 새 UUID를 부여하므로 값이 원래대로 돌아온 경우에도 이전 요청이 통과하지 않는다.
- 기존 평면 정책 파일은 변경 없이 읽으며 scope 식별자와 정규화한 값의 SHA-256으로 legacy revision을 산출한다. 실제 저장 시 정책·revision·최근 history를 함께 원자 교체한다. require_mfa가 빠진 요청은 현재 MFA 값을 유지하지만, 버전과 사유를 생략한 이전 쓰기 클라이언트는 거부한다.
- 저장 잠금 안에서 현재 회원 ID·관리 권한·회원 수 제한·MFA 조건을 다시 확인한다. 회원→로그인 세션 순서로 잠금을 유지하며 현재 로그인 ID·회원 ID·만료 시각도 다시 확인한 뒤 파일을 교체한다. 인증 미들웨어 이후 로그아웃한 요청이 정책을 저장하지 못한다.
- history는 서버가 확인한 변경자 ID·이름, 시각, 사유, 이전/새 revision, 필드별 이전/새 값을 기록한다. 요청자가 history나 actor를 넣어 덮어쓸 수 없고, 거부된 쓰기는 저장 이력에 추가하지 않는다. 설정과 이력을 따로 저장하다 한쪽만 성공하는 경로를 만들지 않았다.
- GET /api/scope-settings/{kind}/{id}/history는 최고관리자 또는 해당 소속 관리자만 읽는다. 일반 회원과 다른 소속 관리자는 거부한다. 정책·이력 응답은 no-store다. 화면에서 충돌한 초안을 유지하고 재저장을 막으며, 사용자가 명시적으로 취소한 뒤 최신 값으로 다시 작성한다. 네트워크 응답 유실 시 자동 재전송하지 않고 현재 값과 이력으로 확인한다.
- 최근 100회만 보관하며 이전 변경 이력을 만들어내지 않는다. 이는 기존 일반 요청 로그를 보완하는 운영 기록이다. 독립·불변 감사 보관소, 전체 보존/내보내기, 외부 timestamp, 분산 writer/CAS·정책 DB migration, 대규모 이력 읽기 성능, 전원 장애 시 parent fsync 보증은 아직 완료하지 않았다.
- 새 정책 파일 형식은 이전 바이너리가 읽을 수 없다. 롤백은 바이너리만 교체하지 않고 배포 전 정책 파일도 함께 복원해야 한다. 새 버전에서 발생한 변경은 별도 보존 후 검토해야 하며, 기존 배포 도구는 실패 상태를 따로 보존한다. 이번 운영 배포 검증은 실제 운영 정책을 수정하지 않고 읽기만 수행한다.

운영 배포 완료: 2026-10-06 02:11 KST, `/workspace/nabiman/deployments/policy-history-20261006-v1`.
바이너리 SHA-256 `73f6ec2b31d7a5042ae08ecd6f3c1e8b26cbb0fa174b14482f16523a2be6a972`, 소스 아카이브 SHA-256 `e927f36cdfda3a0e7c79d424147ad73fd7d1fd9b02786be1c310f1490e57d220`.
백엔드 407개·프런트엔드 158개, 타입·빌드·배포 패키지 검증 통과. lint 오류 0개·기존 경고 7개. 기존 MFA 필수 정책과 회원 승인·소속 격리·SSH 신뢰 검증 및 배포 보존 테스트 3개도 통과했다.
격리한 실제 debug/release 서버와 브라우저에서 버전/사유 누락 거부, 동시 쓰기 한 건 성공, 충돌한 초안 유지와 명시적 재검토, 변경자·사유·정확한 전후 값, 실패 요청의 이력 미추가, 다른 소속·일반 회원의 이력 접근 거부, 같은 값으로 돌아온 뒤 과거 버전 거부, 재시작 후 버전·이력 보존을 확인했다. 단위 시험은 legacy 읽기의 파일 미변경, 다른 소속 설정 보존, 100회 보존 한도, 파일 교체 실패, 미들웨어 인증 이후 로그아웃한 요청의 저장 차단을 검증했다.
운영 HTTPS에서 변경 사유 입력·최근 이력 화면과 이력 API 200/no-store를 읽기 전용으로 확인했다. 실제 정책 파일 바이트는 배포 전과 동일하다. 가입 신청 왼쪽 정렬·로그아웃 화면 분리·회원/소속 관리·MFA 화면·로그인 종료·실행 지문·실제 터미널 입출력·갱신 토큰 재사용 차단 검증 통과, 브라우저 오류 0건.
기존 회원 ID·권한·비밀번호·MFA 상태, 서버 신뢰 정보 6개, 등록 터미널 16개와 tmux 17개를 보존했다. SQLite quick_check 정상, supervisor 정상 실행, 공개 HTTPS와 실제 실행 파일 지문 일치를 확인했다. 백업·manifest·브라우저 증거는 배포 디렉터리에 보관했다. 전체 상용화 및 독립 감사·transactional 저장·특권 분리·HA/DR 등의 남은 범위는 유지한다.


### 2026-10-06 — 최소 수집 지원 보고서와 외부 개선 리포트 격리

Red Hat의 지원 자료 검토·정제 절차와 [sos 프로젝트](https://github.com/sosreport/sos/wiki)를 참고해 C28의 첫 지원 수집 기능을 구현했다. 원본 자료를 광범위하게 모은 뒤 정제하는 방식 대신 명시한 숫자·상태·실행 지문만 DTO로 반환한다. sos와 동등한 진단 범위를 제공한다는 의미는 아니다.

- 최고관리자 전용 POST /api/diagnostics/support-report는 수집 전후의 현재 권한·로그인 세션을 확인한다. 다른 소속 관리자와 일반 회원은 거부한다. 요청 본문은 빈 객체만 허용하며 파일 경로나 추가 수집 옵션을 받지 않는다.
- 실행 SHA-256, OS·아키텍처·root 실행 여부·메모리 수치, 계정·로그인·등록 터미널·서버 집계, 인증 저장소 용량·디렉터리 권한, 최근 작업 1,000건의 상태 집계만 제공한다. 사용자·서버 이름/주소, ID 목록, 비밀정보, 환경 변수, 원본 오류/로그, 터미널 내용을 반환하지 않는다. 명령 실행·외부 연결·자동 업로드는 없다.
- 프로세스 내 동시 수집 1개와 최소 간격 5초를 적용한다. 파일 크기 제한과 비차단 잠금으로 busy/unavailable/too_large를 구분한다. 항목별 시점이 다를 수 있으며 연결·무결성·보안 보증이 아니다. 전체 JSON 미리보기와 검토 체크 후 동일한 파일만 내려받는다.
- 브라우저 시험에서 기존 외부 개선 리포트 스크립트가 관리자 로그인 시 자동 실행되는 것을 발견했다. 로컬 버튼으로 대체하고 실제 최고관리자 확인 후 명시적 클릭 때만 교차 출처 sandbox iframe을 연다. no-referrer, 정확한 메시지 origin/source 검사와 고정 공개 제품 정보만 허용한다. DOM 선택·현재 URL·진단 자료·토큰 전달·외부 화면 이동 요청은 차단하며 로그아웃 시 프레임을 제거한다.
- 외부 서비스에는 실제 개선 리포트를 제출하지 않았다. 해당 출처를 브라우저 라우팅으로 모사해 부모 저장소 접근 차단, 고정 context, DOM 선택 거부, 이동 거부와 로그아웃 제거를 검증했다.
- 전체 상용화 완료가 아니다. root API 분리, 독립 감사 보관, HA/DR, 서명 배포와 고객 현장 검증 등 기존 상용화 게이트는 계속 열린 상태다.

운영 배포 완료: 2026-10-06 02:37 KST, `/workspace/nabiman/deployments/support-report-20261006-v2`.
바이너리 SHA-256 `500dc12e777327956472f53208dee996659d2629b2079104d991100546296d44`, 소스 아카이브 SHA-256 `af36333a4196ad69cbf8003966bcbb4da9b90362ae423ed84166cc4fea5b055c`.
백엔드 413개·프런트엔드 160개, 타입·빌드·배포 패키지 검증 통과. lint 오류 0개·기존 경고 7개. 실제 debug/release 브라우저 지원 보고서·외부 프레임 격리 시험, release 갱신 토큰 회귀 시험, 배포 보존 테스트 3개 통과.
갱신 시험은 새 로컬 권한 확인 요청 때문에 요청 순서 기반 만료 재현이 불안정해져 첫 갱신 전 기존 토큰을 사용하는 요청에 만료 응답을 주도록 수정했다. 같은 초에 발행된 access JWT가 동일할 수 있으므로 첫 갱신 이후에는 해당 시험용 응답을 해제한다. 제품의 토큰 동작을 이 시험에 맞춰 변경하지 않았다. v1은 리허설만 수행했으며 최종 시험 소스를 포함한 v2를 실제 배포했다.
공개 HTTPS에서 진단 자료의 실제 실행 SHA-256, 검토 전 다운로드 제한, 미리보기와 다운로드 파일의 완전 일치, no-store를 확인했다. 가입 신청 왼쪽 정렬·로그아웃 터미널 분리·회원/소속 관리·정책 이력·MFA 설정 읽기·로그인 종료·실제 터미널 입출력·갱신 토큰 재사용 차단 통과, 브라우저 오류 0건.
기존 회원 ID·권한·비밀번호·MFA와 정책 파일, 서버 신뢰 정보 6개, 등록 터미널 16개와 tmux 17개 보존. SQLite quick_check 정상, supervisor PID 1102054 정상 실행. 백업·소스·manifest·다운로드/브라우저 증거는 배포 디렉터리에 보관했다. 전체 상용화 완료로 판정하지 않는다.


### 2026-10-06 — 예약 백업 실행 결과와 파일 복원 검증

[Portainer 백업 범위](https://docs.portainer.io/faqs/getting-started/what-does-portainers-backup-include), [Teleport 상태 복구](https://goteleport.com/docs/zero-trust-access/deploy-a-cluster/reliability/backup-restore/)를 참고해 로컬 파일 백업과 관리 플랫폼 상태 복구의 범위를 구분했다. 현재 기능은 전자이며 C15 전체 복구 게이트는 미완료다. Cron과 tar 인수 처리는 [crontab 형식](https://www.man7.org/linux/man-pages/man5/crontab.5.html), [GNU tar 옵션](https://www.gnu.org/software/tar/manual/html_section/Synopsis.html)을 참고했다.

기존 화면이 id/enabled 등 서버 필수 필드를 보내지 않아 예약 생성이 실패했고, 실행기는 tar 종료 결과와 저장 실패를 무시했다. 실행 시각만 갱신하고 비압축에도 .tar.gz를 붙였으며 저장 파일 손상은 빈 예약으로 처리했다. 요청·저장 모델을 분리하고 서버 생성 필드의 위조를 거부한다. 예약 파일은 임시 파일 fsync→원자 교체→부모 fsync 후 메모리에 반영하며 저장 실패 시 예약 기능을 중단한다.

실행 전 running을 먼저 저장하고 성공/실패·시작/종료·완성 파일명·고정 오류를 저장한다. 마지막 성공 시각은 성공일 때만 갱신하며 이전 버전 기록을 검증된 성공으로 변환하지 않는다. 재시작 시 미완료 running은 interrupted로 남기고 자동 재시도하지 않는다. 이전 프로세스가 파일을 완성했으나 결과를 저장하지 못했을 수 있으므로 재실행 전 파일 확인을 안내한다.

예약 실행은 단일 worker, 분 단위 durable slot으로 중복을 방지한다. UTC 숫자·리스트·범위·step·일자/요일 OR를 지원한다. 중단 중 누락된 예약을 재생하지 않고, 5분·1 GiB 한계와 source 64개 제한을 적용한다. 원본은 절대 경로만 허용하며 상위 이동·옵션 인수·출력 폴더를 포함한 재귀 백업을 거부한다. tar 환경 옵션을 제거하고 -- 이후에 경로를 전달하며 shell을 사용하지 않는다. private staging에서 성공한 결과만 고유 이름과 0600 권한으로 공개하고 tar 실패 시 부분 결과를 숨기고 정리한다. 원본 변경 중 snapshot 일관성이나 모든 내부 symlink/특수 파일의 백업을 보장하지 않는다.

압축 생성 API도 같은 worker 구현을 사용한다. 화면에는 수집 오류, 실행 상태와 결과 파일을 표시하고 지금 실행을 제공하며 새 로그인 갱신 경로를 사용한다. 일반 회원은 백업 목록/경로도 읽지 못한다. 아카이브를 단일 파일 복원 API로 잘못 덮어쓰지 않도록 .bak만 화면 복원을 허용한다. 기존 암호화 CBC API, 전체 관리 상태 transaction backup, 독립 원격 보관·보존 정책, 서비스 권한 분리와 실제 고객 DR은 후속 과제다.

운영 배포 완료: 2026-10-06 02:52 KST, `/workspace/nabiman/deployments/backup-schedules-20261006-v1`.
바이너리 SHA-256 `bec97801294184e4d5d0f97cfa5ac1b3e6be850494396dd05ff7c24f1e8eadd8`, 소스 아카이브 SHA-256 `775d87d8b652391d4bf215a33d319b2ebf1752cd6956869f0b39b2205007001c`.
백엔드 418개·프런트엔드 160개, 타입·빌드·운영 패키지, 배포 보존 3개 시험 통과. lint 오류 0개·기존 경고 7개. debug/release 실제 서버와 브라우저에서 예약 화면 생성, 수동/UTC 자동 실행, 실패의 last_run/filename 미기록, 일반 회원의 경로 조회 거부, 0600 권한, 실제 아카이브 추출 후 원본 바이트 일치, 같은 분의 자동 실행 중복 방지, 재시작 시 running→interrupted, 손상 파일의 503·원본 보존을 검증했다. 최초 브라우저 시험은 잘못된 #administration/backup 경로로 실패했고 실제 메뉴의 #management/backup으로 수정 후 통과했다. API 경로나 인가를 시험에 맞춰 완화하지 않았다.
운영에서는 기존 예약이 0건임을 확인하고 예약을 생성하거나 실행하지 않았다. 공개 HTTPS 예약 화면/API, 진단 보고서 다운로드, 가입 정렬·로그아웃 화면 분리·회원/소속 관리·정책 이력·MFA 상태·로그인 회수·실제 터미널 입출력·갱신 재사용 차단 통과, 브라우저 오류 0건. 예약 파일·소속 정책·회원 권한과 비밀번호/MFA·서버 신뢰 6개·등록 터미널 16개·tmux 17개 보존. SQLite quick_check 정상, supervisor PID 1110889 정상 실행. 백업과 검증 manifest/증거는 배포 디렉터리에 보관했다. C15 전체 재해 복구와 전체 상용화 완료를 의미하지 않는다.


### 2026-10-06 — age 인증 암호화와 새 디렉터리 복원

[age v1 명세](https://age-encryption.org/v1), [Rust age API](https://docs.rs/age/0.12.1/age/), [scrypt 비용 제한](https://docs.rs/age/0.12.1/age/scrypt/struct.Identity.html)을 참고했다. 새 파일은 표준 age 형식이며 암호 알고리즘이나 파일 암호 포맷을 직접 설계하지 않는다. age 0.12.1·tar 0.4.46을 고정하고 lockfile을 갱신했다. age crate의 BETA 표기는 제거하지 않으며 해당 의존성의 독립 보안 검토가 끝났다는 주장을 하지 않는다.

기존 CBC API는 비밀번호를 `openssl -pass pass:...` 프로세스 인수에 넣고 인증 없이 입력 경로로 직접 압축을 풀었다. 새 API는 SecretString과 age 라이브러리를 사용하며 외부 암호화 명령을 호출하지 않는다. scrypt work factor 18, 최대 복호화 비용 18, 비밀번호 12자 이상/1024바이트 이하, 제어 문자 거부, 동시 작업 한 건을 적용한다. 작은 단일 scrypt 헤더만 허용한다.

생성은 typed regular-file/directory 순회→크기 제한 tar→gzip→age 스트림이다. 평문 임시 아카이브를 만들지 않는다. 파일 크기 변화는 tar 라이브러리가 검사하지 않음을 소스에서 확인하여 정확한 길이의 reader와 후속 metadata 검사를 추가했다. 파일 snapshot 보장은 아니다. 원본 겹침·링크·특수 파일·출력 폴더 재귀를 거부한다. 완성 ciphertext만 고유 이름과 0600 권한으로 공개한다.

복원은 ciphertext 크기·헤더/KDF 비용 제한→전체 인증 복호화→private staging의 gzip/tar 검증→NOREPLACE 원자 디렉터리 공개 순서다. 1 GiB 확장 한도와 100,000항목 한도를 적용하고 경로 탈출·링크·특수 파일·중복 파일을 거부한다. 파일 fsync·디렉터리 fsync 후 공개하고 부모 fsync 결과까지 확인한다. 대상이 검사 이후 생겨도 덮어쓰지 않는다. 권한은 0600/0700으로 정규화하며 소유자/실행 비트/ACL/xattr는 복원하지 않는다. API root 분리와 파일시스템 snapshot·전체 관리 상태 복구는 여전히 미완료다.

복원 중 평문 staging은 필요하며 crash 후 잔여가 있을 수 있다. 이를 암호화된 임시 저장소나 보안 삭제로 표현하지 않는다. CBC .enc는 파일을 보존하고 422와 오프라인 복구 안내를 제공한다. 기존 운영 .enc 수는 0개였다. README에 비특권 격리 환경·원본 보존·프롬프트 비밀번호·선택 파일 복구·age 재백업 순서를 기록했다.

화면은 비밀번호 확인, 복원 대상 변경 시 검토 해제, 요청 직후 비밀번호 입력 비움, 링크/권한/용량 제한과 legacy 안내를 제공한다. 목록 갱신 때문에 폼이 unmount되어 성공 메시지를 잃던 문제도 수정했다. 복원이나 암호화를 운영 데이터로 시험하지 않고 격리 fixture를 사용한다.

운영 배포 완료: 2026-10-06 03:14 KST, `/workspace/nabiman/deployments/encrypted-backup-20261006-v1`.
바이너리 SHA-256 `801bf4a2d4653fc37ec93ba7e64855e771907ad0eee563aafa33c86626a1d7d3`, 소스 아카이브 SHA-256 `8324a8421686b1fa9be95a315afa722bccd6b576bb4ce0fec77cc031b3d73488`.
백엔드 423개·프런트엔드 162개, 타입·빌드·배포 패키지, 배포 보존 3개 시험 통과. lint 오류 0개·기존 경고 7개. debug/release 실제 서버·브라우저에서 생성/복원 내용 일치, 잘못된 비밀번호·변조·잘림·기존 대상 거부, 일반 회원 거부, 비밀번호 입력 비움, legacy 파일 보존, private artifact와 정상 종료 후 평문 잔여 없음 확인. 경로 탈출·링크·특수 파일·중복·과도한 KDF·소스 크기 변경·복원 대상 생성 경쟁은 단위 시험으로 확인했다.
표준 Go age v1.2.1로 나비맨 파일을 해독하고 Go가 생성한 파일을 나비맨으로 복원하는 양방향 호환성을 debug/release 모두 확인했다. 재현 도구는 `scripts/age-interop-test`이며 `NABIMAN_AGE_REFERENCE`에 해당 Go 도구 바이너리 경로를 주고 `scripts/smoke-encrypted-backup.py`를 실행한다. CI의 기본 smoke는 Go oracle 없이 실행하며 양방향 시험은 이번 배포 증거에 별도 포함한다. 기존 예약 백업의 실제 자동 실행·아카이브 추출·중단 복구 회귀 시험도 통과했다.
운영에서는 암호화/복원을 실행하지 않았다. 공개 HTTPS에서 새 화면의 한글 표시·입력 정렬·실행 전 입력/검토 조건, 기존 예약 조회·진단 보고서·가입 신청·회원/소속 관리·정책 이력·MFA 상태·로그인 종료·갱신 재사용 차단·실제 터미널 입출력을 확인했다. 브라우저 오류 0건. 기존 회원/비밀번호/MFA·정책·예약 파일·서버 신뢰 6개·등록 터미널 16개·tmux 17개 보존, 백업 파일 전후 해시 일치, SQLite quick_check 정상. supervisor PID 1132902 정상 실행. 백업·manifest·스크린샷과 시험 증거를 배포 디렉터리에 보관했다. 전체 상용화/전체 DR 완료로 판정하지 않는다.


### 2026-10-06 — 관리 상태 묶음과 격리 복구 검증

[Teleport 복구 문서](https://goteleport.com/docs/installation/self-hosted/helm-deployments/disaster-recovery/)의 상태·키 복구 범위와 [SQLite backup API](https://www.sqlite.org/backup.html)의 일관된 DB 복사를 참고했다. SQLite snapshot만으로 auth JSON과 DB 사이의 일관성이 생기지는 않으므로 API 정지 및 기존 DB/터미널 저장소 소유 잠금을 전제로 한다. 도구가 서비스를 임의로 정지하거나 자동 활성화하지 않는다.

웹 API에서 검증된 age 처리 코어를 라이브러리로 분리하고 오프라인 `nabiman-recovery` 및 운영자용 `management-recovery.py`를 패키지·설치·배포에 포함했다. manifest 파일 목록/해시/크기, 회원 불변 ID, 필수 인증 파일, 실제 tenant_profiles/jobs 스키마와 SQLite quick_check, 원격 비밀번호와 저장 키의 해독 일치를 복원 공개 전에 검사한다. 임시 복사본의 평문과 crash 잔여, 용량 제한 및 외부 의존성 제외를 공개 운영 안내에 명시했다.

통합 시험에서 실제 스키마 이름과 정책 PUT 요청 형태의 불일치를 발견해 바로잡았다. 단위 fixture에만 기대지 않고 실제 서버에서 만든 계정·소속·정책·암호화 자격 증명·DB 값을 캡처하고 복구한 실행 파일로 재로그인하는 시험을 배포 조건으로 둔다. 이 단계는 전체 호스트 DR, 외부 보관, 자동 failover 또는 상용화 전체 완료를 뜻하지 않는다.

배포 전 실제 복구 시험 통과: 실행 중 저장소 캡처 거부, 정지 후 캡처, 암호화 비밀번호/키 일치 검증, 기존 대상 덮어쓰기 거부, DB canary 값과 정책 이력 보존, 복구한 실행 파일의 관리자/회원 로그인 및 불변 ID·서버 소속 일치. 백엔드 전체 426개 후 수정한 라이브러리 22개 재검증, 프런트엔드 162개·타입·빌드 통과, lint 오류 0개/기존 경고 7개. 통합 시험은 `scripts/smoke-management-recovery.py`이고 CI의 격리 테스트 단계에도 추가했다.

운영 배포 완료: 2026-10-06 04:04 KST, `/workspace/nabiman/deployments/management-recovery-20261006-v1`.
서버 SHA-256 `98cf68670af509e96d977240c9d89791e8f62dd21ab4bdb05c41241d907b4366`, 소스 아카이브 SHA-256 `923bf0c9de5de4b73d5e9cf84a91a3a4cdd342fb88ea03528d42ffd43b6ca087`.
최종 패키지 실행·운영 사본 리허설·supervisor 교체·로컬/공개 HTTPS 인증 검사를 통과했다. 공개 브라우저 첫 시도는 터미널 연결 완료 전 입력에 따른 대기 시간 초과로 실패했다. 검증 스크립트가 세션 handshake와 초기 출력을 기다리도록 수정한 재시험은 실제 터미널 입출력까지 통과했다. 수정된 검증 스크립트와 해시는 배포 evidence/completion.json에 별도로 보관했다. 제품 바이너리·정적 화면은 배포 manifest 그대로다.
가입 신청·왼쪽 입력 정렬·로그아웃 터미널 격리·회원/소속 관리·정책 이력·MFA 읽기·로그인 세션 종료·refresh 재사용 차단·진단 보고서·백업 입력 조건·복구 안내 링크·실제 터미널 동작을 공개 HTTPS에서 확인했다. 브라우저 오류 0건. 기존 회원 자격/권한(정상 로그인 시각 갱신 제외)·인증 키·서버 신뢰 6개·등록 터미널 16개·tmux 17개·예약/알림·기존 공유 기록 보존, SQLite quick_check 정상, supervisor PID 1161775.
실제 교체 직전의 정지 상태 사본을 설치된 복구 도구로 암호화하고 새 경로에 복원했다. 관리 파일 69개·계정 1개·서버 6개·암호화 자격 증명 5개의 키 해독 및 DB를 검증했다. 복원한 기존 바이너리로 격리 인스턴스를 기동해 실제 관리자 로그인/관리 API를 확인하고 종료했다. 격리 터미널은 기존 등록 목록을 재현한 16개 fixture이며 원래 원격 연결 자체를 복원한 것은 아니다. 훈련 후 운영 인스턴스 해시와 원래 tmux 보존도 확인했다. 약 105.5초의 측정은 이 호스트의 훈련 시간이며 상용 RTO 약속이 아니다.
복구 proof는 `management-recovery-verification.json`, 종합 결과는 `completion.json`이다. 암호화 묶음과 복원 사본은 private 배포 경로에, 복구 암호는 해당 묶음 밖의 root 전용 로컬 파일에 보관한다. 이 훈련에는 활성 예약/알림이 없고 외부 공급자는 mock이다. 외부 키 보관·다른 호스트 DR·OS/TLS/실제 원격 연결·전체 상용화 완료를 주장하지 않는다.


### 2026-10-06 — 전역 감사 저장 실패 처리와 보존 파일 검증

[OWASP Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html)의 민감 데이터 제외·오류 시험·기록 보호 원칙과 [rsyslog queue 문서](https://docs.rsyslog.com/doc/concepts/queues.html)의 저장 내구성과 전송 분리를 비교했다. 이번 단계는 로컬 요청 기록의 내구성·오류 표시·검사·관리자 내보내기에 적용하며 외부 전송 queue의 내구성을 구현했다는 뜻은 아니다.

사전 운영 검사에서 활성 파일의 기존 형식 오류를 확인했다. 배포된 검증기의 JSONL LF 구분 기준으로 활성 파일의 형식 오류는 30줄이다. 초기 Python splitlines 점검의 61개 조각과 구분한다. 기존 파일을 보정하거나 삭제하지 않는다. 저장소는 현재 원본의 파싱 가능한 최근 1,000개와 해시 tail을 읽고, 제외된 형식 오류 줄 수를 health에 표시한다. 매 요청마다 전체 파일을 다시 읽던 동작을 제거하고 inode/크기 변경 감지, O_NOFOLLOW/일반 파일, 0600 신규 생성, append/file fsync/parent fsync, 직렬 회전과 실패 고정 상태를 추가했다. 실패한 쓰기를 메모리에서 정상 저장으로 표시하지 않으며 작업 결과를 허위로 실패/rollback 처리하지 않는다. 회전 중 실패는 기록이 이미 디스크에 있을 수도 있으므로 점검 후 재시작이 필요하다.

새 v2 레코드는 domain과 JSON tuple로 필드 경계를 해시하고 기존 방식은 검증용으로 유지한다. 기존 해시 방식의 필드 경계 모호성, 파일별 체인 재시작, 독립 anchor가 없다는 한계를 문서화했다. 무결성 검사는 고정된 현재/회전 파일만 대상으로 하며 최대 12 MB 읽기 한도를 적용한다. 파일 읽기 실패를 정상 빈 기록으로 표시하지 않는다. export는 동일 writer lock 아래의 일관된 파일 사본에서 원본 digest·검사 결과를 만들고 파싱 가능한 항목을 경로 민감 값 제거 후 반환한다. 원본과 동일한 chain 검증 파일이라고 표시하지 않는다.

전역 audit 경로 전체를 최고관리자 전용으로 제한하고 일반 회원 UI에서 숨겼다. 웹 clear API는 410으로 원본 삭제를 거부한다. 저장 장애는 health·응답 헤더·프로세스 오류 기록에 나타난다. 이는 pre-action audit transaction, root 분리, 모든 오류/WS/frame 기록, 독립 불변 감사, 외부 forwarding 성공 보장 또는 C11 전체 완료가 아니다. 기존 forwarding 구현의 transport/credential/queue 보강은 여전히 남는다.

배포 전 검증: backend 433개(22 library + 378 main + 5 auth + 28 integration), frontend 164개 통과. 타입·프런트 빌드 통과, lint 오류 0개/기존 경고 7개, 배포 보존 3개 시험 통과. 새 단위 시험은 동시 쓰기, 실패 시 메모리 성공 방지·명시적 재시작, 비정규/부분 파일, 과거 오류 보존, 회전 파일과 0600 권한, 구형 연결과 v2 필드 경계, 전체 전역 감사 권한을 다룬다.
`scripts/smoke-audit.py`의 실제 debug 서버와 Chromium에서 일반 회원 거부(인코딩 경로 포함), 과거 형식 오류 보존, 파일별 검사, 공유 토큰/쿼리 제거, clear 410, 검토 후 실제 다운로드, 파일을 디렉터리로 교체한 저장 장애의 응답 헤더/health, 저장소 복원 후에도 자동 정상화하지 않음, 재시작 후 기록 재개를 확인했다. CI에도 이 격리 시험을 추가했다. 운영 데이터에 장애를 주입하지 않는다. 배포 리허설과 운영 교체 성공 조건에도 실제 로그인 뒤 감사 writer 상태를 확인하도록 추가했다.

운영 배포 완료: 2026-10-06 04:44:59 KST, `/workspace/nabiman/deployments/audit-integrity-20261006-v1`.
바이너리 SHA-256 `6e48a88da7d2d8113a01770b947675d09e0cadb8c2c4351fd47821bcd8ef80ce`, 소스 아카이브 SHA-256 `22412859d6ab9661998129521db66f3d895f8937d02a4b5d53ce9a4dc036c274`.
최종 release에서도 실제 서버·Chromium 장애/내보내기 시험과 배포 패키지 smoke를 통과했다. 운영 사본 리허설 및 실제 교체 뒤 로그인·읽기 API·감사 writer 정상 상태를 확인했다. 공개 브라우저에서 새 감사 화면·보관 파일 검사, 기존 가입/정렬/로그아웃 격리·회원/소속 정책·MFA 읽기·로그인 회수·refresh 재사용 차단·지원 보고서·백업/복구 안내·실제 터미널 입출력을 확인했다. 브라우저 오류 0건.
`scripts/verify-production-audit.py`는 교체 직전 원본 감사 파일 3개의 모든 바이트가 현재 보관 파일의 동일 내용/접두부로 유지됨을 확인했다. 새 v2 기록 23개를 별도 Python JSON tuple/SHA-256 계산으로 검증했다(해당 검사 시점의 사본 범위). 운영 검사에서 과거 파일 .2는 형식 오류 30줄/해시 불일치 529건, .1은 16줄/289건, 활성 파일은 30줄/224건이었다. 모두 이전 원본을 보존한 채 표시하며 공격 여부나 과거 기록 전체의 진위를 단정하지 않는다. 따라서 운영의 전체 과거 무결성 결과는 false이고 새 writer의 저장 상태는 정상이다.
회원 자격/권한(정상 로그인 시각 갱신 제외)·비밀번호·인증/자격 증명 키·원격 서버 신뢰 6개·등록 터미널 16개·tmux 17개·정책·예약/알림·공유 기록 보존, SQLite quick_check 정상. supervisor PID 1184665. 기존 관리 데이터 복구 도구도 패키지에 유지했다. `audit-verification.json`, `browser-verification.json`, `completion.json`, 비공개 evidence에 검증 결과를 보관했다. 독립 외부 감사·전송 queue·조직별 전체 감사·상용화 전체 완료는 여전히 별도 과제다.

## 2026-10-06 — 영속 HTTPS 감사 전송

rsyslog 공식 queue 개념 문서(https://docs.rsyslog.com/doc/concepts/queues.html)의 큐 영속성과 수신 확인 구분을 참고해 기존 curl 기반 전송을 SQLite 대기열과 검증된 HTTPS 클라이언트로 대체했다. 이 구현을 rsyslog 수준의 모든 신뢰성·처리량 보장으로 간주하지 않는다. 최고관리자만 목적지 생성·시험·활성화·중지·삭제·차단 재시도를 수행한다. 기존 설정은 보존하되 자동 전송하지 않는다. Authorization은 암호화 저장하고 API·명령 인수·오류 응답으로 반환하지 않는다. CAS revision, 목적지별 큐 상태, 누락·취소·HTTP 2xx 확인 횟수를 표시한다.

대기열은 synchronous FULL/트랜잭션으로 기록하고 재시작 시 진행 중 요청을 같은 Idempotency-Key로 재시도한다. 수신 측 중복 제거가 필요하며 2xx를 독립 불변 보관 증명으로 표시하지 않는다. 8회 실패 후 차단하고 용량 초과 시 자동 과거 기록 재수집 없이 누락을 표시한다. 최대 8개 목적지, 5,000건·32 MiB payload·개별 16 KiB 한도와 중지/삭제 시 진행 중 요청의 한계를 README에 명시했다. 복원·리허설에는 발송 중지 환경 변수를 적용하고 복원 키 검증에 전송 자격 증명을 포함했다. 기존 암호문이 있으면 누락된 키를 새 키로 대체하지 않는다.

backend 441개 시험과 frontend 166개 시험, 타입·lint(오류 0/기존 경고 7)·빌드 통과. 실제 release 서버와 격리 HTTPS 수신기에서 503/302 실패 표시, TLS 신뢰/호스트명 검증, redirect 미추적, 암호문 저장, 재시작 재시도 ID 유지, 복원 발송 중지, 활성화된 요청 전달, 구형 설정 보존, 브라우저 등록/시험/삭제를 검증했다. 시험 인증서는 서버 용도 CA:FALSE로 수정했으며 TLS 검증을 완화하지 않았다. 실제 외부 수신기는 추가하지 않았다. root API 분리·전역 pre-action 감사 원자성·독립 불변 보관·상용화 전체 완료는 별도 과제다.

운영 배포 완료: 2026-10-06 05:14:02 KST, `/workspace/nabiman/deployments/audit-forwarding-20261006-v1`.
바이너리 SHA-256 `3a37e49b259eebc7def4ace2dbc8ef10d14ee8fe9525e407b400f3e4df808a87`, 소스 아카이브 SHA-256 `b6438a4425e7e55761ee11a72dbc5d6bc73f38af0705abbc4dc016ae5032874e`.
배포 패키지·감사 장애/내보내기·관리 데이터 age 복원 회귀·운영 사본 리허설을 통과한 후 supervisor의 실제 실행 파일과 정적 화면을 교체했다. 공개 HTTPS에서 실행 digest 일치, 새 전송 화면/API 정상 및 실제 외부 수신처 0개를 확인했다. 공개 브라우저에서 가입 신청·입력 왼쪽 정렬·로그아웃 터미널 격리·회원/소속 정책·MFA 읽기·로그인 회수/refresh 재사용 차단·감사 검사·복구 안내·실제 터미널 입출력 시험 통과, 브라우저 오류 0건. 기존 회원 권한·인증 키·서버 신뢰 6개·등록 터미널 16개·tmux 17개를 보존했고 DB quick_check 정상이다. 기존 감사 파일 원본 보존과 v2 해시를 별도 Python으로 검증했다. 과거 기록의 기존 불일치는 삭제하거나 정상으로 표시하지 않는다. 완료 증거는 배포 디렉터리의 `completion.json`, `forwarding-verification.json`, `browser-verification.json`, `audit-verification.json`, 비공개 evidence에 보관했다.

## 2026-10-06 — 만료형 읽기 자동화 토큰

벤치마킹: Grafana 공식 service accounts 문서(https://grafana.com/docs/grafana/latest/administration/service-accounts/)는 독립 identity·권한 상속·복수 토큰과 만료/회수를 설명한다. Teleport Machine & Workload Identity(https://goteleport.com/docs/machine-workload-identity/)는 단기 인증서 기반의 workload identity를 제공한다. 두 제품의 전체 모델을 구현했다고 주장하지 않는다. 나비맨의 기존 API 키 생성 UI와 저장 함수는 있었으나 JWT 인증 경로에 연결되지 않아 실제 API 호출 인증에는 사용되지 않았다. 우선 해당 기능을 실제 검증 가능한 owner-bound 읽기 자동화 토큰으로 교체하고, 독립 service account·tenant 관리자 발급·쓰기 action·단기 workload 인증서는 C18/C08 후속으로 유지한다.

새 토큰은 최고관리자 본인의 불변 회원 ID에 귀속되고 256-bit OS 난수/해시 저장, 필수 1~90일 만료, 명시적 status/servers/tenants 읽기 범위만 가진다. URL·method는 정확한 allowlist로 판정하며 사용자/인증/터미널/비밀/쓰기 경로를 차단한다. Bearer 또는 X-API-Key 한 종류만 받고 혼합·중복 credential은 거부한다. 기존 키는 보존하되 version 0으로 인증하지 않는다. 발급 당시와 현재의 비밀번호/MFA/역할/소속/접근 버전 지문 및 불변 ID가 달라지면 재발급이 필요하다. 소유 회원의 실제 권한도 매 요청 확인한다.

마지막 사용과 회수를 private file fsync·rename·directory fsync 후 반영하고 저장 오류에는 토큰 인증/변경을 중지한다. 이미 인가된 요청은 회수 후에도 완료될 수 있다. 최고관리자 JWT 관리 화면은 별도이며 목록에 해시/보안 지문/원문 토큰을 노출하지 않는다. 감사에는 token ID와 owner ID를 표시하고 token으로 조회한 status도 기록한다. 복구/리허설에는 `NABIMAN_API_TOKENS_DISABLED=1`을 적용하고 과거 백업이 회수 전 토큰을 포함할 수 있음을 복원 안내에 명시했다. 운영의 기존 API 키 레코드는 사전 검사 시 0개였다.

검증: backend 442개(23 library + 386 main + 5 auth + 28 integration), frontend 168개/43 suite 통과. 타입·빌드 통과, lint 오류 0/기존 경고 7. 실제 release 서버 시험은 Bearer/X-API-Key 양쪽 인증, 정확한 읽기 범위, 다른 API와 쓰기 거부, 혼합/중복/URL credential 거부, 만료 입력 한도, 구형 키 비활성 보존, 0600 해시 전용 저장, 재시작 후 사용/회수, 운영자 인증 중지, 불변 소유자 변경 시 거부, 감사 식별 및 브라우저 발급·실제 호출·회수를 포함한다. 단위 시험에서 잘못된/과거 만료, 비정규 저장소, 중복 ID, 저장 장애 고정 상태, 인증/권한 지문 변경을 확인했다. 관리 화면에서 자동화 인증 subset OpenAPI JSON을 제공한다. 전체 API와 독립 service account 명세로 표시하지 않는다.

운영 배포 완료: 2026-10-06 05:28:59 KST, `/workspace/nabiman/deployments/automation-tokens-20261006-v1`.
바이너리 SHA-256 `254502f4e90c9f23bd0c99adfa11752556f74907888e63bdd69a58d1a43d68d3`, 소스 아카이브 SHA-256 `91ee7ff961bb466de152f8dcfcf8f1968cf75c733b4256f26f87651553d2b0d7`.
최종 release의 감사 장애/내보내기·HTTPS 전송/재시도·관리 데이터 복구 회귀 및 배포 패키지 smoke 통과. 운영 사본에서 외부 전송과 API 토큰 인증을 중지한 리허설을 통과한 후 실제 supervisor 바이너리/UI를 교체했다. 공개 HTTPS에서 브라우저로 임시 토큰을 발급하고 status 조회 성공·회원 조회 403·회수 후 401을 확인했다. 비밀값은 증거에 남기지 않았고 시험 토큰 0개/기존 토큰 목록 보존을 확인했다. 공개 OpenAPI JSON도 정상 응답했다. 관리 메뉴의 실제 표기는 **보안 → 엔터프라이즈 인증 → API 키 관리**이다.

가입·왼쪽 정렬·로그아웃 터미널 격리·회원/소속 정책·MFA 읽기·로그인 회수/refresh 재사용 거부·감사/복구 안내·실제 터미널 입출력 검증 통과, 브라우저 오류 0건. 회원 권한·기존 인증 키·서버 신뢰 6개·등록 터미널 16개·tmux 17개 보존 및 SQLite quick_check 정상. 감사 원본 파일 3개 보존과 v2 레코드 305개(검사 시점의 전체 v2 범위) 별도 계산 검증 통과. 과거 기존 불일치는 계속 표시한다. 배포 디렉터리에 `completion.json`, `tokens-verification.json`, `browser-verification.json`, `audit-verification.json`, 비공개 evidence를 보관했다. C18 전체 및 상용화 전체 완료로 처리하지 않는다. 배포 후 검증 스크립트의 실패 출력에서 브라우저 stderr를 제거하여 자격 증명 포함 가능성을 차단했고 README의 메뉴 표기를 실제 UI와 맞췄다(실행 제품 변경 없음).

## 2026-10-06 — 배포 구성 목록과 패키지 파일 검증

CycloneDX 공식 개요(https://cyclonedx.org/specification/overview/)의 구성/의존 관계 표현과 SLSA provenance 정의(https://slsa.dev/spec/v1.2/provenance)를 검토했다. 의존 목록과 파일 체크섬이 서명·빌드 출처 증명을 대신하지 않는다는 차이를 제품과 문서에 반영했다. 기존 패키지는 외부 tar SHA-256만 제공했다. 패키징 단계에 모든 내부 일반 파일의 SHA-256·크기·mode manifest와 검사기를 추가하고 잠금 파일 기준 CycloneDX 1.6 inventory를 생성한다. Cargo.lock 및 npm v3 잠금 파일의 1,334개 구성 항목과 잠금 파일에서 확인 가능한 연결·체크섬을 기록한다. 개발/선택/다른 플랫폼 패키지도 포함하고 npm peer graph·OS 라이브러리·Cargo license 정보의 불완전성을 명시한다. 이 목록의 lifecycle은 design이며 실제 바이너리 구성 측정으로 표기하지 않는다.

manifest 검사기는 빠진/추가/변조/중복 파일, 위험 경로·링크·특수 파일·특권 mode, 수량/확장 크기 초과를 거부한다. 새 경로에만 추출하고 추출 중 해시 재검사와 Linux renameat2 NOREPLACE로 공개한다. 외부 신뢰 루트가 없는 파일 일관성 검사라고 표시한다. installer는 새 릴리스 자산의 검사기를 사용한 새 형식만 허용한다. GitHub 공개 릴리스/태그는 발행하지 않았으며 해당 installer 게시에는 새 자산의 동시 발행이 필요하다.

진단 화면에서 SBOM/명세를 열고 현재 실행 파일의 digest와 패키징 기록을 비교한다. 기록 오류나 실행 정보 조회 실패를 정상 일치로 표시하지 않는다. reviewed deployment prepare는 바이너리 3개·복구 wrapper·잠금 파일·프런트 입력 파일이 receipt와 일치해야 진행한다. 공개된 SBOM과 receipt에는 로컬 절대 경로·환경 변수·인증 자료·저장소 URL의 자격 증명을 넣지 않는다.

검증: 프런트 170개/44 suite, 타입/빌드, lint 오류 0/기존 경고 7 통과. 변조·누락·추가·mode·중복, 이탈 경로와 symlink/hardlink/device/FIFO, 확장 한도·파일/부모 충돌·신규 추출/덮어쓰기 거부를 포함한 Python 4개 시험 통과. 생성 SBOM은 CycloneDX 공식 1.6 JSON schema 및 format 검증 통과(고정 schema SHA-256 `3e92dddbc30cf7f6a02b80f0942b1a4cfd4fb1c26f1dfc4310afa9d613cafb93`). 패키지의 내부 파일 검사와 실제 서버 실행·로그인·보호 API smoke 통과. 서버 바이너리 변경은 없으며 앞 단계에서 검증한 digest를 유지한다. C16 전체 완료나 서명된 릴리스로 처리하지 않는다.

운영 배포 완료: 2026-10-06 05:41 KST, `/workspace/nabiman/deployments/release-inventory-20261006-v1` (정확한 시각은 activated.json).
서버 바이너리 SHA-256 `254502f4e90c9f23bd0c99adfa11752556f74907888e63bdd69a58d1a43d68d3` 유지, 소스 아카이브 SHA-256 `11cdf566d9391a87f2791e249db66c27a13876d1ba20305bbcb36a9200853112`.
새 UI·SBOM·명세를 실제 supervisor 운영 경로에 배포한 뒤 공개 HTTPS에서 두 JSON digest·구성 항목 수·현재 실행 파일 digest 일치 및 진단 화면의 일치 표시/다운로드 링크를 확인했다. 실제 메뉴는 **모니터링 → 진단**이다. 공개 브라우저의 가입/회원/정책/로그인 회수/감사/복구 안내/터미널 입출력 회귀 통과, 오류 0건. 기존 회원 권한·인증 키·서버 신뢰 6개·등록 터미널 16개·tmux 17개를 보존하고 DB quick_check 정상이다. 감사 원본 보존과 v2 해시 별도 검사도 통과했으며 과거 불일치를 보정하지 않았다.
실제 패키지 28개 일반 파일(자기 manifest 제외), 확장 45,245,763 bytes의 검증 및 새 디렉터리 추출을 확인했다. 배포 디렉터리의 artifacts에 패키지/체크섬/명세/검증기를, evidence와 inventory-verification.json·browser-verification.json·audit-verification.json·completion.json에 검증 결과를 보관했다. 이 단계는 운영 배포와 로컬 배포물 준비이며 GitHub 공개 릴리스, 독립 publisher signature/attestation, CVE/license 승인 또는 C16/상용화 전체 완료가 아니다.


## 2026-10-06 — 의존성 보안 갱신

cargo-audit 0.22.2와 RustSec DB commit `ef6173cbc5c50ec8166f9a5b28f07834144373ee`를 사용했다. 최초 Cargo.lock 12개 취약점에서 최종 잠금 파일 2개(h2/RSA)로 감소했다. 나머지는 삭제·업데이트된 의존성에 해당하며 독립 침투 시험 결과가 아니다. `rustls 0.23.45`, `rustls-webpki 0.103.15`, `crossbeam-epoch 0.9.21`, `quinn-proto 0.11.19`, `docx-rs 0.4.22`/`quick-xml 0.41.0`, `russh 0.64.1` 등을 반영했다. anyhow/event-listener/rand의 unsound 경고와 yanked spin도 해소했다. 최종 유지보수 중단 경고는 backoff/derivative/instant/proc-macro-error2/rustls-pemfile 5개로 남는다.

npm의 baseline-browser-mapping/browserslist/js-yaml/picomatch 전이 의존성과 연결 잠금 항목을 갱신해 npm audit 4개 패키지에서 0개로 감소했다. frontend 직접 패키지 정의는 바꾸지 않았다. CI/릴리스 npm audit을 필수로 하고 Cargo 기존 시험/릴리스 빌드는 잠금 파일을 강제한다. Rust audit의 잔여 실패를 무시하는 옵션은 추가하지 않았다.

현행 [h2 공지](https://rustsec.org/advisories/RUSTSEC-2026-0258.html)는 0.4.16 이상을 수정 버전으로 제시하지만 Actix의 0.3 의존성과 호환되지 않는다. 현재 서버는 `.bind()` HTTP/1 리스너이며 h2c/TLS HTTP/2 자동 협상을 사용하지 않는다. [RSA 공지](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)는 0.10.0-rc.18도 여전히 영향받는다고 명시한다. 운영 Rust SSH 경로는 공개키 지문 확인과 비밀번호 인증이며 RSA 개인키 연산을 노출하지 않는다. 이는 코드 사용 경로 검토이며 전체 서비스 무취약점 선언이 아니다.

russh 최신 Handler/인증 결과 API에 맞게 이전했고, host certificate는 별도 CA 정책이 없어 거부한다. 지문 불일치 시 인증 전 차단하며 기존 승인된 기한부 legacy 예외 외에는 완화하지 않는다. SSH용 ML-KEM 0.3과 age 0.12.1의 ML-KEM 0.2가 kem 버전 충돌을 일으켜 상류 age 0.11.5 수정 릴리스로 고정했다. 제품의 passphrase-only age v1 경로에 필요한 기능은 유지하고 PQ 수신자 지원은 추가하지 않는다. 복구 smoke에 별도 capture 도구 선택을 추가해 이전/새 바이너리 교차 복원을 검증할 수 있도록 했다. 운영 배포 및 최종 시험 증거는 아래에 추가한다.


배포 전 회귀 시험: backend 444개(23 library + 388 main + 5 auth + 28 integration), frontend 170개/44 suite 통과. 타입·프런트 빌드 통과, lint 오류 0/기존 경고 7. RSA 호스트 키 일치 시 인증 성공 및 불일치 시 비밀번호 전달 전 거부를 실제 SSH fixture로 검증했다. RSA/ECDSA/Ed25519 공개키 파싱 지문은 독립 OpenSSH `ssh-keygen` 결과와 같았다. 기존 Ed25519·기한부 legacy 예외·OpenSSH strict 접속 회귀도 통과했다. 배포 보존 Python 3개와 릴리스 검증 Python 4개 시험 통과.


최종 release 실행 파일에서 age 0.12.1로 만들고 0.11.5로 복원하는 방향과 반대 방향 모두 관리 데이터 복구 시험을 통과했다. 이전 복구 실행 파일은 직전 운영 배포 plan의 SHA-256과 일치함을 확인했다. 각 복원에서 실제 관리자/하위 회원 로그인·소속·권한·암호화 자격 증명 키·DB sentinel·정책 이력·실행 파일 digest를 검증했다. 독립 Go age v1.2.1과의 양방향 파일 호환, 브라우저 생성/복원 및 틀린 비밀번호/변조/절단/기존 경로 거부 시험도 통과했다.

새 TLS 라이브러리의 실제 HTTPS 수신기/브라우저 시험은 인증서·호스트명 검증, redirect 거부, 503/302 실패 표시, 암호문 저장, 재시작 후 동일 ID 재시도, 중지 및 자동 전달을 통과했다. 패키지 smoke에서 단일 HTTP/2 preface를 격리된 실제 서버에 보내 HTTP/1.1 400 거부를 확인한 뒤 정상 로그인/API를 확인했다. 이는 flood/취약점 악용 시험이나 운영 프록시 전체 검사와 다르다. 새 패키지는 잠금 파일 구성 1,377개, manifest 대상 일반 파일 28개이며 검증 후 실제 서버 실행 smoke를 통과했다.


운영 배포 완료: 2026-10-06 06:11:03 KST, `/workspace/nabiman/deployments/dependency-security-20261006-v1`.
서버 SHA-256 `c3e7e0172ca1f1d672b13dbb529282d5cfa6a875b8cec4e0dcdddd7cb46de335`, 소스 아카이브 SHA-256 `3031faedfa6c5a1113881171e4dbf91750ca823a874385f22dbd4c6e4470c128`, 검토 plan SHA-256 `a63d8ee6ab63395dc6a9910b0be0c752047db94ede2bd3bb6295ef73784f04b0`.

운영 사본 리허설 후 실제 supervisor 바이너리 3개/UI를 교체했다. 로컬·공개 HTTPS의 실행 instance/digest 일치, 공개 브라우저 가입/정렬/로그아웃 터미널 격리/회원·소속 정책/로그인 회수/감사·복구 안내/실제 터미널 입출력 회귀를 통과했다. 브라우저 오류 0건. 공개 API 토큰 시험의 범위 밖 403·회수 후 401·시험 토큰 0개를 확인했다. 공개 SBOM 1,377개와 manifest digest 및 진단 화면의 실행 파일 일치 표시를 확인했고 공식 CycloneDX 1.6 schema/format 검증을 통과했다.

교체 직전 운영 관리 데이터 사본을 새 복구 도구로 암호화·복원한 뒤 격리된 기존 runtime에서 실제 관리자 로그인과 권한 읽기를 확인했다(11.331초, 같은 호스트의 fixture 범위). 회원 1개·원격 서버 6개·암호화 자격 증명 5개·키·SQLite 무결성 및 등록 터미널 16개 fixture를 검증했다. 이 시험은 외부 서버 디스크나 외부 복구 장소까지 다루지 않는다. 복원된 인스턴스는 운영으로 전환하지 않았다.

운영 회원 권한/기존 인증 키·서버 신뢰 6개·등록 터미널 16개·tmux 17개·예약/알림/공유 이력 보존 및 SQLite quick_check 정상. 공유 JSON의 키 순서는 재직렬화로 바뀌었지만 기존 9개 레코드의 파싱된 전체 내용은 동일했다. 감사 원본 3개 파일 바이트 보존과 v2 기록 549개(검사 시점 전체 v2 범위) 별도 계산 검증을 통과했다. 과거 오류를 지우거나 전체 과거 무결성을 정상으로 바꾸지 않았다.

배포 디렉터리에 패키지·SBOM·manifest·검사기, 초기/최종 감사 원본 보고서·잠금 파일·빌드/시험 로그, browser/inventory/tokens/audit/management-recovery 검증 및 completion.json을 보관했다. h2/RSA 2개 공지와 유지보수 중단 5개는 여전히 남으며 새 plan에도 명시했다. GitHub 공개 릴리스·서명된 출처 증명·root API 분리·상용화 전체 완료로 처리하지 않는다.


## 2026-10-06 — 계정별·총량별 인증 입장 제한

[OWASP Authentication Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)의 계정 중심 실패 제한과 잠금에 의한 접근 방해 문제를 검토하고, [NIST SP 800-63B-4](https://pages.nist.gov/800-63-4/sp800-63b.html)의 온라인 인증 시도 제한을 참고했다. 이 구현으로 특정 AAL·표준 인증을 획득했다고 주장하지 않는다. 기존 제한은 socket IP별 실패 5개로 프록시 뒤 전체 계정을 함께 막았고, 만료 bucket을 전체 정리하지 않아 기록 수가 무한히 증가할 수 있었다.

새 구현은 계정별 실패/진행 중 예약과 주소별·프로세스별 총량을 분리한다. 전체 worker가 같은 bounded state를 공유한다. 계정/설정/가입 제한은 5분 5개, 주소별 총량 1분 60개, 프로세스 총량 1분 300개다. 기록은 해시 키 최대 4,096개이고 미만료 기록을 강제 퇴출하지 않는다. 동시 처리 16개와 본문 32 KiB/3초 한도를 인증 처리 전에 집행한다. 취소·오류는 시도 예산을 보존하고 동시 처리 permit은 RAII로 반환한다. 정상 인증은 자기 예약 ID만 제거해 다른 동시 실패를 지우지 않는다. 전달 헤더는 제한 주소를 바꾸지 않는다.

429는 Retry-After와 no-store를 포함하며 계정 존재 여부와 관계없이 동일한 제한 형식을 쓴다. 로그인과 가입 UI는 남은 시간 동안 제출을 막고 자동 재전송하지 않는다. 새로고침으로 UI 상태가 사라져도 서버 제한은 유지된다. 메모리 한도와 단일 프로세스 범위를 명시했으며, 재시작을 넘는 기록·다중 노드 집계·외부 IdP 정책 또는 고의적인 계정 잠금 자체의 완전한 방지는 제공하지 않는다. 인증 성공/실패 응답 전체의 timing 동일화까지 수행한 것은 아니다.

backend 451개(23 library + 395 main + 5 auth + 28 integration), frontend 172개/45 suite, 타입·lint(오류 0/기존 경고 7)·빌드를 통과했다. 새로운 시험은 교차 주소/경로 제한·대문자 변형·기본 admin 이름 생략·전달 헤더 우회 거부·다른 계정 분리·성공 시 본문 보존·가입 총량 분리·동시 예약/취소·만료·가득 찬 저장소·중복 username/큰 본문 거부·UI countdown 만료를 다룬다. 실제 실행 파일/브라우저 smoke를 CI에 추가했다. 운영 배포와 실제 서버 증거는 아래에 이어 기록한다.


최종 release 실제 서버에서 5개 소스 주소의 한 계정 실패가 공통 한도를 사용하고, 같은 프록시의 다른 정상 계정은 로그인할 수 있음을 확인했다. 5개 주소에서 합계 300개 서로 다른 존재하지 않는 계정을 시도한 후 301번째 요청은 전체 한도로 거부됐다. 이 부하·느린 본문 시험은 모두 격리된 데이터/DB/포트에서만 수행했다. 인증 본문 미완성은 3초 후 400으로 종료했고 일반 GET은 계속 정상 응답했다. 실제 브라우저 countdown/제출 차단·새로고침 후 서버 제한 유지도 통과했다. MFA enrollment/recovery, scope MFA policy, 정책 이력, 로그인 session privacy/revocation 회귀를 각각 통과했다. 패키지 smoke는 첫 실행의 임시 포트 충돌(AddrInUse)을 기록하고 다른 fixture 종료 후 동일 패키지로 재실행해 통과했다.


운영 배포 완료: 2026-10-06 06:28:17 KST, `/workspace/nabiman/deployments/auth-admission-20261006-v1`.
서버 SHA-256 `2a9025bf379108e74b128baccea3d2c5dae85d223bbc97e938823a21a4cd48f1`, 소스 아카이브 SHA-256 `1620743ab40aec2992b88ae14338341434bb50b2e397b64462f5d6073a3cf3ed`, 검토 plan SHA-256 `c5b24fdbe2261064f5d8dabd4fba7d2f831ea0fd9dac5032f59f3a51dfa90961`.

운영 사본 리허설 후 실제 supervisor를 교체했다. 로컬/공개 runtime instance/digest 일치, 공개 가입/왼쪽 정렬/로그아웃 터미널 격리/회원·소속 정책/로그인 회수·refresh replay 거부/감사·복구 안내/실제 터미널 입출력 검증을 통과했다. 별도의 공개 인증 증명은 실제 사용자와 겹치지 않는 임시 이름을 확인한 뒤 실패 5회와 429를 검증했다. 브라우저에서 남은 시간과 제출 차단, 새로고침 후 제한 유지, 정상 관리자 로그인 및 해당 시험 로그인 로그아웃을 확인했다. 실제 계정의 잘못된 비밀번호 반복이나 MFA 변경은 하지 않았고 임시 계정도 생성하지 않았다. 브라우저 오류 0건.

SBOM 1,377개와 공개 배포 명세/현재 실행 파일 digest의 일치 및 진단 화면 표시를 확인했다. 공식 CycloneDX schema/format 검증 통과. 기존 회원 권한·키·서버 신뢰 6개·등록 터미널 16개·tmux 17개·예약/알림/공유/API 토큰 기록 보존과 SQLite quick_check 정상. 감사 원본 3개 파일 보존 및 v2 레코드 691개(검사 시점 범위)의 별도 해시 계산을 통과했다. 과거 불일치는 계속 표시한다.

artifacts에 패키지/SBOM/명세/검사기를, evidence/auth-admission에 원본 시험·빌드 로그를 보관했다. auth-admission-verification.json, browser-verification.json, inventory-verification.json, audit-verification.json, completion.json을 남겼다. 대소문자만 다른 ID는 같은 제한 그룹을 사용하며 외부 LDAP 별칭 전체를 정규화하는 기능은 아니다. 메모리 기반 제한·proxy 공유 총량·특정 계정을 노린 접근 방해의 잔여 한계와 기존 RustSec 2개/유지보수 경고 5개를 유지한다. 상용 GA 전체 완료로 처리하지 않는다.

## 2026-10-06 — 로컬 OS 계정 변경 보호

보안 → 사용자 계정 화면을 나비맨 회원 관리와 구분하고, 현재 최고관리자만 OS 변경을 실행하도록 서버에서 다시 확인한다. 계정명은 소문자 POSIX 형식, 비밀번호는 제어 문자 없는 12–256바이트, 셸은 /etc/shells 목록으로 제한한다. [shadow chpasswd 매뉴얼](https://man7.org/linux/man-pages/man8/chpasswd.8.html)의 줄 단위 입력/PAM 부분 실패 특성을 반영하여 비밀번호는 단일 레코드로 표준 입력에만 전달한다. 변경 직전과 완료 후 관찰한 UID 및 NSS 일치를 검사하고 root·시스템·공유 UID 계정은 보호한다. 각 명령 실행 직전에 로그인/권한 회수를 다시 확인한다.

삭제 기본값은 홈 보존이다. [userdel 매뉴얼](https://man7.org/linux/man-pages/man8/userdel.8.html)의 -r 범위를 설명하고, 홈 삭제는 계정명 입력과 별도 선택 및 정확한 전용 홈 경로 확인이 필요하다. 기존 홈 재사용·심볼릭 링크·공유 홈 삭제를 거부한다. 보존된 파일은 UID가 재사용되면 새 계정 소유로 해석될 수 있으므로 이 한계를 UI에 명시한다. 실패 후 비밀번호를 지우고 자동 재시도하지 않으며, 생성 후 비밀번호 설정 실패 시 계정이나 홈을 자동 삭제하지 않는다. 로그인 여부를 수집하지 않으므로 알 수 없음으로 표시한다. 목록을 읽지 못하면 빈 성공 대신 503을 반환한다.

backend 461개, frontend 177개/46 suite 통과. 타입/빌드 통과, lint 오류 0/기존 경고 7. 최종 release와 실제 배포판 shadow/PAM 도구를 사설 chroot에서 실행하여 생성·실제 암호 해시 변경·기본 홈 보존·명시적 홈 삭제·잘못된 UID/줄 주입/보호 계정 거부 및 브라우저 생성/삭제를 검증했다. 호스트 passwd/shadow/group/gshadow 바이트 불변을 확인했다. 운영 계정 생성/삭제를 시험하지 않았다. CI에도 격리 시험을 추가했다.

운영 useradd/userdel은 기존 계정 보존 래퍼를 계속 사용한다. 격리 시험은 배포판 원본 도구를 사용하므로 이 운영 래퍼 및 외부 NSS/PAM/삭제 훅까지 검증한 것은 아니다. chroot는 PID/네트워크 네임스페이스 격리가 아니며 시험용 /proc/self/exe 링크는 runtime identity 검증이 아니다. 명령 대기는 30초 제한이지만 NSS/파일시스템 전체 지연 상한은 아니다. 프로세스 내부 쓰기를 직렬화하나 외부 OS 변경과의 원자적 트랜잭션, 재시작 이후 작업 상태/중복 실행 방지, UID 재사용 방지는 아직 없다. API root 실행 분리(C05)는 미완료이며 기존 취약점 잔여 2개도 유지한다. 운영 배포 결과는 아래에 기록한다.

운영 배포 완료: **2026-10-06 06:55:41 KST**, `/workspace/nabiman/deployments/os-account-safety-20261006-v1`.
서버 SHA-256 `cbfdb711bafe02ea569d5c179c5d130cd41b36482e8abc222902de1fc5a0bafb`, 소스 아카이브 SHA-256 `811ddf5a2f2c03688e2a3f7d04b6c4e68b6e4a8db80e5087e044407def24cb76`, 검토 plan SHA-256 `d3a3add73814d9a32977aa1465a1a7d603a1a10d48f5fcb4071fe303bb3ad01e`.

운영 사본 리허설과 공개 HTTPS 로그인/API를 통과한 후 실제 브라우저에서 본 서버 OS 계정 제목, root 변경 버튼 부재, 최고관리자 생성 버튼을 확인했다. 이 운영 화면 검증은 읽기만 수행했으며 OS 쓰기 요청 0건, 계정 파일 및 기존 관리 도구 바이트 변경 0건이다. 가입 입력 왼쪽 정렬·로그아웃 터미널 격리·회원/소속 관리·실제 터미널 입출력·세션 회수/refresh replay 차단·감사·복구 안내 회귀도 통과했고 브라우저 오류는 0건이었다.

기존 원격 서버 신뢰 6개·등록 터미널 16개·tmux 17개·회원 권한·키·공유/예약/알림/API 토큰 기록과 SQLite 무결성을 보존했다. 공개 SBOM 1,377개와 현재 실행 파일 digest/진단 화면 일치를 확인하고 CycloneDX 1.6 schema 검증을 통과했다. 감사 원본 3개 파일과 v2 기록 822개(검사 시점 범위)의 해시를 확인했다. 기존 과거 감사 불일치는 그대로 표시한다. 패키지/명세/시험 로그와 공개 browser/os-accounts/inventory/audit/data-preservation 증거 및 completion.json은 배포 디렉터리에 보관했다. 이번 단계 배포 완료이며 상용화 전체 완료를 의미하지 않는다.

## 2026-10-06 — HTTP 요청 경계와 WebSocket 라이브러리 강화

[Actix HTTP 변경 기록](https://github.com/actix/actix-web/blob/master/actix-http/CHANGES.md)을 검토하고 잠금 파일의 actix-http 3.12.0을 3.18.12로 갱신했다. 실제 내려받은 3.18.12 crate 소스에서 CL/TE 동시 사용·중복 Host·누락 Host 거부 및 WebSocket 프레임 검증 수정을 확인했다. 3.12.1의 요청 경계 보안 수정, 후속 WebSocket 검증, chunked trailer와 스트리밍 압축 수정이 포함된다. 변경 기록만으로 우리 실행 경로를 검증했다고 간주하지 않는다.

현재 백엔드의 HTTP/1 + WebSocket 구조를 명시하고 Actix 기본 feature 중 HTTP/2만 제외했다. 매크로·쿠키·압축·Unicode·compat·WebSocket은 유지한다. 정상 의존성 그래프에 h2가 없음을 CI에서 검사한다. [RustSec RUSTSEC-2026-0258](https://rustsec.org/advisories/RUSTSEC-2026-0258.html)의 h2 0.3.27은 actix-test/awc 경유 개발 의존성에 남아 있어 전체 lockfile 감사 결과에서 숨기지 않는다. RSA 공지와 유지보수 중단 5개도 여전히 남는다. 공개 프록시의 HTTP/2 구현을 검사하거나 제거한 것은 아니다.

패키지 smoke에 격리된 TCP 요청 8종(CL/TE 순서 양방향·중복 길이/인코딩·잘못된 TE·중복/누락 Host), 거부 이후 pipeline 미처리와 연결 종료, 정상 chunked JSON 및 trailer 로그인 검증을 추가했다. 실제 설치 대상 외부 서버나 공개 프록시에 모호한 요청을 보내지 않는다. 전체 회귀·운영 배포 결과는 아래에 기록한다.

이전 운영 패키지에 새 HTTP 시험을 적용한 결과 CL/TE 혼합 요청 뒤 후속 pipeline 요청이 처리되는 실패를 재현했다. 라이브러리만 갱신한 첫 터미널 시험은 protocol error를 애플리케이션이 무시해 연결이 유지되는 문제를 발견했다. WsBridge에서 protocol error를 받으면 1002 close를 요청하고 actor를 종료하도록 수정했다. [RFC 6455 §7.1.7](https://www.rfc-editor.org/rfc/rfc6455.html#section-7.1.7)의 오류 후 연결 종료 원칙과 [RFC 9112 §6.3](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3)의 요청 경계 규칙을 참고했다. 공격자의 원문을 close reason이나 로그에 반영하지 않는다. 정상 terminal registry/tmux 세션은 보존하고 문제가 있는 연결만 닫는다.

운영 배포 완료: **2026-10-06 07:05:18 KST**, `/workspace/nabiman/deployments/http-hardening-20261006-v1`.
서버 SHA-256 `31c3ccec079a7d589da2acb856fa26f875d9330e2c022657d9b2a1a2b76d3d24`, 소스 아카이브 SHA-256 `bb696f87549c2b4a9404119215f650a3841257ba564be3ec4d2ec3c8e8b9aa47`, 검토 plan SHA-256 `4b1ac7a3c1d2f5b8cb0f302263f047d3a8ac9ca2165483c35ccb5327eae88275`.

최종 backend 461개, 배포 보존 시험 3개, 패키지 검증 시험 4개 통과. UI 소스는 변경하지 않았다. 최종 release 격리 TCP 시험의 모호한 HTTP 8종은 400 및 연결 종료/후속 요청 미처리, 정상 chunked JSON/trailer는 로그인 성공을 확인했다. 잘못된 WS 4종(RSV·금지 close code·잘못된 UTF-8 close reason·1바이트 close)은 연결 종료와 정상 종료로 승인하지 않음을 확인했다. 유한 시험이며 RFC 전체 적합성·fuzz·부하 검증을 의미하지 않는다. 실제 브라우저 terminal 입장권/재사용/만료/권한 회수와 dashboard SSE 종료 회귀를 통과했다. 인증 admission과 HTTPS 감사 전송 회귀도 통과했다.

운영 사본 리허설 후 supervisor를 전환했다. 공개 브라우저 가입·왼쪽 정렬·로그아웃 격리·회원/소속·OS 계정 조회·MFA/감사/복구 안내·실제 terminal 입출력·세션 회수/refresh replay 검증 통과, 브라우저 오류 0건. 운영에는 모호한 HTTP/잘못된 WS 프레임 시험이나 OS 계정 쓰기를 수행하지 않았다. 기존 원격 서버 신뢰 6개·등록 terminal 16개·tmux 17개·회원 권한·키·공유/알림/예약/API 토큰 기록과 DB 무결성을 보존했다. 공개 구성 목록 1,378개와 실행 digest/진단 표시 및 CycloneDX 1.6 schema 검증 통과. 감사 원본 3개 보존 및 v2 기록 933개 해시 검증(검사 시점 범위) 통과, 과거 불일치는 유지한다.

배포 디렉터리에 이전 패키지 재현 실패, 첫 WS 시험 실패, 최종 성공, 잠금 파일 전후, RustSec 전체 보고서, 패키지/SBOM/명세 및 공개 browser/os-accounts/inventory/audit/data-preservation 증거와 completion.json을 보관했다. RustSec DB는 이전 보안 단계와 동일한 고정 사본으로 검사했으며 최신 공지를 빠짐없이 포함한다는 주장은 하지 않는다. 전체 lockfile 공지 2개(h2 개발 의존성·RSA)와 유지보수 경고 5개는 유지한다. 상용 GA 전체는 계속 미완료다.

## 2026-10-06 — 로컬 예약 작업 원문 보존과 변경 충돌 검사

기존 Cron 구현은 읽기 실패를 빈 설정으로 바꾸고, 목록에서는 환경변수 행을 제외하면서 삭제에서는 포함해 다른 행을 지울 수 있었다. 삭제 시 주석을 위로 모으면서 원문 순서도 바꾸었다. [crontab 형식 매뉴얼](https://man7.org/linux/man-pages/man5/crontab.5.html)을 참고해 환경변수·주석·공백·탭·특수 일정 행을 구분하고 삭제 대상을 실제 원문 행으로 연결했다. 추가·삭제 시 다른 행의 바이트와 순서를 유지한다. 명령은 OS 계정 권한·기존 cron 환경에서 실행되며 % 처리와 실행 시각은 시스템 cron 규칙을 따른다.

화면에서 읽은 사용자/UID/전체 원문 해시를 필수 revision으로 제출하고 변경 직전 재검사한다. 예전 revision 없는 클라이언트는 쓰기를 거부한다. 이름과 NSS 일치를 확인하고 최고관리자/현재 로그인·권한·만료를 명령 실행 직전에 다시 검사한다. 프로세스 내부 변경은 직렬화하며 취소되어도 worker가 끝날 때까지 유지한다. crontab 실행 경로는 고정이고 shell을 중간 실행하지 않는다. 출력 1 MiB·조회 명령 5초·설치 명령 10초 제한과 비동기 pipe 처리를 사용하고 stdin을 닫아 완료를 기다린다. 읽기 실패·비정상 spool·설치 실패·사후 원문 불일치를 성공이나 0건으로 표시하지 않는다.

시스템 → 예약 작업에서 기존 환경을 읽기 전용으로 검토한 뒤 실행 계정/명령을 확인해야 추가할 수 있다. 삭제도 정확한 계정·일정·명령을 표시하고 별도 확인한다. 네트워크 오류 후 자동 재시도하지 않고 원문을 다시 조회한다. 저장 성공은 향후 예약 명령 실행 성공과 구분하며, 삭제는 이미 실행 중인 프로세스를 종료하지 않는다. 나비맨 tenant 작업·/etc/cron.d·systemd timer는 이 화면 범위 밖이다. 새 명령의 control 문자/줄 주입은 거부하고 실제 일정 문법 검증은 설치된 crontab 도구에 맡긴다.

현재 root API/로컬 crontab 방식은 유지된다. 프로세스 밖의 crontab 편집과 최종 검사/설치 사이를 원자적으로 잠그지 못하며 UID 재사용·외부 NSS 변경·파일시스템 지연의 전체 상한·재시작 후 durable job/중복 차단은 미완료다. cron 없는 호스트나 다른 spool/진단 형식은 조용한 fallback 대신 명시적 오류를 반환한다. 정식 OS 지원 matrix 전체 검증을 의미하지 않는다. 실제 시험 및 운영 배포 결과는 아래에 기록한다.

최종 backend 471개(23 library + 415 main + 5 auth + 28 integration), frontend 182개/47 suite 통과. 타입·lint(오류 0/기존 경고 7)·빌드 통과. 실제 crontab 도구를 사설 chroot에서 실행하여 원문 보존·바뀐 revision 거부·잘못된 일정과 줄 주입 거부·읽기 도구 장애 시 덮어쓰기 방지·실제 브라우저 검토/확인/추가/삭제를 검증했다. cron daemon은 실행하지 않아 예약 명령 실행은 시험 범위가 아니다. 동일 fixture의 OS 계정 생성/비밀번호/삭제 회귀도 통과했다. 호스트 crontab과 passwd/shadow/group/gshadow 바이트 불변을 확인했다. 최종 패키지 HTTP 경계 smoke도 통과했다.

운영 배포 완료: **2026-10-06 07:21:01 KST**, `/workspace/nabiman/deployments/cron-preservation-20261006-v1`.
서버 SHA-256 `645e7536825e1e2a5b86a7ea7d11ff0811b61ac36afa1b7810f4511f0fca4423`, 소스 아카이브 SHA-256 `2124cc20130a03c2a4b55266967db705b17799bd6bf3748c795461c9dd915497`, 검토 plan SHA-256 `a3d825e7db05e36d7f94241c068ae02a961666486faa783ac1e81f8c5b503681`.

운영 사본 리허설에 실제 host crontab 읽기 전용 호환 검사를 추가했고 통과 후 supervisor를 전환했다. 공개 Cron API에서 기존 작업 6개와 revision을 확인했으며, 실제 브라우저에서 기존 설정 읽기·실행 확인 전 제출 차단을 검증했다. 운영 예약 작업 쓰기 요청은 0건이고 preflight부터 최종 확인까지 기존 spool 파일의 바이트가 동일했다. 원격 서버 신뢰 6개·등록 terminal 16개·tmux 17개·회원 권한·인증 키·공유/예약/알림/API 토큰 기록과 SQLite 무결성을 보존했다.

공개 가입/왼쪽 정렬/로그아웃 터미널 격리/회원·소속/세션 회수·refresh replay/감사·복구 안내/실제 terminal 입출력 회귀 통과, 브라우저 오류 0건. 공개 SBOM 1,378개와 실제 실행 digest 및 진단 화면 일치, CycloneDX 1.6 schema 검증 통과. 감사 원본 3개 파일과 v2 1,085개(검사 시점 범위) 해시를 확인했고 과거 불일치는 유지한다.

배포 디렉터리의 artifacts, evidence/cron, cron-rehearsal.json, cron-verification.json, browser-verification.json, inventory-verification.json, audit-verification.json, data-preservation-verification.json, completion.json에 증거를 보관했다. 이 단계는 로컬 사용자 crontab의 보존과 결과 확인 개선이며, 전체 특권 분리·원자적 외부 writer fencing·durable job·예약 명령 실행 관측을 완료한 것은 아니다. 상용 GA 전체는 계속 미완료다.

## 2026-10-06 — pidfd 기반 로컬 프로세스 신호

기존 구현은 ps에서 본 PID 숫자만 받아 외부 kill 명령에 넘겼다. 조회와 실행 사이 PID가 재사용되는 위험을 줄이기 위해 [pidfd_open](https://man7.org/linux/man-pages/man2/pidfd_open.2.html)과 [pidfd_send_signal](https://man7.org/linux/man-pages/man2/pidfd_send_signal.2.html)을 사용한다. 신호 전 process handle을 열고 boot ID·proc inode·시작 tick·UID·명령 바이트의 검토 해시를 확인한다. 이후 신호는 고정된 handle로 보내며 숫자 PID 방식으로 fallback하지 않는다. pidfd 기능이 없으면 쓰기 기능을 사용할 수 없다. 다른 커널/컨테이너/seccomp 조합 전체를 검증했다는 의미는 아니다.

최고관리자만 신호를 보낼 수 있고 실행 직전에 현재 로그인·MFA/권한·만료·최고관리자 상태를 재검사한다. PID 0/1, 나비맨 자신 및 읽어 확인한 상위 관리 프로세스를 보호한다. 기존 TERM/KILL/HUP/INT/STOP/CONT와 숫자 별칭을 유지한다. kernel이 신호를 수락한 것과 종료를 관찰한 것을 구분한다. 같은 pidfd를 100ms 관찰해 exited/still_present/unknown을 반환하며 정상 종료 처리·데이터 flush·서비스의 재시작/복구를 보증하지 않는다.

모니터링 → 프로세스에서 선택한 계정 UID·명령·시작 시각을 표시하고 PID를 입력해야 신호를 보낸다. 5초 목록 갱신에서 대상 해시가 바뀌면 제출을 막으며, 서버도 다시 검사한다. 네트워크 응답 유실 후 자동 재시도하지 않는다. 목록은 기존 상위 200개 제한을 유지하고 최대 100개를 표시한다. 사라지거나 읽을 수 없는 개별 프로세스는 제외하며 CPU/메모리 표본과 신호 대상 정보가 원자적 스냅샷은 아님을 UI에 표시한다. 시작 시각은 UTC, 계정은 UID로 명확히 표시한다. ps 실패·잘못된 수치·관리 ancestry 조회 실패는 빈 성공 대신 503이다.

Cron에서 사용하던 bounded command pipe 실행기를 공통 모듈로 옮겨 ps 수집에도 적용했다. 목록 수집 5초와 출력 한도를 집행한다. PID handle은 프로세스 수명을 고정하지만 프로세스의 exec/인자/credential 변경을 원자적으로 정지시키는 기능은 아니다. 재시작 후 durable intent/중복 실행 방지, 최소 특권 helper로의 전체 이전, PID namespace별 제품 지원 matrix는 후속 과제다. 실제 운영에 시험용 종료 신호를 보내지 않는다.

최종 backend 476개(23 library + 420 main + 5 auth + 28 integration), frontend 187개/48 suite, 타입·lint(오류 0/기존 경고 7)·빌드 통과. 최종 release fixture에서 직접 만든 자식 프로세스만 신호 대상으로 사용해 잘못된 해시/구형 요청 거부, 시스템/self/parent 보호, TERM 종료 관찰, TERM 무시 시 미종료 표시, KILL 완료와 실제 브라우저 PID 확인을 검증했다. 실제 PID 재사용을 강제로 발생시킨 시험이나 kernel 전체 검증은 아니다. 공통 실행기 이전 후 실제 crontab/shadow/PAM 및 브라우저 회귀를 통과했고 host 계정 파일과 cron 원문을 보존했다. 최종 패키지 HTTP 경계와 SBOM 검증도 통과했다.

운영 배포 완료: **2026-10-06 07:33:42 KST**, `/workspace/nabiman/deployments/process-identity-20261006-v1`.
서버 SHA-256 `4ce8fe7d8f53196503ddcd029cb36e83c703f905aa95f54eaec5f5495f4bc8fb`, 소스 아카이브 SHA-256 `7917a4119cad3a1b5fb851fb31fbe276105fa12daa03ab12eaea490fd3f19616`, 검토 plan SHA-256 `38b65d51b277904db164ef53494cce3cedb5c64f1a7fd876896a7c800bc80048`.

운영 사본 리허설과 실제 crontab 읽기 호환을 통과한 후 supervisor를 교체했다. 공개 프로세스 API에서 검토용 identity와 pidfd 기능을 확인했고, 브라우저에서 목록·대상 확인·PID 입력 전 제출 차단을 확인했다. 이 검증은 process 쓰기 요청 0건이며 운영 프로세스에 종료/정지 신호를 보내지 않았다. API 내부 기능 확인은 자기 pidfd에 대한 signal 0 지원 조회만 수행한다.

공개 가입/왼쪽 정렬/로그아웃 터미널 격리/회원·소속 관리/로그인 회수·refresh replay/감사·복구 안내/실제 terminal 입출력 회귀 통과, 브라우저 오류 0건. 원격 서버 신뢰 6개·등록 terminal 16개·tmux 17개·회원 권한·키·공유/예약/알림/API 토큰 기록·cron spool 바이트와 SQLite 무결성을 보존했다. 공개 SBOM 1,378개와 현재 실행 digest/진단 화면 일치 및 CycloneDX 1.6 schema 검증을 통과했다. 감사 원본 3개와 v2 1,216개(검사 시점 범위)의 해시를 확인했으며 과거 불일치는 유지한다.

artifacts와 evidence/processes에 패키지·검증기·명세·시험 로그를 보관하고 process-verification.json, browser-verification.json, inventory-verification.json, audit-verification.json, data-preservation-verification.json, completion.json을 기록했다. 전체 특권 분리·durable job·독립 보안 검토·상용 GA는 완료로 처리하지 않는다.


## 2026-10-06 — Webmin cron 실동작 비교와 배포본 재검증

Webmin 2.670 원본 tar SHA-256 `617af07a890bea18a83dca5c9d7168de1ad4d5f3076d8a1df3475f123748dc21`을 GitHub release asset digest와 대조했다. 별도 최소 chroot에 설치하고 loopback에만 열었다. root 시험 계정과 별도 spool을 사용했으며 cron daemon을 시작하지 않았다. `nostart`, `nopostinstall`, `nouninstall`을 적용하고 설치 후 gray-theme로 제한된 모듈 비교를 수행했다. 완전한 지원 배포판 설치나 기본 테마 사용성 비교는 아니다.

동일 seed를 사용한 일반 삭제·외부 주석·선택 행 앞 작업 삽입을 각각 3회 반복했다. 일반 삭제 시 두 제품 모두 나머지 바이트를 보존했다. 외부 주석은 Webmin이 보존하면서 삭제했고 나비맨은 409로 원문 전체를 유지했다. 삽입 시 Webmin에서는 새 작업이 삭제되고 원래 선택 작업이 남았다. 나비맨은 서버와 브라우저 제출 모두 409로 두 작업을 보존했다. [벤치마크 기록](benchmark.md)에 범위와 한계를 기록했다.

`scripts/cron_fixture.py`에 선택 행 앞 삽입 부정 시험을 추가했고 `frontend/scripts/smoke-cron.js`에서도 실제 삭제 확인 화면을 연 뒤 private chroot crontab을 변경해 검증한다. 시험 경로는 `/tmp/nb-os-account-root-*`만 허용하며 원문을 finally에서 복원한다. 운영 바이너리 해시 `4ce8fe7d8f53196503ddcd029cb36e83c703f905aa95f54eaec5f5495f4bc8fb`로 실제 shadow/PAM/crontab/브라우저 묶음을 3회 통과했다. 각 실행 동안 host 계정 파일과 cron spool 바이트는 유지됐다.

운영 공개 cron API/UI도 재확인했다. 예약 작업 6개, 쓰기 요청 0건, 브라우저 오류 0건, 기존 배포 preflight 이후 spool 원문 보존을 확인했다. 제품 런타임 변경 없이 시험 코드와 조사 기록만 추가한 단계이므로 07:33:42 KST의 `process-identity-20261006-v1` 배포를 유지했다. 이전 배포 아카이브를 새 시험 코드가 포함된 것처럼 갱신하지 않는다.

원본 증거 22개와 해시 목록을 `/workspace/nabiman/benchmarks/webmin-cron-20261006-v1/`에 보관했다. fixture 인증정보는 증거 묶음에 포함하지 않았다. 전체 제품 비교·독립 보안 검토·GA는 계속 미완료다.

## 2026-10-06 — 예약 작업 편집

Webmin의 [Scheduled Cron Jobs](https://webmin.com/docs/modules/scheduled-cron-jobs/)는 기존 작업의 시간·명령 편집을 지원한다. 나비맨은 기존 UI가 추가·삭제만 제공해 수정하려면 작업을 지워야 했다. `/api/cron/update`와 시스템 → 예약 작업의 수정 화면을 추가했다. 실행 계정과 물리적 행 번호·검토 revision을 고정하고 해당 행만 교체한다. 환경변수·주석·중복 작업·주변 행 순서를 유지한다. 일반 다섯 시간 필드와 표준 @reboot/@hourly/@daily/@midnight/@weekly/@monthly/@yearly/@annually를 처리하며 실제 crontab 도구가 최종 문법을 검증한다.

화면은 수정 전 작업, 원문 전체, 저장할 작업을 보여준다. 현재 원문의 revision이 목록에서 선택한 작업과 다르면 검토를 수락하지 않는다. 입력을 변경하면 확인 체크를 해제한다. 기존 변경 경로의 최고관리자·현재 로그인·만료·MFA·NSS/UID 재검사, 쓰기 permit, 실행 시간/출력 제한, 사후 읽기 검증을 공유한다. 구형 revision 없는 요청과 알 수 없는 필드는 거부한다. 이미 실행 중인 작업은 수정하지 않으며 외부 writer에 대한 원자적 fencing, durable job, 실행 관측은 포함하지 않는다.

서버 테스트 479개(23 library + 423 main + 5 auth + 28 integration), 프런트엔드 189개/48 suite와 타입 검사·UI 빌드가 통과했다. lint 오류 0개, 기존 경고 7개다. 실제 crontab 검증과 운영 배포 결과는 완료 후 아래에 기록한다.


운영 배포 완료: **2026-10-06 07:57:29 KST**, `/workspace/nabiman/deployments/cron-edit-20261006-v1`.
서버 SHA-256 `b5ac9e39665a454bb2143f35fe1c5abe0d154b7b5b3714c3224606de82459a95`, 소스 아카이브 SHA-256 `b7350db8691ba745fae8e6f4ce4e0c97be5a5a933609a957070b29e2dd84524d`, 검토 plan SHA-256 `8a116ff1c94a34177d58114dcaddb9259a2375cd15a0d3b230114991db29c2b6`.

명령 마지막의 공백을 파서가 제거하던 부분도 수정했다. 시간만 수정할 때 셸의 escaped space를 포함한 명령 바이트를 유지하는 단위 시험과 실제 crontab 읽기·수정 시험이 통과했다. 최종 릴리스의 private chroot에서 실제 shadow/PAM/crontab과 브라우저 추가→수정→삭제, @예약어, 잘못된 문법·대상·구형 요청·stale revision 거부, 외부 삽입 보존을 검증했다. 이 시험에서 host 계정 파일과 cron spool 원문을 보존했다. 패키지 HTTP framing과 SBOM CycloneDX 1.6 검증도 통과했다.

운영 사본 리허설을 통과한 후 배포했고 공개 화면의 수정 전·후/기존 환경 검토와 확인 전 저장 차단을 검증했다. cron 쓰기 요청 0건, 예약 작업 6개, 기존 spool 바이트 보존이다. 공개 가입·왼쪽 정렬·로그아웃 터미널 격리·회원 관리·세션 회수·refresh replay·감사/복구 안내·실제 terminal 입출력 회귀가 통과했고 브라우저 오류 0건이다. 원격 서버 신뢰 6개·등록 terminal 16개·tmux 17개·회원 권한·키·terminal 공유·예약/알림/API 토큰 설정과 SQLite 무결성을 유지했다.

공개 SBOM 1,378개와 진단 화면/현재 실행 digest가 일치했다. 감사 원본 파일 3개 및 v2 1,369개(검사 시점 범위)를 확인했으며 과거 불일치는 보존했다. artifacts와 evidence/cron-edit에 패키지·시험 로그·검증 명세를 보관했다. cron-verification.json, browser-verification.json, inventory-verification.json, audit-verification.json, data-preservation-verification.json 및 completion.json은 이 단계의 증거다. 전체 특권 분리·durable job·독립 보안 검토·상용 GA는 계속 미완료다.


## 2026-10-06 — 특권 실행기의 root 정책 철회

특권 helper의 정책이 시작 시점에만 로드되는 경계를 확인했다. 실행 직전 `Policy::load`의 root 소유권·0600·보호된 부모 경로·비symlink·크기/JSON 검증과 승인 snapshot hash 대조를 추가했다. root가 승인 파일을 철회·삭제·손상시키거나 안전하지 않은 권한으로 바꾸면 재시작 전에도 새 OS command는 실행하지 않는다. 이미 저장한 요청은 `not_dispatched`로 기록하고 조회할 수 있으며 같은 ID는 재실행하지 않는다. 포맷만 바뀐 동일 정책이나 정확히 복구한 승인 정책은 새 ID를 허용한다. 새 권한 채택은 helper 재시작이 필요하다.

실제 helper/비특권 API/SQLite commit 경계 시험과 최종 패키지 배포 결과를 완료 후 아래에 기록한다. 현재 운영의 helper는 systemd 없는 supervisor 환경에서 비활성 상태를 유지한다. 이 변경은 배포 패키지의 특권 실행기 개선이며 운영 API의 전체 비특권 전환을 완료한 것은 아니다.
