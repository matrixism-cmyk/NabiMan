# 공유 터미널의 수명과 진행 중 접속 회수

2026-10-05 개발 소스, 같은 날 23:48 KST 검증 코드를 man.bebo.kr에 반영했다. 기존 공유 링크 9개는 모두 이미 만료돼 있었으며 보존하되 재활성화하지 않았다. 공유 접속은 계정 터미널과 별도의 bearer capability이며, 계정 인증·MFA·개인별 OS identity를 대체하지 않는다.

## 적용한 경계

새 링크는 OS CSPRNG의 32 byte를 64자 hex로 발급한다. 난수 생성 실패에 대체 난수를 사용하지 않는다. 수명은 1초~30일이고 무기한 링크를 거부한다. API에서 read_only를 생략하거나 UI에서 새 form을 열면 읽기 전용이다. 편집 가능한 공유는 명시적으로 선택해야 한다. 기존 tmux read-only attach와 input 차단을 함께 유지한다.

링크에는 발급한 최고관리자의 불변 local user ID와 username, 발급 시점의 계정 승인 version, 정확한 terminal session ID, password hash, read-only와 만료를 저장한다. public metadata는 password hash와 owner ID를 제공하지 않는다. owner 목록·생성·회수와 public metadata/password 확인 응답에 no-store를 적용한다. 기존 링크에 불변 owner ID 또는 승인 version이 없으면 이름으로 추정하거나 자동 이전하지 않는다. 무기한 링크와 만료된 링크도 접속을 허용하지 않는다. 소유자 목록에는 사용 불가 상태를 표시하고 필요하면 삭제·재발급한다.

`ShareGrant`는 현재 링크, 발급 snapshot, 현재 활성 최고관리자의 ID·username과 terminal 정책, 현재 terminal의 불변 owner_id·tmux_name을 확인한다. 표시 이름만 같은 terminal이나 owner_id 없는 legacy terminal을 링크에 연결하지 않는다. 링크 취소·만료·정책 내용 변경·owner 삭제/정지/역할 회수·세션 제거/대상 변경은 기존 연결을 invalid로 만든다. 서버 시간의 now가 created보다 이전이거나 expiry 이상이면 거부한다. 완전한 신뢰 clock·분산 revocation은 구현한 것이 아니다. 링크의 수명은 소유자의 특정 로그인 SID와 독립적이므로 소유자의 로그아웃만으로 모든 발급 링크를 자동 취소하지 않는다.

새 링크 생성은 현재 회원 ID와 session owner_id를 비교한다. 소유자 목록·회수는 동일한 불변 link owner ID를 기준으로 하므로 표시 이름 변경이 다른 사람에게 소유권을 넘기지 않는다. username이 현재 계정과 다른 과거 링크는 기존 live-link 검사와 계정 승인 version에 따라 입장을 거부한다. owner ID 없는 legacy link는 기존 이름에 해당하는 최고관리자에게 사용 불가 목록과 삭제만 허용하며 정상 새 발급으로 승격하지 않는다. public metadata의 alive/grid, 소유자 목록의 usable과 마지막 WS 입장도 terminal owner ID를 확인한다. [세션 저장·legacy 이전 계약](terminal-registry.md)을 함께 적용해야 한다.

실제 WebSocket handshake를 확인한 뒤만 attach를 진행한다. `WsBridge`는 시작, 각 수신 메시지, history/geometry 전달, PTY output frame 이전과 유휴 1초 주기에 현재 grant를 검사한다. 한 PTY poll을 최대 16×8192 byte로 제한해 지속 출력이 actor를 무한 점유하지 않도록 했다. invalid면 1008 policy close를 전송하고 attachment를 종료한다. 브라우저는 해당 close에 자동 재접속하지 않는다. 검사와 OS write 사이의 완전한 원자 cutoff, 이미 큐에 든 네트워크 frame의 회수 또는 하드 실시간 SLA를 보증하지 않는다.

