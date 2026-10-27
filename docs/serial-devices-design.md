# 세션별 시리얼 장치 (Web Serial → 터미널 세션) 설계

작성: 2026-10-27. 대상 코드: `src/backend/src/terminal/serial_bridge.rs`,
`src/frontend/src/components/remote/serialBridge.ts`, `SerialView.tsx` 와 이 문서에서 새로 만드는 파일들.

## 0. 배경과 목표

NabiMan 은 회원이 자기 서버를 여러 대 등록해 두고 쓰는 서비스다. ESP32 같은 보드를 다룰 때는
**사용자마다, 세션마다 서로 다른 장치**를 붙여 두고 작업하고, **공유 세션**에서도 장치를 써야 한다.

지금(2026-10-27 기준) 시리얼 브리지는 관리자 한 명·장치 하나를 전제로 만들어졌다.

| 항목 | 현재 | 문제 |
|---|---|---|
| 권한 | `vnc_console::is_admin` (Admin 역할) | 회원(Remote) 사용 불가. 회원 허용 목록 `member_servers::permitted` 에도 없음 |
| 이름 공간 | `$DATA/serial/<name>` 전역 | 같은 이름이면 서로의 링크를 지움 (`serial_bridge.rs` pump 의 `remove_file`) |
| 위치 | NabiMan 호스트의 PTY | 회원 서버(SSH 세션) 쪽에서는 보이지 않음 |
| 개수 | 브라우저당 1개 (`serialBridge.ts` 싱글턴) | 세션별 여러 장치 불가 |
| DTR/RTS | PTY 로 전달 불가 → `<link>.cmd` / `.ack` 우회 | esptool·arduino-cli 의 일반 자동 리셋이 안 됨 |
| 재검증 | 접속 시 1회 (`let _ = grant;`), 최대 12시간 | 권한 회수·세션 종료가 반영되지 않음 |
| 공유 세션 | 없음 | 게스트 장치를 붙일 수 없음 |

목표:

1. 장치는 **(사용자, 브라우저, 보드)** 이고, **터미널 세션에 붙는다**. 한 세션에 여러 개.
2. 그 세션이 실행되는 곳(로컬 셸 또는 회원 서버)에서 **표준 도구가 그대로** 쓴다 — 포트 경로와
   `rfc2217://` URL, DTR/RTS·보드레이트까지.
3. 세션 권한을 그대로 따른다. 세션에 들어올 수 없는 사람은 장치에도 붙을 수 없다.
4. 쓰기 권한 공유 링크의 게스트는 자기 장치를 그 세션에 붙일 수 있다 (기본 허용).
5. 한 번 붙인 장치는 세션을 다시 열면 **선택 창 없이 자동 재연결**.

## 1. 개념

- **장치(device)**: 브라우저가 Web Serial 로 연 포트 하나. 브라우저 쪽 식별자는 USB `vendor:product`
  (`SerialPort.getInfo()`). Web Serial 은 USB 시리얼 번호를 주지 않는다 →
  **같은 기종 보드 두 개가 동시에 재열거되면 서로 바뀔 수 있다** (알려진 한계, 8장).
- **부착(attachment)**: 장치를 세션 `sid` 에 이름 `name` 으로 붙인 것. 키는 `(sid, name)`.
  이름은 `[a-z0-9_-]{1,24}` 이고 **세션 안에서만 유일**하다 (다른 세션·사용자와 겹쳐도 됨).
- **부착자(attacher)**: 장치를 붙인 주체. 계정(`user_id`) 또는 공유 링크 게스트(링크 토큰의 해시 앞부분).
- **브리지**: 부착 하나당 WebSocket 하나 + 서버 쪽 릴레이 하나. 지금의 `SerialBridge` 액터를 일반화한다.

## 2. 서버 쪽 노출 — 세션이 보는 것

부착 하나는 세션 쪽에 두 가지 입구를 만든다.

| 입구 | 형태 | DTR/RTS·보드레이트 | 용도 |
|---|---|---|---|
| RFC2217 | `rfc2217://127.0.0.1:<port>` | 전달됨 | esptool, pyserial, PlatformIO, `arduino-cli upload` (esptool 경유) |
| PTY | 경로 (로컬: `$DATA/serial/<owner_id>/<sid>/<name>`) | 전달 안 됨 | 경로만 받는 도구, `screen`/`cat` 모니터 |

