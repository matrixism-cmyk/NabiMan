# 계정 터미널 일회용 접속권과 접속 회수

2026-10-05 개발 구현, 같은 날 23:48 KST man.bebo.kr 운영 반영. 공개 HTTPS에서 실제 ticket 기반 terminal 입출력을 확인했다. C09의 접속 인증·회수 기반이며 승인·전체 기록·OS identity 격리까지 완료한 것은 아니다. 공유 링크의 수명·owner ID·진행 중 회수와 저장 경계는 [공유 터미널 문서](shared-terminal-access.md)에 별도로 기록한다.

## 설계 근거와 적용 범위

[OWASP WebSocket Security Cheat Sheet](https://github.com/OWASP/CheatSheetSeries/blob/master/cheatsheets/WebSocket_Security_Cheat_Sheet.md)는 query token의 access log 노출과 연결 이후에도 계속되는 session 검증 문제를 다룬다. [OWASP Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)는 URL에 포함한 session identifier의 노출과 CSPRNG 생성을 설명한다. [Teleport 인증 구조](https://goteleport.com/docs/reference/architecture/authentication/)의 짧은 수명 credential과 live identity/session locking도 참고했다.

나비맨에 적용한 것은 **현재 계정 session에 묶인 자체 일회용 admission ticket**이다. Teleport의 CA·SSH 인증서·OS 사용자 격리·접속 승인·독립 기록 저장을 구현하거나 대등함을 입증한 것은 아니다. 경쟁 제품에 대한 자료 검토와 나비맨 구현의 자체 시험이며 경쟁 제품 설치·성능 비교 결과는 없다.

## 접속 흐름

1. 브라우저는 JWT를 Authorization header에 넣고 `POST /api/terminal/tickets`에 접속 query를 전송한다. 공통 refresh client를 사용하므로 만료된 access token의 갱신도 URL에 비밀을 추가하지 않는다.
2. 서버는 공통 JWT/session/현재 회원 인가를 통과한 요청만 허용한다. 현재 전역 최고관리자만 계정 터미널을 사용할 수 있는 기존 경계를 유지한다. 서버·tenant 소속 관리자의 root 터미널 접근을 새로 열지 않는다.
3. OS CSPRNG에서 32 byte의 opaque ticket을 생성하고 digest만 process memory의 cross-worker 공유 map에 저장한다. record에는 username·현재 session ID·원래 접속 query·Origin과 monotonic 만료 시각을 보관한다. 응답은 Cache-Control: no-store다.
4. 접속권은 최대 30초이며 발급에 사용한 access JWT 만료 시각을 넘지 않는다. 브라우저는 JWT 없이 `GET /api/terminal?...&ticket=<opaque>` WebSocket에 접속한다. 접속 대상·세션·history·geometry 등 원래 query 순서와 bytes를 유지한다.
5. 서버는 ticket을 원자적으로 제거하여 사용한다. 만료·다른 query·Origin·재사용은 거부한다. 현재 session의 존속·username과 현재 회원 상태·실제 역할을 다시 검사한다. 예전 JWT의 admin claim 또는 X-User header를 현재 권한으로 사용하지 않는다.
6. 실제 WebSocket upgrade를 확인한 뒤에만 tmux 생성·attach로 진행한다. 일반 HTTP GET이 shell을 생성하는 경로를 제거했다.

접속 query는 알려진 terminal 필드만 받으며 중복 key·unknown key·token·ticket·password·control character를 발급 요청에서 거부한다. 별도 ssh_host 연결도 기존 등록 서버와 host trust를 실제 연결 시 다시 확인하는 기존 경로를 사용한다. ticket 발급 자체가 원격 서버 생존·host key·tmux session 존재를 증명하지는 않는다.

Origin이 발급 요청에 있으면 WebSocket에도 정확히 일치해야 한다. Origin 없는 native client는 Origin 없는 접속권을 사용하거나 기존 Authorization header WebSocket을 사용할 수 있다. 이는 전역 CORS·reverse proxy Origin allowlist를 새로 완성했다는 뜻은 아니다. HTTP Origin 설정을 의도적으로 바꿀 수 있는 native client의 동일성을 Origin만으로 증명하지 않는다.

## 상한과 재연결

유효 미사용 접속권은 login session당 8개, process 전체 4096개 상한이다. 초과 요청은 429이며 유효한 다른 접속권을 eviction하지 않는다. CSPRNG 실패는 503이고 insecure fallback은 없다. 만료된 record는 새 발급 때 정리한다. 시작·재시작 시 map은 비어 있으며 미사용 접속권이 이어지지 않는다. persistent job·DB lock·tmux session과 다른 수명이다.

브라우저는 재연결마다 새 접속권을 발급한다. 이전 pending 요청을 취소하고 늦은 응답을 연결 generation으로 무시한다. history 조회 실패는 새 연결을 막지 않으며 접속권 발급 실패는 제한된 backoff 재시도로 처리한다. Policy close code 1008은 무한 재연결하지 않으며 한국어·영어·일본어로 session 종료 또는 접근 회수를 안내한다. 창 분리·재접속·unmount 뒤 늦은 socket callback이 이미 정리한 xterm을 다시 쓰지 않도록 차단한다. 익명 공유 링크의 기존 접속권 경로는 계정 접속권으로 바꾸지 않았다.

## 이미 열린 연결 회수

계정 `WsBridge`는 authenticated username·login session ID·session store·user store로 만든 ConnectionGrant를 보관한다. 각 수신 메시지 처리 전에 현재 권한을 검사하고, idle 연결도 1초 주기로 검사한다. 대시보드·작업 SSE도 같은 grant로 실제 GET endpoint의 현재 권한을 검사한다. SSE는 각 데이터 전달 전 및 유휴 1초 주기에 검사해 회수된 연결을 닫는다. 터미널의 최고관리자 제한을 SSE에 그대로 적용하지 않고 기존 endpoint의 읽기 권한을 사용한다. 로그아웃·session revoke·사용자 삭제/정지·최고관리자 역할 회수 또는 7일 된 login session은 close code 1008로 계정 연결을 끊는다. 실제 탐지 시간은 runtime scheduling에 영향을 받으므로 하드 실시간 1초 SLA를 주장하지 않는다.

이것은 browser attachment와 앞으로 받는 input의 회수다. 이미 OS에 전달한 입력·시작한 명령·백그라운드 프로세스를 취소하거나 tmux session 자체를 삭제하지 않는다. 검사와 OS write 사이에 완전한 transaction cutoff가 있는 것도 아니다. 강제 작업 종료·OS identity별 session teardown·권한 version의 실행기 전달은 다음 특권/중앙 인가 단계에서 다뤄야 한다. 공유 연결은 별도의 ShareGrant로 현재 링크·불변 owner·대상·수명을 검사하고 진행 중 접속을 회수하도록 보강했다. 공유도 긴 서명 proof를 제거하고 별도의 30초 일회용 소비·Origin/query binding·시도/연결 상한을 구현했으며 상세 계약은 공유 문서를 따른다.

## 공통 API 갱신과 로그인 전환

2026년 10월 5일 공통 `fetchWithRefresh`의 갱신·재시도를 로그인 generation과 refresh credential에 묶었다. 이전 요청의 401이 다른 계정 로그인 후 도착해 새 계정으로 변경 요청을 재전송하거나, 이전 refresh 응답이 새 credential을 덮어쓰는 경로를 차단한다. login/logout은 generation을 바꾸며 같은 session의 access token 갱신은 유지한다. 새 login에 refresh token이 없으면 이전 credential을 남기지 않는다.

같은 session의 동시 401은 하나의 refresh를 공유하고, 이미 갱신이 끝난 뒤 도착한 401은 그 session의 새 access token으로 한 번만 재시도한다. 원래 method·body·idempotency key를 유지한다. session이 바뀌었으면 갱신·재시도·credential 저장·실패에 의한 logout을 진행하지 않고 `AbortError`로 종료한다. `apiRequest`·`apiPost`·`apiDelete`와 `useApi`는 JSON 읽기 후에도 generation을 확인한다. 서비스 작업 hook은 계정 변경/unmount 뒤 늦은 receipt·오류·refetch도 무시한다.

신규 unit 11건은 동시 갱신, 늦은 401/성공/실패/JSON, 동일 credential 문자열 재로그인, 이미 갱신된 token의 재사용, abort·최대 한 번 재시도와 refresh 누락을 검증한다. 서비스 화면 시험은 pending receipt 중 계정 변경을 검증한다. 실제 Chromium fixture는 별도 실제 최고관리자 session의 요청/refresh 응답을 지연시킨 뒤 UI에서 logout하고 승인된 viewer로 로그인한다. 늦은 401·실제 refresh 성공·refresh 실패의 세 경우 모두 새 credential과 `/api/users/me`의 viewer identity를 유지하고 원래 서비스 POST가 한 건이며 helper ledger에 새 명령이 없음을 확인한다.

이는 [OWASP session lifecycle 원칙](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)을 참고한 나비맨의 browser race 처리다. 서버의 현재 계정 인가·SID 회수를 대체하지 않는다. `AbortError`는 오래된 결과를 버린다는 client 상태이며 이미 서버가 받은 변경의 취소나 rollback을 뜻하지 않는다. raw fetch·직접 stream/body 소비·외부 인증 adapter 전체의 lifecycle 통합, 서버 session의 immutable account ID 전환과 분산 회수는 남은 범위다.

## 대시보드와 다운로드

`useDashboardStream`은 Authorization header를 제공하는 fetchWithRefresh만 사용하며 URL token을 제거했다. 파일 다운로드도 새 창에 JWT query를 전달하는 방식에서 같은 header client로 응답 blob을 내려받는 방식으로 바꿨다. 다운로드 응답에도 Cache-Control: no-store를 적용한다. 기존 server의 50MB 한도는 유지한다. 대용량 streaming·미리 서명한 object download·download ticket/사용량 정책까지 구현한 것은 아니다.

## 운영·migration과 잔여 항목

ConnectionGrant는 인증 middleware가 검증한 현재 회원의 불변 ID를 보관한다. 열린 terminal·SSE에서 현재 username에 해당하는 회원의 ID가 바뀌면 같은 SID·이름만으로 재인가하지 않는다. 빈 ID는 grant를 만들거나 유지할 수 없다. JWT·ticket wire format 자체에 새 ID 필드를 추가한 것은 아니며 입장 전 기존 SID와 현재 회원 검증 계약을 유지한다.

terminal metadata에는 같은 ID의 `owner_id`를 발행한다. 재접속·목록·scrollback·관리 변경·공유는 표시 이름 대신 이 ID로 소유권을 확인한다. account shared flag도 소유자의 현재 terminal 정책이 유효할 때만 사용할 수 있다. 요청한 session의 ownership가 없거나 legacy ID가 누락됐으면 안내/END로 끝내며 새 local shell로 fallback하지 않는다. 기존 metadata를 현재 이름으로 자동 adoption하지 않으므로 [저장소와 legacy 이전 계약](terminal-registry.md)을 운영 rollout 전에 검토해야 한다.

새 frontend와 backend를 함께 배포해야 한다. 예전 `/api/terminal?token=<JWT>` 방식은 401로 거부한다. 자동 호환 fallback을 두지 않는다. REST 회원·tenant API와 Authorization header를 사용하는 native WebSocket은 계속 같은 verified identity 경계를 사용한다.

짧은 접속권도 소모 전에는 bearer secret이다. WebSocket query에서 access JWT는 제거했지만 query secret 노출을 완전히 없앤 것은 아니다. 외부 proxy access log·APM·debug log의 ticket query redaction, HTTPS/WSS와 신뢰 proxy 구성, request body·header logging 정책은 실제 배포에서 검증해야 한다. ticket을 탈취한 공격자가 만료/정상 접속 전에 같은 target·Origin으로 경쟁하는 것을 완전히 막는 기기 증명은 없다.

map은 동일 process의 worker에서만 공유한다. 여러 replica 사이 원자적 소비와 sticky routing·분산 저장, 외부 auth session 수명·사용량 rate limit, session approval/JIT·2인 승인·per-session MFA, recording·독립 audit·retention·privacy, scoped OS identity, 공유 owner version의 분산 전달·백업 복구 인가, 실제 운영 proxy·패키징·upgrade 검증과 규모 시험은 남아 있다. 기존 최고관리자의 root 권한을 별도 helper로 분리한 것도 아니다.