회수는 공유 attachment와 앞으로 처리할 입력/출력을 차단한다. 이미 전달된 명령·화면·브라우저 버퍼를 되돌리지 않으며 owner shell·tmux session 자체나 다른 링크를 삭제하지 않는다. 전체 작업 취소·OS principal별 teardown은 별도 특권/인가 설계의 범위다.

## 소유자의 승인 version과 재승격

계정의 `share_authorization_version`은 서버가 생성하는 UUID이며 users.json에 저장하고 사용자 API 응답에는 포함하지 않는다. 링크에는 발급 당시 version을 저장한다. 현재 계정이 활성 최고관리자여도 링크 version과 일치하지 않으면 metadata·비밀번호 확인·접속권 소비·live attachment를 거부한다. 소유자 목록에는 기존 링크를 사용 불가로 표시하므로 검토 후 삭제하거나 새로 발급할 수 있다. 기존 링크의 version을 현재 값으로 자동 갱신하지 않는다.

중앙 계정 저장 경로에서 username·role·status·password hash·TOTP 활성 상태 또는 secret 변경 시 version을 교체한다. 강등·정지 뒤 재승격·재활성화해도 이전 version으로 돌아가지 않는다. 비밀번호·MFA 변경도 이전 공유 승인을 무효화한다. last_login이나 소속 bookkeeping만 바뀌면 공유를 일괄 취소하지 않는다. 로그아웃은 이 version을 교체하지 않는다. 변경한 계정과 version은 같은 users.json publication에 저장하며 파일 교체 이전 실패는 이전 계정 상태와 version을 함께 복구한다. 파일 교체 뒤 directory fsync 실패에는 결과를 확정할 수 없으므로 이전 메모리로 되돌리지 않고 서버 process를 종료한다. 새 링크 발급에서는 현재 계정 version을 다시 검사하고 account mutex를 publication까지 유지해 중간 권한 변경을 오래된 승인으로 발급하지 않는다.

기존 계정에 version이 없으면 시작 시 생성해 저장한다. 기존 공유에 없는 version을 이름이나 현재 권한으로 채우지 않으며 재발급을 요구한다. 성공적인 정상 재시작에서는 새 version을 유지한다. users.json 저장은 file fsync·atomic rename·필수 parent directory fsync 뒤 성공으로 처리한다. rename 뒤 sync 실패는 exit 78이며 연결과 실행 중 요청이 끊어질 수 있다. 이때 mutation 결과는 불명확하므로 자동 재시도하지 않고 디스크의 계정 상태·version과 감사/작업 기록을 확인한다. 재시작은 관측한 regular users file과 directory를 sync하고 형식·계정 목록을 검증한 뒤 수용한다. 지속적인 sync 실패에서는 시작하지 않는다. 실제 전원 차단/장치 cache 내구성·상위 directory 생성의 crash durability·여러 서버의 계정 변경·mixed-version writer·수동 파일 편집·오래된 users/share 백업의 동시 복구를 통한 권한 부활까지 해결한 것은 아니다. 운영 복구는 오래된 공유 승인 재사용을 허용하지 않도록 별도 검토해야 한다.

## 일회용 공유 접속권과 자원 상한

기존 12시간 재사용 digest proof를 제거했다. 암호가 있는 링크와 없는 링크 모두 `POST /api/share/{token}/auth`에 `password`와 검토한 `query`를 보내 먼저 접속권을 발급받아야 한다. query는 빈 값 또는 중복 없는 숫자 cols/rows만 허용한다. 비밀번호는 최대 72 UTF-8 byte이며 새 링크 생성과 확인 양쪽에서 제한해 bcrypt truncation에 의존하지 않는다. 실패는 현재 링크·비밀번호를 확인하라는 403이고, 잘못된 입력은 400이다. 비밀번호 검증은 event loop 밖에서 실행하며 완료 뒤 현재 owner와 링크의 승인 snapshot을 다시 확인한다.

