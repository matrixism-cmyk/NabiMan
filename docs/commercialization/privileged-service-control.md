# 서비스 제어의 API·root helper 경계

2026년 10월 5일 개발 구현이며 C05 전체 완료가 아니다. `/api/services/action`만 typed helper로 이전했다. 파일·package·database·firewall·config·backup·local shell 등 다른 legacy 실행 경로와 root installer·Docker는 아직 이전 대상이다. 현재 설치 스크립트의 root API에서 새 서비스 변경은 503으로 거부한다. 기존 기능을 모두 보존한 비특권 설치·데이터 이전을 검증해야 상용 upgrade gate를 닫을 수 있다.

같은 날 23:48 KST 사용자의 명시적 운영 배포 지시에 따라 현재 검증 binary/UI를 man.bebo.kr에 반영했다. 이 호스트는 원래 systemd inventory가 동작하지 않는 supervisor 환경임을 전환 전에 확인했다. helper binary는 준비하되 실행하지 않았고 root API의 서비스 변경 거부를 유지했다. 기존 local/SSH terminal·회원·소속 기능을 검증했으며 이 배포가 비특권 API로의 전체 이전이나 systemd helper 운영 실증을 뜻하지 않는다.

Cockpit은 로그인한 OS 사용자의 일반 권한과 권한 상승을 구분한다. 나비맨의 첫 단계는 전용 API Unix UID와 제한된 root helper를 분리하는 것이다. OS 사용자별 로그인·Polkit·재인증·사람별 실행 identity를 구현한 것은 아니다. [Cockpit 공식 문서](https://docs.cockpit-project.org/cockpit-guide/main/guide/privileges.html)

## 구현한 경계

`nabiman-server`의 서비스 변경은 `nabiman-privileged-helper`의 pathname Unix socket만 사용한다. `/usr/bin/systemctl` 직접 호출·sudo·shell fallback은 없다. 조회용 `list-units`와 `list-unit-files`는 API의 일반 권한으로 유지한다. API가 root이면 변경·helper 결과 조회를 거부한다. helper 미설치·잘못된 peer·대기열 포화는 503이며 새 OS 변경을 실행하지 않는다.

helper는 root로만 시작한다. API UID는 0을 허용하지 않으며 root 소유·0600인 설정과 root 소유·group/other 쓰기 불가인 모든 상위 디렉터리를 요구한다. policy·ledger·lock에서 symlink와 일반 파일이 아닌 대상을 거부한다. socket parent도 root가 보호하고 socket은 해당 API UID 소유·0600이다. IPC의 파일 접근 권한에 더해 양쪽에서 커널 `SO_PEERCRED`로 UID를 확인한다. API는 helper UID 0을 고정하고 helper는 설정한 API UID만 받는다. [Linux Unix socket 매뉴얼](https://man7.org/linux/man-pages/man7/unix.7.html)

현재 local terminal은 여전히 API UID로 실행돼 API 소유 파일과 같은 UID signal에 접근할 수 있다. 사람/terminal 전용 OS identity·credential 분리는 C05/C09의 후속 gate이며 process pin만으로 session 격리를 완성하지 않는다.

이 UID는 웹 사용자가 아니라 서비스 프로세스 identity다. 전용 Unix 계정에 interactive login·sudo·다른 app을 공유하지 않는다. local terminal도 API와 같은 UID로 실행되는 현재 구조를 확인해 UID 검사만으로 입장을 허용하지 않는다. root supervisor가 `--pin-api PID`로 kernel start ticks·boot ID·실제 UID를 확인하고 root 전용 `control.api-peer.json`을 원자 발행한다. helper는 매 요청의 커널 peer PID와 현재 process identity까지 비교한다. 같은 UID의 다른 process·PID 재사용·이전 boot·pin 누락/손상은 입장을 거부한다. API 시작 시 dumpability를 해제해 core dump와 같은 UID의 process memory 접근을 제한한다. [Linux dumpability 명세](https://man7.org/linux/man-pages/man2/PR_SET_DUMPABLE.2const.html) 해당 API process 자체가 침해되면 helper의 허용 action을 요청하고 actor ID를 주장할 수 있다. 따라서 root ledger의 actor는 API가 검증해 전달한 회원 ID이며 독립적인 사람 인증 증명이 아니다. API 침해 뒤 허용 서비스 변경까지 막았다고 주장하지 않는다.

root policy는 정확한 `.service` 이름과 `start|stop|restart|enable|disable`의 허용 부분집합이다. wildcard·path·option·shell 문자열·실행 파일·환경 변수·추가 argv는 protocol에 없다. unit당 허용 action을 별도로 검사한다. 자기 API/helper 서비스 이름은 policy에서 거부한다. 새 정책의 권한을 채택하려면 helper 재시작이 필요하다. 2026-10-06부터 실행 직전에 root 정책을 다시 열어 소유권·권한·상위 경로·크기·형식 및 의미상 정책 hash를 검사한다. 시작 시 승인과 다르면 새 OS command를 보내지 않고 durable receipt를 `not_dispatched`로 남긴다. 정책 철회·삭제·손상·symlink·권한 완화는 재시작 전에도 차단에 반영된다. 의미상 동일한 정책의 포맷 변경은 허용하며, root가 정확한 승인 정책을 복구하면 새 operation ID로만 다시 요청할 수 있다. 이미 접수된 systemd 작업을 취소하거나 root의 검사 직후 변경을 원자적으로 차단하는 기능은 아니다. restart·start·stop의 영향에는 systemd unit의 dependency와 configuration이 포함되므로 하나의 unit 변경이 다른 서비스에 영향을 주지 않는다고 보장하지 않는다. [systemd 공식 systemctl 명세](https://raw.githubusercontent.com/systemd/systemd/main/man/systemctl.xml)

서비스 승인 때 unit·drop-in·실행 파일·스크립트·환경/설정 참조를 검토해야 한다. API UID가 수정할 수 있는 파일을 root service가 실행/해석하도록 승인하면 권한 상승 경로가 될 수 있다. helper가 이 참조 전체의 ownership·digest·drift를 자동 검증한 것은 아니다. 이 검증과 승인 UI는 C05/C10 후속 gate이며 현재 템플릿을 곧바로 운영에 적용하지 않는 이유다.

## 요청과 저장된 결과

protocol v1은 4-byte big-endian 길이와 최대 4,096-byte JSON 한 건이다. unknown field·action·version·UUID·잘못된 actor를 거부한다. API는 현재 회원 ID, SID, 역할/소속을 실행 직전과 응답 직전에 확인한다. helper는 최대 3초 남은 요청만 받고 저장 뒤에도 만료를 검사하고 실행 직전 root pin의 현재 값과 process 생존/identity를 다시 확인한다. socket read/write 각 2초, API read 25초, connect 최대 2초이며 full accept queue는 재시도 없이 거부한다. helper는 한 번에 한 request/command만 처리하고 command deadline은 20초다.

API는 `Idempotency-Key`의 canonical UUID를 사용하거나 서버에서 UUID를 만든다. helper는 root 전용 SQLite ledger에 요청·actor ID·정확한 unit/action·승인 policy snapshot/hash·running intent를 `journal_mode=DELETE`·`synchronous=EXTRA` transaction으로 commit한 뒤 실행한다. 별도 0600 flock으로 writer를 제한한다. SQL insert 실패는 명령을 실행하지 않으며 결과 저장 실패는 helper를 중단하고 확인되지 않은 성공 응답을 보내지 않는다. 접속 단절은 이미 저장한 명령을 자동으로 재실행하지 않는다.

DELETE mode에서는 FULL만으로 commit 직후 power loss의 마지막 transaction 수명을 보장할 수 없다는 공식 설명을 확인해 EXTRA를 사용하고 실제 적용값을 확인한다. EXTRA는 journal unlink 뒤 parent directory sync를 추가한다. 이는 SQLite 설정과 startup 검사이며 실제 지원 filesystem·hardware의 power-cut 시험을 완료한 것은 아니다. [SQLite 공식 synchronous 명세](https://www.sqlite.org/pragma.html#pragma_synchronous)

같은 UUID는 409이고 반복 실행하지 않는다. 다른 actor에게 이전 receipt를 노출하지 않는다. `running` 또는 `unknown_result`가 있는 unit은 새 UUID·다른 허용 action으로도 변경하지 않는다. helper 재시작은 running을 unknown_result로 이전하며 policy snapshot과 receipt의 unit·action·ID·state 일치를 검증한다. `GET /api/services/operations/{id}`는 현재 계정이 만든 기록만 읽으며 명령을 실행하지 않는다. 조회·변경 응답은 `Cache-Control: no-store`다.

| 상태 | 의미 |
|---|---|
| `command_succeeded` | systemctl client exit 0. workload 건강·실제 상태 전체 검증을 뜻하지 않음 |
| `command_failed` | systemctl client의 실패. 부분 영향이 없었다는 보장은 아님 |
| `not_dispatched` | spawn 실패·저장 뒤 만료·실행 직전 승인 process identity 변경/종료. OS command를 실행하지 않음 |
| `unknown_result` | deadline·중단·결과 저장 실패 이후 재시작. systemd가 이미 받은 작업은 계속될 수 있음 |

API가 helper 응답을 받지 못하면 receipt를 만들어 성공을 주장하지 않는다. 작업 ID만 유지하고 결과 조회를 안내한다. browser는 ID를 요청 전에 계정 ID별 sessionStorage에 기억하고, 새 클릭의 중복 제출을 막으며 reload 뒤 조회만 수행한다. 401 refresh가 HTTP를 다시 보내더라도 같은 idempotency key를 사용한다. UUID는 재생 방지 namespace이며 외부 동작의 분산 exactly-once 보장은 아니다.

공통 refresh client는 [로그인 generation](terminal-admission.md#공통-api-갱신과-로그인-전환)이 바뀐 요청을 새 계정으로 재시도하지 않는다. 서비스 화면도 계정 전환 뒤 늦은 receipt를 새 화면이나 새 계정의 작업 ID cache에 반영하지 않는다.

ledger는 최대 100,000개 operation을 허용한다. 신규 admission 용량 초과와 저장 문제는 helper 중단·검토된 유지보수를 요구한다. 동일 ID와 read-only inspection은 신규 용량을 소비하지 않는다. 운영용 retention/archival·복구 실행·unknown 결과 재조정·승인/fencing·분산 worker·외부 불변 감사·전체 actor 정책 재평가는 아직 없다. 현재 unknown unit을 API에서 강제로 해제하는 기능도 없다. 저장된 policy snapshot은 root 승인 구성의 근거이며 root 공격자에 대한 불변 감사가 아니다.

## 배포 준비물과 이전 gate

API 재시작마다 trusted supervisor가 실제 API MainPID를 다시 pin해야 한다. API 검토 템플릿의 root ExecStartPost가 이 단계를 선언한다. [systemd service 명세](https://raw.githubusercontent.com/systemd/systemd/main/man/systemd.service.xml) API가 자기 PID를 JSON protocol이나 API 소유 파일로 주장하는 방식은 없다. CLI pin 대상은 root supervisor가 신뢰한 실제 API process여야 하며 단순 UID 일치만으로 임의 process를 승인하지 않는다.

archive에 두 binary와 `packaging/`의 helper unit·빈 policy 예제·비특권 API 검토 템플릿을 포함한다. 기존 installer는 아직 이 템플릿을 자동 적용하지 않는다. `api_uid: 1001`은 예시이며 실제 전용 `nabiman` 계정의 UID로 대체해야 한다. services의 기본값은 빈 배열이고 실제 설치 service를 자동 허용하지 않는다.

helper unit은 root 소유 runtime directory와 독립 state directory, NoNewPrivileges, 제한된 address family·capability, 파일시스템 보호와 process/memory limit을 선언한다. 이 unit의 실제 systemd sandbox 동작과 지원 배포판 matrix는 후속 설치 시험 대상이다. smoke에서 root helper를 실행한 것은 전체 unit hardening 실증과 다르다.

운영 전에는 실제 backup/restore, root 기존 auth·DB·SSH trust·session·tmux의 명시적 이전, 모든 legacy root 기능의 typed 경계 또는 별도 OS identity, 정책/서비스 참조 drift·권한 회수·사전 승인, installer와 Docker의 전용 계정, rollback과 고객 pilot을 검증한다. 표준 service/install 파일의 User=root를 단순 변경해 완료 처리하지 않는다.

## 실제 시험 범위

`scripts/smoke-privileged-helper.py`는 non-root 계정과 `sudo -n`을 사용하는 격리 시험이다. 고유 `/run/nabiman-helper-smoke-*` directory의 root 소유 helper binary·policy·ledger만 생성하고 마지막에 제거한다. 명령 대상은 존재하지 않는 고유 service 이름이며 기존 service·systemd unit file·운영 terminal을 바꾸지 않는다. 실제 systemctl client의 nonzero 결과를 검증하며 정상 workload 변경/건강 검증을 대체하지 않는다.

시험은 실제 UID 위조·같은 UID의 다른 process 거부·API memory 접근 제한·허용 범위·JSON framing·중복 writer·일회 실행·role 경계·private socket·own-only 결과 조회·helper 없음과 PATH fallback 차단, 실제 SQLite insert/update fault·중단/재시작·unknown과 새 key 차단, 손상/유실 ledger·unsafe policy 거부를 포함한다. Chromium 시험은 inventory reader만 fixture로 대체하고 변경/조회는 실제 API·root helper를 사용한다. 확인된 실패 표시·응답 유실 후 reload·조회에 의한 회복·명령 미재생을 확인한다.

실행 직전 재검사는 실제 SQLite shared reader로 intent commit을 막고 journal 생성 이후 root operator가 pin을 다른 process로 바꾸는 시험으로 확인했다. lock을 풀면 기록은 `not_dispatched`이며 실행되지 않는다. `/proc/PID` 디렉터리의 소유권을 검사했던 최초 시험은 잘못된 assertion으로 실패했다. [Linux proc_pid 명세](https://man7.org/linux/man-pages/man5/proc_pid.5.html)와 실제 작은 process 실험을 확인한 뒤 `/proc/PID/mem`의 root 소유권과 같은 UID parent의 실제 open 거부를 검증한다. memory 내용을 읽지 않는다.


## 2026-10-06 — 실행 직전 root 정책 재검사

이전 helper는 시작할 때 정책을 읽고 계속 보유했으므로, root가 파일을 바꿔 권한을 철회한 뒤에도 재시작 전에는 이전 승인이 남았다. 현재는 commit된 intent 이후 process pin 확인에 더해 정책을 다시 검사한다. 다른 action·unit뿐 아니라 전체 승인 snapshot hash가 바뀌면 `not_dispatched`다. 새 정책 자동 채택이나 권한 확대 hot reload는 하지 않는다. 이전 receipt의 읽기·같은 operation ID의 중복 차단은 유지한다.

실제 root helper 시험에 정책 철회·0600→0644·유실·잘못된 JSON·symlink 교체, 동일 의미의 JSON 재정렬, 원래 정책 복구와 새 ID, SQLite reader로 intent commit을 잡은 상태에서 정책 철회를 추가했다. 대상은 존재하지 않는 고유 시험 unit이며 운영 서비스는 변경하지 않는다. 이 시험의 실제 실행 결과는 [실행 기록](execution.md)에 남긴다.

Cockpit은 일반 OS 세션 권한과 사용자가 올리는 관리 권한을 구분한다는 [공식 설명](https://docs.cockpit-project.org/cockpit-guide/main/guide/privileges.html)을 다시 확인했다. 나비맨의 정책 hash 재검사는 자체 실행기의 설계이며 Cockpit이 같은 protocol이나 저장 방식을 사용한다는 주장은 아니다. 시스템 전체의 최소 권한 분리·service 참조 파일 drift·실제 systemd workload 건강 검증은 여전히 별도 gate다.
