# 터미널 세션 저장과 상태 확인

2026년 10월 5일 개발 구현, 같은 날 23:48 KST man.bebo.kr 운영 반영. 기존 16개 terminal은 과거/현재 회원 ID와 등록 target·기존 known_hosts를 검토한 명시적 배포 계획으로 이전했으며 앱의 이름 기반 자동 adoption은 추가하지 않았다. Root tmux의 전체 17개 session을 보존했다. 고객 복구·고가용성 검증은 미완료다. 계정 입장은 [terminal-admission.md](terminal-admission.md), 공유 인가는 [shared-terminal-access.md](shared-terminal-access.md), 원격 서버 승인은 [ssh-host-trust.md](ssh-host-trust.md)를 함께 적용한다.

## 저장 계약

`terminal_sessions.json`에 session ID, 소유자, tmux 이름, 제목, timeout, 원격 승인 binding을 저장한다. 실행 중 연결 수는 복원하지 않는다. 서버는 같은 디렉터리의 `terminal_sessions.lock`에 배타적 nonblocking flock을 유지한다. 동일 저장소를 사용하는 두 번째 프로세스는 시작하지 못한다. 이는 단일 writer 보호이며 분산 lease나 HA가 아니다.

소유권에는 `owner_id`를 저장한다. 이는 인증 middleware가 현재 회원 record에서 검증한 불변 local ID이며 요청 header·로그인 이름·tmux 이름으로 추정하지 않는다. `owner` 문자열은 생성 당시 표시 이름이다. 목록·scrollback·제목/공유/timeout/keepalive 변경·종료·공유 link 생성과 account 재접속에서 ID를 비교한다. 이름이 같아도 다른 ID는 이전 session의 소유자가 아니다. 이 분리는 [OWASP의 user ID와 username 구분](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html#user-ids)을 참고한 나비맨 구현이며 경쟁 제품 실측 결과가 아니다.

account 연결의 ConnectionGrant도 불변 ID를 snapshot으로 유지한다. SID·현재 role/status·현재 endpoint 정책 외에 현재 회원 ID가 같은지 검사하므로 열린 연결의 사용자가 같은 이름의 다른 회원으로 바뀌면 계속 인가하지 않는다. session의 account 공유 flag는 owner ID가 있고 소유자가 현재 terminal 권한을 가질 때만 유효하다. 소유자 삭제·권한 상실은 그 계정의 연결뿐 아니라 다른 회원이 붙어 있는 account 공유 연결도 input/output·idle 재검사에서 1008로 종료한다. 일반 OS process·이미 실행한 명령을 되돌리는 기능은 아니다.

legacy session의 누락 owner ID는 빈 값으로 읽되 이름으로 채우지 않는다. 계정 목록·재접속·scrollback·관리 변경·새 공유와 공개 공유 입장은 거부한다. 특정 session ID로 접속하다 ownership 검사에 실패하면 안내/END로 종료하며 새 local shell로 fallback하지 않는다. 자동 migration으로 shell을 죽이거나 다른 회원에게 소유권을 배정하지 않는다. 기존 expiry·정리 정책은 별도로 적용된다. 운영 배포 전에 필요한 출력·작업을 검토하고 새 승인 session으로 이전해야 한다. orphan metadata의 명시적 관리자 복구·재배정 UI와 독립 감사는 후속 범위이며 JSON 이름을 근거로 ID를 일괄 주입하지 않는다.

최초 설치에 JSON과 lock이 모두 없으면 빈 저장소를 원자적으로 발행한다. 기존 lock이 있는데 JSON이 없으면 정상 초기화로 간주하지 않고 시작을 거부한다. 기존 JSON은 크기·일반 파일 여부·JSON 구조·중복 ID·중복 tmux 이름과 각 필드의 유효성을 검사한다. 손상 또는 조회 오류를 빈 목록으로 바꾸지 않는다. legacy 원격 binding을 현재 승인으로 추정해 채우지도 않는다.

새 snapshot은 같은 디렉터리의 임시 파일을 권한 0600으로 생성해 write, file fsync, rename, parent-directory fsync를 순서대로 수행한다. 기존 snapshot을 직접 truncate하지 않는다. 로드에서도 file과 parent-directory fsync를 요구한다. 일반 파일과 저장소 검증은 password·JWT·users 초기화 전에 수행한다. 앞서 공유 저장소를 열면서 빈 공유 JSON·lock을 생성할 수 있으므로 모든 파일 생성보다 먼저라는 의미는 아니다.

저장소 상한은 4,096개 세션과 JSON 16 MiB다. ID 128 bytes, tmux 이름 256 bytes, owner 128 bytes, label/title 1,024 bytes 등 필드 제한도 검사한다. tmux 이름은 `nb_` 접두사와 문자·숫자·밑줄만 허용한다. 새 이름에 넣는 계정 부분은 UTF-8 문자 경계를 보존해 128 bytes까지 사용한다. 소유자 판정에는 원래 계정 이름을 사용하며 잘린 이름으로 인가하지 않는다.

새 세션은 OS 생성 전에 현재 map과 후보를 검증한다. 중복·세션 수·필드 제한에 더해 모든 세션의 제목·시간·timeout·boolean이 커질 때의 JSON 공간도 예약한다. 제목은 최대 1,024 bytes가 모두 JSON escape로 6배 커지는 상한을 사용한다. 이 보수적 예약 때문에 실제 입장 한도는 4,096개보다 작을 수 있다. 검사부터 tmux 생성·metadata 발행까지 같은 store mutex를 유지하므로 동시 요청이 같은 잔여 공간을 중복 사용하지 않는다. 용량 또는 계정 metadata 제한에 걸리면 화면 안내와 종료 신호를 보내고 새 tmux를 만들거나 저장소를 바꾸거나 서버를 종료하지 않는다.

생성된 shell은 attach 전에 기록하므로 attach 오류가 나더라도 목록에서 관리할 수 있다. 제목·공유·timeout·keepalive REST 변경도 현재 map을 복제한 후보를 검증하고 실제 JSON 크기가 한도 안인 경우에만 반영한다. 한도를 넘는 후보는 409와 기존 상태 보존 안내로 거부한다. 전체 입장 여유 공간을 요구하지 않으므로 이미 가득 찬 legacy snapshot의 크기를 줄이는 변경도 허용한다. 제목 변경 거부 시 tmux 창 이름도 변경하지 않는다. legacy preference의 timeout을 keepalive 해제에 적용할 때에는 최대 30일로 제한한다.

실제 디스크 저장 실패는 아래 exit 79 정책을 유지한다. legacy snapshot의 접속/분리 시 timestamp 증가, 긴 외부 계정 지원 확대, 조직·사용자별 상품 한도, lock 대기·OS 명령 timeout·대규모 성능은 후속 범위다. 사전 검증은 파일시스템 쓰기 성공을 예약하거나 OS 작업과 원자 commit하는 기능이 아니다.

## 저장 실패와 운영 복구

메모리와 OS 작업이 이미 변경됐을 가능성이 있으므로 저장 실패를 무시하고 계속 서비스하지 않는다. validation·write·rename·fsync 중 실패하면 프로세스가 exit 79로 종료하며 해당 HTTP 성공 응답을 제공하지 않는다. rename 뒤 fsync 실패처럼 발행 여부가 불확실한 상황도 포함한다. 이 정책은 모든 웹 연결과 진행 중 요청을 중단시킬 수 있다.

이미 생성·종료한 tmux나 실행한 원격 명령을 되돌리는 트랜잭션은 아니다. 운영자는 원본·백업과 현재 tmux 상태를 보존하고 저장 장애를 해결한 뒤 소유자·원격 승인 binding·공유 승인과 snapshot을 검토해야 한다. 복구를 위해 알 수 없는 작업을 자동 재실행하거나 현재 승인 version을 과거 snapshot에 덮어쓰지 않는다. 살아 있는 writer의 lock 파일을 삭제하면 inode가 달라져 배타 보호를 우회할 수 있으므로 삭제하지 않는다.

새 parent-directory 생성 전체의 crash durability, 하드웨어 전원 손실, 네트워크 파일시스템, metadata·OS 작업·감사 사이 원자성, 백업 복구 인가와 다중 replica는 후속 범위다. 저장 성공만으로 이 범위를 보장하지 않는다.

## 조회 실패와 세션 종료

tmux 명령은 `LC_ALL=C`에서 성공이면 존재, 해당 정확한 대상에 대한 `can't find session`이면 부재로 분류한다. 실행 파일 부재·권한 오류·socket/server 오류·다른 오류는 확인 불가다. 확인 불가인 세션은 시작·정리 과정에서 metadata를 보존한다. 종료 명령이 성공하거나 정확한 대상의 부재를 확인한 경우에만 목록에서 제거한다. 확인할 수 없는 종료는 HTTP 503이며 성공으로 응답하지 않는다.

PTY EOF에서도 같은 명시적 부재 검사를 적용한다. 접속 클라이언트가 종료돼도 tmux 조회가 확인 불가이면 metadata를 제거하거나 session END 신호를 보내지 않는다. 해당 웹 연결은 닫고 연결 수를 줄이며, 조회 도구가 복구되면 기존 shell을 다시 확인할 수 있다. 실제 shell 부재가 확인되면 기존 END 계약을 유지한다.

REST 목록은 `alive`, `ended`, `unknown`, `blocked`를 구분한다. `blocked`는 원격 승인 등 현재 입장 검사를 통과하지 못한 세션이다. 화면은 확인 불가·차단 상태의 접속과 새 공유를 비활성화하고 preview를 제공하지 않는다. 이미 공유된 대상의 회수와 관리 작업은 가능하다. HTTP 응답이 유실되면 결과 확인 안내를 표시하며 자동으로 변경 요청을 반복하지 않는다.

tmux의 기본 대상 선택은 접두사나 패턴으로 다른 세션을 고를 수 있다. 접속·조회·종료는 `=세션이름`, pane 작업은 `=세션이름:`을 사용해 정확한 세션을 지정한다. 이는 [tmux 공식 target 문법](https://man.openbsd.org/tmux)에 따른다. 승인한 세션이 사라져도 이름이 비슷한 다른 세션으로 연결하거나 그 화면을 읽지 않아야 한다.

## 개발 검증

단위 시험은 배타 writer, private file, 정상 재시작, 기존 저장소 유실·손상·중복 ID/이름 거부, 실제 자식 프로세스의 exit 79, tmux 조회 오류 분류를 확인한다. 별도 tmux namespace에서 대상이 없는 경우 이름이 비슷한 세션을 조회·capture·kill하지 않는 실제 명령 시험도 포함한다.

`scripts/smoke-terminal-registry.py`는 임시 계정·DB·tmux namespace에서 실제 HTTP 저장 실패로 성공 응답 없이 exit 79, 검토된 원본 보존, 잘못된 저장소 재시작 거부, 명시적 fixture 복구 후 원래 shell 복원을 확인한다. tmux 실행 도구를 일시적으로 사용할 수 없게 해 unknown 표시·종료 503·metadata 보존과 도구 복귀 뒤 정상 상태도 확인한다. 유효한 대형 legacy snapshot으로 실제 동시 입장 두 건을 거부하고 안내·종료 신호, tmux 목록 불변, metadata 불변, 서버 생존과 명시적 snapshot 복구 뒤 기존 shell 유지도 확인한다. `scripts/smoke-commercial-integrity.py`는 손상된 terminal JSON을 보존하면서 인증 초기화 전에 시작을 거부하는 것을 검증한다. 운영 저장소에 장애를 주입한 시험이 아니다.

상세 시험 결과와 잔여 범위는 [execution.md](execution.md)에 기록한다. 경쟁 제품 실측이나 C03·C09·C10·상용화 전체 완료를 의미하지 않는다.

후속 실제 시험은 JSON이 정확히 16 MiB인 legacy snapshot에서 제목·timeout·keepalive 변경의 409와 메모리·파일·tmux 창 이름 불변, 크기를 줄이는 shared boolean 변경의 성공을 확인했다. 임시 tmux wrapper에서 조회를 실패시키고 해당 namespace의 정확한 client PID만 종료해 실제 PTY EOF, END 미발행, metadata·shell 보존, 연결 수 감소와 조회 복구 뒤 alive 표시도 확인했다. tmux server나 운영 세션을 종료한 시험이 아니다.

## 환경설정 저장과 장애 격리

`terminal_settings.json`은 별도 단일 writer registry다. `terminal_settings.lock`에 배타적 flock을 유지하고 읽기·변경을 같은 mutex로 보호한다. HTTP 설정 GET/PUT과 새 session 생성·keepalive 변경이 같은 registry를 사용한다. username별 설정이며 tenant 정책·OS 권한·상품 quota를 대신하지 않는다.

정상 저장소에 해당 계정 record가 없는 경우만 기본 설정을 반환한다. 처음 설치에 JSON·lock이 모두 없으면 빈 파일을 발행한다. 기존 lock이 있는데 JSON이 없거나 일반 파일이 아니거나 JSON이 손상됐거나 username이 중복되거나 필드가 잘못되면 startup을 거부한다. 1 MiB·4,096개 계정·username 256 bytes, scrollback 100–200,000, keepalive interval 5–3,600초, font 8–28, idle timeout 0–30일을 검사한다. keepalive가 켜져 있으면 저장된 idle timeout은 0이어야 한다. 누락된 legacy 필드는 기존 serde 기본값을 적용하되 잘못된 값은 기본값으로 덮지 않는다. 검증은 password·JWT·users 초기화 전에 수행하며 앞서 공유·session 저장소 파일은 생성될 수 있다.

PUT은 전체 후보를 검증한 뒤 0600 임시 파일·file fsync·rename·parent-directory fsync로 발행한다. 모든 계정 snapshot의 동시 변경을 직렬화하므로 한 계정 저장이 다른 계정 설정을 유실시키지 않는다. 검증 실패는 기존 memory와 저장소 health를 유지한다. 파일 발행 실패는 해당 preferences registry를 격리한다. rename 뒤 fsync 오류처럼 결과가 불확실한 경우도 포함하며, 실패 응답을 성공으로 바꾸지 않는다.

격리 후 GET/PUT과 session keepalive 변경은 503, 새 session은 preferences unavailable 안내와 END로 거부한다. 이미 열려 있는 shell을 종료하거나 기존 tmux metadata 저장소·서버 전체를 중단시키는 정책은 아니다. 기존 session 재접속은 새 설정이 필요 없으므로 기존 계정·소유권·원격 승인 검사에 따라 처리한다. 단순 파일 복구로 실행 중 격리를 자동 해제하지 않는다. 운영자는 저장 장애와 발행된 snapshot을 검토하고 정상 파일을 복구한 뒤 서버를 재시작해야 한다. 저장 내용이 불확실한 PUT을 자동 재실행하지 않는다.

설정 GET은 프로세스가 검증한 memory snapshot을 반환한다. 실행 중 직접 JSON을 수정하는 hot reload 기능이 아니다. 브라우저 localStorage mirror는 화면 표시를 위한 cache이며 서버 설정 검증·성공 응답을 대신하지 않는다. 설정 로드 실패를 modal에서 표시하고 저장의 HTTP 실패·네트워크 오류를 처리한다. 확인된 저장 응답이 없으면 cache와 현재 화면 설정을 새 값으로 바꾸지 않으며 자동 재시도하지 않는다. 한국어·영어·일본어 안내를 제공한다.

단위 시험은 12개 계정의 실제 동시 저장·배타 writer·0600·재시작, 기존 파일 유실·중복·손상·잘못된 값 거부, 검증 실패 시 정상 상태 보존과 실제 rename 실패 뒤 조회·쓰기 격리를 확인했다. HTTP fixture는 환경설정 PUT/GET·재시작 보존, 실패 503·원본 backup 보존·새 session 차단·서버/기존 shell 유지·파일 복구 후 계속 격리·명시적 재시작 복구를 확인했다. startup fixture는 손상 JSON이 인증 초기화를 차단함을 확인했다. parent-directory 생성 전체의 crash durability·실제 전원 손실·다중 replica·외부 계정 rename 시 설정 이전·브라우저 cache의 계정별 분리·독립 감사는 잔여 범위다.