접속권은 OS CSPRNG의 256-bit opaque token이고 digest만 process memory에 저장한다. 유효기간은 최대 30초·현재 링크 만료 이전이며 monotonic clock으로 검사한다. link ID·owner ID·session ID·password hash·read-only·created/expiry·정확한 query bytes·Origin을 연결한다. WS의 단 한 번의 원자 소비에서 다른 링크·query·Origin은 거부하며 다시 사용할 수 없게 한다. duplicate ticket와 unknown parameter도 거부한다. 소비 뒤 실제 handshake, 현재 승인 snapshot과 terminal tuple을 다시 검사한다. 유효 접속권을 일반 HTTP GET에 쓰면 소비 후 400이며 셸을 만들지 않는다. 계정 접속권과 map이 분리되어 서로의 인증에 사용할 수 없다.

발급 POST에 Origin이 있으면 WS와 정확히 같아야 하며 Origin 없는 native POST는 Origin 없는 WS에서만 소비한다. 이는 글로벌 Origin allowlist나 기기 증명을 구현했다는 의미가 아니다. 짧은 접속권도 소비 전에는 bearer secret이고 같은 target/Origin을 제시하는 탈취자와의 경쟁을 완전히 막지 않는다. HTTPS/WSS와 proxy redaction이 필요하다.

미사용 접속권은 링크당 8개·process 전체 4096개다. 초과 시 유효한 다른 접속권을 eviction하지 않는다. 시작/재시작 때 map이 비므로 이전 접속권은 다시 쓸 수 없다. 기존 서명 proof와 ticket 없는 WS를 허용하는 호환 fallback은 없다. 30초는 입장권의 수명이며 이미 연결된 세션의 최대 실행 시간은 아니다. 연결 중에는 앞 절의 현재 링크·owner·대상·수명 검사를 계속한다.

발급 시도를 링크별 30건·실제 TCP peer IP별 60건·process 전체 240건의 60초 고정 window로 제한한다. 정상·잘못된 비밀번호·존재하지 않는 링크·busy 요청도 해당 시도에 포함한다. 임의 X-Forwarded-For를 IP identity로 믿지 않는다. proxy 뒤에서는 여러 사용자가 같은 peer bucket을 사용할 수 있으므로 실제 고객 환경에서 수치와 신뢰 proxy를 검증해야 한다. in-memory bucket 수도 8192 상한이며 활성 bucket을 밀어내지 않는다. CPU verification은 동시에 4개, live 공유 attachment는 링크당 8개·process 전체 64개다. attach 전 atomic lease를 잡고 실패·actor 종료에 반환한다. 초과는 no-store·Retry-After 60의 429다. 이 숫자는 현재 구현의 기본 안전 상한이며 측정한 제품 성능·상품 한도·SLA가 아니다.

브라우저는 접속·재접속마다 새 입장권을 사용한다. gate에서 받은 첫 입장권은 한 번만 전달하고, 이후에는 다시 발급한다. 공유 요청은 계정 JWT header·refresh·cookie를 사용하지 않는다. 재접속을 위해 입력한 암호는 해당 문서의 memory ref에만 보관하고 입력칸을 비우며 문서 정리 때 참조를 제거한다. password와 입장권을 local/session storage·cookie에 저장하지 않는다. socket query에는 짧은 입장권만 넣고 password는 URL에 넣지 않고 POST body에만 보낸다. JavaScript memory의 물리적 zeroization을 보증하지 않으며 XSS 방어·request-body logging 정책은 별도 운영 과제다. 발급 거부·429·503은 자동으로 password endpoint를 반복 호출하지 않고 명시적인 재접속을 기다린다. 다른 링크로 문서가 바뀌면 이전 승인 상태를 이어받지 않는다.

## 저장과 장애 경계