결정 사항(2026-10-27, 사용자): **원격 서버 쪽 노출은 TCP localhost(RFC2217)** 로 한다.
그 서버의 다른 로컬 계정도 포트에 접근할 수 있다는 점은 감수한다(RFC2217 에는 인증이 없다).
공용 서버용으로 유닉스 소켓 + PTY 옵션을 나중에 둔다.

세션 안에서 경로·URL 을 찾는 방법: UI 에 표시·복사 버튼, 그리고 세션 디렉터리의
`devices.json` (`[{name, path, rfc2217, attached_by, since}]`).

## 3. 단계

각 단계는 따로 배포할 수 있다.

### 1단계 — 세션 부착 기반 (로컬 세션)

- 새 WebSocket `GET /api/terminal/serial?session_id=<sid>&serial=<name>&ticket=…`.
  - 티켓: 기존 `/api/terminal/tickets`, 바인딩 쿼리 `session_id=<sid>&serial=<name>` (두 키 모두
    `terminal_tickets::valid_query` 가 이미 허용). `url_pass` 목록에 경로 추가.
  - 인가: `ConnectionGrant::valid()` + 세션 존재 + `grant.can_access_session(s)` +
    `remote_access::allowed(s)`. **1단계는 로컬 세션(`ssh_cmd == None`)만** — 원격 세션은
    3단계 전까지 `409` 와 이유 문자열로 거절. 로컬 셸은 Admin 만 가지므로 1단계 사용자는 사실상 관리자.
  - 30초마다 재검증 (RDP 의 `Lifeline` 방식): 권한·세션 존재·접근 가능 여부가 바뀌면 닫는다.
    세션이 종료되면 그 세션의 부착을 모두 닫는다.
- 장치 허브 `SerialHub` (`web::Data<Arc<Mutex<…>>>`): 살아 있는 부착 목록
  `(sid, name) → {owner_id, attached_by, path, since}`.
  - 같은 `(sid, name)` 이 이미 있으면 거절 (클라이언트가 다음 이름을 고른다).
  - 상한: 계정당 8, 세션당 8, 전체 64.
- 경로: `$DATA/serial/<owner_id>/<sid>/<name>` (세션 소유자 기준; 디렉터리 0700, 세션 종료 시 제거).
  `serial_dir()` 은 `nabiman_server::data_dir()` 를 쓰도록 고친다 (지금은 다른 기본값을 따로 가짐).
- 목록 API `GET /api/terminal/sessions/{sid}/serial` — 그 세션을 볼 수 있는 사람에게
  `[{name, path, attached_by, mine, since}]`. 다른 사람이 붙인 장치도 보인다(누가 붙였는지 표시).
- 리셋/다운로드 모드 제어: 2단계 전까지는 지금의 `<link>.cmd` / `<link>.ack` 를 부착별로 유지.
- 감사: 부착은 기존 `AuditMiddleware` 가 WebSocket 핸드셰이크(`GET /api/terminal/serial?session_id=…&serial=…`, 사용자 포함)로 이미 남긴다. 해제·재검증 종료는 `nabiman.err.log` 에 `[serial] …` 로 남는다. 별도 의미 이벤트(`append_entry`)는 4단계(게스트 표기)와 함께.
- 기존 `/api/remote/serial`(전역, 관리자) 과 Serial 탭은 그대로 두되 "세션에 붙이기" 안내를 단다.
  2단계 이후 제거.

프런트엔드:

- `serialDevices.ts` — `serialBridge.ts` 를 **장치 여러 개**로 일반화한 모듈 (React 밖, 탭을 옮겨도 유지).
  부착마다 포트·WebSocket·재연결(포트 재열거, 서버 재접속)·리셋/다운로드 모드 처리를 가진다.
  `useSyncExternalStore` 로 구독.
- 자동 재연결: `localStorage nabiman_serial_devices` = `{ [sid]: [{name, baud, device}] }`.
  세션 id 가 정해지는 곳(`TerminalView` 의 `onSessionId`, 워크스페이스 `currentSessionId`,
  창 `sessionId`)에서 그 세션의 기억된 장치를 `getPorts()` 로 다시 연다 (선택 창 없음).
  "해제"하면 기억에서 지운다. 사라진 세션의 기억은 세션 목록 폴링 때 정리.