`terminal_shares.json`은 process worker 전체에서 같은 mutex registry를 사용한다. 안정적인 companion `terminal_shares.lock` inode의 exclusive nonblocking flock을 서버 수명 내내 유지한다. 같은 registry를 새 구현의 두 process가 동시에 소유하지 못한다. 파일 형식·현재 map 상한 4096건·16MiB 파일 상한을 검증하고 손상 데이터를 빈 registry로 대체하지 않는다. 시작 검증은 auth 초기화 전에 진행한다.

변경은 clone한 다음 0600 임시 파일에 쓰고 file fsync → atomic rename → parent directory fsync 후 in-memory snapshot을 교체한다. 저장 실패는 성공 receipt를 반환하지 않는다. rename 뒤 directory fsync 실패처럼 디스크 결과가 불명확할 수 있으므로 실패한 registry 전체를 process 내에서 unavailable로 만들고 기존 공유 grant도 거부한다. 후속 API는 503이며 이전 cache로 계속 허용하지 않는다. 운영자는 원인과 마지막 저장 상태를 확인한 뒤 복구·재시작해야 한다. 자동으로 이전 파일을 되살리거나 회수를 취소하지 않는다. 이는 단일 writer 계약이며 HA·replica failover·distributed lease/fencing이 아니다.

last_used는 정보 제공용으로 memory에서 갱신하며 후속 보안 변경 시 함께 저장될 수 있다. 모든 attach를 durable 보안 감사로 기록하는 저장 계약은 아니다. 외부 파일 편집과 구버전의 직접 파일 쓰기는 새 registry의 승인된 mutation 경로가 아니며 mixed-version writer를 지원하지 않는다.

## 감사와 운영 이전

공통 HTTP 감사에서 `/api/share/{bearer}/...`와 `/api/terminal/share-links/{bearer}`는 bearer 자체 대신 SHA-256의 128-bit correlation ID를 저장한다. 정상·실패 요청과 소유자의 취소 요청에 같은 correlation을 사용하고 query를 기록하지 않는다. unknown suffix에도 원문을 남기지 않는다. 이 변경은 새 이벤트부터 적용하며 기존 hash-chain 기록을 다시 쓰지 않는다. reverse proxy·APM·외부 access log의 URL/path/query redaction과 실제 HTTPS/WSS는 별도 운영 검증 대상이다.

배포는 이전 server를 중지한 뒤 frontend/backend를 함께 교체하고 열린 구버전 browser tab도 갱신한다. registry·auth·terminal data를 보존하고 복구 절차를 준비해야 한다. 기존 owner ID 또는 승인 version 없는 링크는 배포 후 사용할 수 없으므로 최고관리자가 검토해 새로 발급한다. 구버전의 긴 서명 proof와 ticket 없는 WS도 거부하며 72-byte 범위를 넘는 기존 공유 password는 재발급 정책이 필요하다. 구버전으로 되돌리면 현재의 공유 회수·owner ID·일회용 경계가 사라지므로 단순 바이너리 rollback을 안전한 보안 복구로 간주하지 않는다. 현재 운영 서비스와 실제 서버 세션은 이 개발 단계에서 변경하지 않았다.

## 근거와 남은 기능

[OWASP WebSocket Security](https://github.com/OWASP/CheatSheetSeries/blob/master/cheatsheets/WebSocket_Security_Cheat_Sheet.md)의 장기 연결 재인가·만료 종료·인증 비밀 로그 제외와 [Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)의 서버 측 수명 집행을 참고했다. 공식 지침 조사와 자체 구현·시험이며 경쟁 제품 실측이나 인증 결과가 아니다.

단일 서버의 정상 계정 변경과 재시작에서는 이전 공유 승인이 재승격으로 부활하지 않도록 version을 연결했다. 실제 전원 장애·백업 복구·분산 계정 저장 경계는 추가 설계와 검증이 필요하다. 2인 승인/JIT/MFA·사용자별 recording·독립 감사·retention/privacy·OS identity·외부 rate/connection 관측·HA·규모/SLA와 운영 proxy 시험은 남아 있다. 이번 경계의 검증만으로 공유 접근 전체나 나비맨의 상용 출시를 완료 처리하지 않는다.