- UI `SessionDevices`: 세션 헤더의 🔌 버튼 + 작은 패널.
  - 위치: `WorkspaceActions`(넓은 화면 `.rw-head-actions`, 휴대폰 시트), `TerminalWindowFrame` 제목줄.
    공유 게스트용 `SharePage` 는 4단계.
  - 패널: 장치 목록(이름, 상태, 경로 복사, 리셋, 다운로드 모드, 해제, 붙인 사람), "장치 추가"(이름·보드레이트 →
    포트 선택). 최근 출력 몇 줄 미리보기.
- i18n: `serial.dev.*` 키를 `remote-base.ts` 에 추가.

### 2단계 — RFC2217 (DTR/RTS·보드레이트)

- 서버가 부착마다 RFC2217 서버를 연다 (Rust, `127.0.0.1:<임시 포트>`). 처리하는 COM-PORT-OPTION(44):
  SET-BAUDRATE, SET-DATASIZE/PARITY/STOPSIZE, SET-CONTROL(DTR/RTS on/off, 흐름제어 none),
  PURGE-DATA, NOTIFY-MODEMSTATE/LINESTATE (고정값 응답), 텔넷 IAC 이스케이프.
- WebSocket 제어 프레임(텍스트): `{"signals":{"dtr":bool?,"rts":bool?},"gap_ms":n}`, `{"baud":n}`.
  - **타이밍 보존**: 서버가 직전 제어 변경과의 간격을 `gap_ms` 로 함께 보내고, 브라우저가 그 간격대로 재생한다.
    네트워크 지터로 esptool 리셋 순서(50–100 ms 간격)가 깨지지 않게 하기 위함.
  - DTR·RTS 는 **한 줄씩** 적용한다 (Windows usbser 가 두 줄 변경을 한 줄씩 반영해 부트 핀이
    중간에 풀리는 문제를 2026-10-27 에 확인함).
  - 보드레이트 변경: Web Serial 은 열린 포트의 속도를 못 바꾸므로 닫고 새 속도로 다시 연다.
    USB-Serial/JTAG(ESP32-C3 등)는 속도가 의미 없으므로 무시한다.
- PTY 입구는 그대로 둔다. `.cmd`/`.ack` 와 `flash.sh` 우회는 폐기.
- 확인된 사실: bootloader 상태(flasher stub 실행 중)의 ESP32-C3 는 DTR/RTS 리셋이 먹지 않았다 —
  esptool `--after watchdog-reset` 은 동작. RFC2217 경유 esptool 도 C3 에는 `--after watchdog-reset`
  을 권장값으로 안내한다.

### 3단계 — 회원 서버 세션 (SSH 원격 포워딩)

현재 원격 세션의 ssh 는 tmux 창 안의 평범한 명령이고, ControlMaster·포워딩이 전혀 없다
(`ssh_command.rs`). 나중에 붙일 손잡이가 없으므로 **부착마다 별도 ssh 연결**을 연다.

- `remote_probe::ssh_invocation(srv, …, extra = ["-N", "-o", "ExitOnForwardFailure=yes",
  "-R", "127.0.0.1:0:127.0.0.1:<로컬 RFC2217 포트>"], None)` — 원격 포트는 sshd 가 고르게 하고
  (`-R 0`), ssh 가 출력하는 할당 포트(`Allocated port N`)를 읽는다. SFTP 풀과 같은 신뢰·인증 경로
  (호스트 키 고정, 인증서/비밀번호 파일) 를 재사용한다.
- PTY 입구가 필요하면 원격에서 `socat PTY,link=~/.nabiman/serial/<name>,raw,echo=0 TCP:127.0.0.1:<N>`
  을 같은 연결로 실행(원격에 socat 이 있을 때만). 없으면 RFC2217 만.
- 원격 쪽 안내 파일 `~/.nabiman/serial/devices.json` 을 같은 연결로 써 둔다.
- 정책: 회원 서버는 지금 `ClearAllForwardings=yes` 이다 (`member_servers.rs`). 이 장치용 연결에서만
  `-R` 을 허용하도록 바꾸고, 회원 권한 `serial_devices`(기본 허용) 로 끌 수 있게 한다.
  `member_servers::permitted` 에 `/api/terminal/serial` 추가.
- 원격 sshd 가 `AllowTcpForwarding no` 면 실패 이유를 UI 에 보여 준다.
- 텔넷 세션은 지원하지 않는다.
- 연결이 끊기면(서버 재시작, 네트워크) 재접속, `RemoteBinding` 이 바뀌면 닫는다.

### 4단계 — 공유 세션

- 계정 공유(`TmuxSession.shared`): 1단계 인가(`can_access_session`)로 이미 된다.
- 링크 공유(계정 없는 게스트):
  - `share_ticket::valid_query` 에 `purpose=serial&serial=<name>` 허용.
  - `GET /api/share/{token}/serial?purpose=serial&serial=<name>&ticket=…` — 받아쓰기의
    `Admission::Share` 패턴(`dictation/bridge.rs`) 그대로. **읽기 전용 링크는 거절, 쓰기 링크는 기본 허용**
    (결정 사항). 링크가 만료·회수되면 닫는다. 링크당 장치 상한 4.
  - 부착자는 `share:<토큰 sha256 앞 8자>` 와 링크 라벨로 표시한다. 감사 로그도 같은 표기.
  - `SharePage` 헤더에 🔌 버튼 (`!info.read_only` 일 때).
  - 소유자 화면 목록에 "공유 게스트 ○○ 의 esp32" 로 보인다.

## 4. 보안

- 장치 데이터는 세션과 같은 수준의 비밀로 본다. 부착·목록 API 모두 세션 접근 권한을 요구한다.
- 티켓은 지금처럼 쿼리·Origin 에 묶인 1회용 30초. 장치 WebSocket 도 30초마다 재검증.
- RFC2217 포트는 `127.0.0.1` 에만 연다. NabiMan 호스트의 다른 로컬 프로세스는 접근할 수 있다
  (로컬 셸은 Admin 전용이므로 1–2단계에서는 같은 신뢰 경계).
- 경로·이름은 `[a-z0-9_-]` 로 제한, 경로 조합 전 검증. 소유자 id 디렉터리 0700.
- 상한(계정 8 / 세션 8 / 링크 4 / 전체 64)과 부착당 대역폭은 기존 WebSocket 프레임 상한을 따른다.

## 5. 메시지 형식 (WebSocket)

- 이진 프레임: 바이트 그대로 (양방향).
- 서버 → 브라우저 텍스트:
  - `{"path": "...", "rfc2217": "rfc2217://127.0.0.1:N"?}` 접속 직후 1회
  - 1단계: `{"signal": "reset"|"bootloader"}`
  - 2단계: `{"signals": {...}, "gap_ms": n}`, `{"baud": n}`
- 브라우저 → 서버 텍스트:
  - 1단계: `{"ack": "...", "error"?: "..."}`
  - `{"port": "up"|"down"}` — 포트 재열거 상태 (목록 API 에 반영)

## 6. 시험

- 백엔드: 인가(소유자·공유 계정·남의 세션·회원·원격 세션 거절), 이름 충돌, 상한, 세션 종료 시 정리,
  재검증으로 닫힘. RFC2217 은 pyserial `rfc2217://` 클라이언트로 왕복·제어 시험.
- 프런트엔드(jest): 장치 추가/해제, 세션별 기억과 자동 재연결, 서버 재접속, 상태 표시.
- 실기: ESP32-C3 SuperMini 로 업로드·모니터·리셋 (2026-10-27 의 `flash.sh` 와 같은 경로).

## 7. 롤아웃

- 단계마다 프런트엔드(정적 링크 교체)와 백엔드(바이너리 교체) 후 `supervisorctl restart nabiman`.
  actix-files 가 정적 디렉터리 링크를 시작 시 해석하므로 프런트엔드만 바꿔도 재시작이 필요하다.
- 재시작하면 장치 WebSocket 이 끊기고 브라우저가 1–10초 간격으로 다시 붙는다.

## 8. 알려진 한계

- 같은 VID:PID 보드 여러 개: 재열거 때 서로 바뀔 수 있다 (Web Serial 이 시리얼 번호를 주지 않음).
  이름 옆에 "같은 기종 여러 개" 경고를 띄운다.
- 브라우저 탭을 닫으면 장치도 끊긴다 (브라우저가 포트를 쥐고 있으므로).
- Chrome/Edge 데스크톱만 (Web Serial).
- RFC2217 과 PTY 를 동시에 열면 바이트가 섞인다 — 한 번에 한 입구만 쓰도록 안내하고, RFC2217 접속 중에는
  PTY 쓰기를 막는다 (2단계).
