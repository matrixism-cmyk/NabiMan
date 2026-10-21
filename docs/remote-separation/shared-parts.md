# 원격(Remote) 분리 — 공통 부품 목록

작성 2026-10-21. 원격 쪽 코드가 나비맨의 무엇에 기대고 있는지, 거꾸로 나비맨이
원격의 무엇에 기대고 있는지를 **실제 import·참조를 기준으로** 정리했다. 다음
단계(경계 정하기, 떼어 내기)의 작업 목록으로 쓴다.

## 0. 범위: 무엇이 "원격"인가

| 구분 | 프런트엔드 | 백엔드 |
|---|---|---|
| 원격 본체 | `components/remote/**` (목록·작업 화면·파일 패널·미리보기·/remote 셸) | `remote_servers.rs`(+`remote_protocols.rs`, `remote_check.rs`), `remote_server_operations.rs`, `remote_probe.rs`, `remote_host_trust.rs`, `ssh_auth.rs`, `ssh_keys.rs`, `models/remote.rs` |
| 터미널 | `components/terminal/**` (xterm, 창, 공유, 받아쓰기, 글꼴) | `terminal/**` (pty·tmux, 세션, sftp/ftp, 공유, telnet, 받아쓰기), `terminal_tickets.rs` |
| 공개 공유 페이지 | `/share/<token>` → `terminal/SharePage` (`index.tsx`에서 분기) | `/api/share/**` (`terminal/share*`) |
| **경계 미정** | `components/claude/**` (Claude 음성 대화) | `terminal/claude_voice/**` |

규모: 프런트엔드 약 11,000줄, 백엔드 약 13,900줄.

Claude 음성 대화는 메뉴는 나비맨에 있지만 코드는 터미널 쪽(받아쓰기, `/api/terminal/claude`)에
있다. 원격 제품에 넣을지, 나비맨에 남길지를 먼저 정해야 한다(§5).

---

## 1. 원격 → 나비맨: 원격이 빌려 쓰는 것

### 1-1. 프런트엔드

| 부품 | 위치 | 원격에서 쓰는 곳 | 성격 | 분리할 때 |
|---|---|---|---|---|
| API 호출·토큰 갱신 | ~~`hooks/useApi`~~ → `shared/api/useApi` (`apiRequest`, `apiPost`, `useApi`, `fetchWithRefresh`) | 18개 파일 | **인증 핵심** | 공통 패키지로 (로그인 공유의 중심) |
| 다국어 | `i18n` (`useT`, `I18nProvider`) | 45개 파일 | 틀은 공통, 문구는 대부분 원격 것 | 틀은 공통 패키지로, 원격 문구 파일은 원격으로 (§1-3) |
| 확인 창 | ~~`common/ConfirmDialog`~~ → `shared/ui/ConfirmDialog` (`confirmDialog`) | 9개 파일 | 범용 UI | 공통 UI 패키지로 |
| 모달 틀 | ~~`common/Modal`~~ → `shared/ui/Modal` | 4개 파일 | 범용 UI | 공통 UI 패키지로 |
| 길게 누르기 | `common/useLongPress` | 2개 파일 | 범용 훅 | 공통 UI 패키지로 |
| 자동 로그아웃 | `hooks/useIdleLogout` (`markActivity`) | 받아쓰기 1곳 | 계정 정책 | 공통(인증) 패키지로 |
| 상단 접기 | `hooks/useChromeHidden` | `/remote` 셸, 원격 설정 | 셸 UI | 원격 셸이 자기 것을 가져가고 나비맨과는 설정 키만 공유 |
| 테마 | `theme` (`ThemeSelector`, CSS 변수) | `/remote` 셸, 전체 색 | 디자인 | 공통 디자인 토큰 (CSS 변수 목록) |
| 언어 선택 | `components/LanguageSelector` | `/remote` 셸 | 셸 UI | 공통 |
| 서버 타입 | ~~`types`~~ → `remote/remoteTypes.ts` (`RemoteServer`, `RemoteServerStatus`, `ProtocolCheck`, `SshKeyInfo`) | 원격 18개 파일 + 나비맨 `ScopeResourcesPanel` | **원격 것** | ✅ 1단계에서 옮김 |
| 다중 서버 대시보드 | ~~`components/MultiServerDashboard`~~ → `remote/MultiServerDashboard` | 전체 현황 → 요약 | **원격 것** | ✅ 1단계에서 옮김 |
| SSH 키 화면 | ~~`components/SshKeyComponents`~~ → `remote/SshKeyComponents` | 전체 현황 → SSH 키, 서버 상세 | **원격 것** | ✅ 1단계에서 옮김 |
| 공통 CSS | `App.css`, `components.css`, `design.css` (`.btn`, `.panel`, `.modal-*`, `.form-group`, `.message` …) | 거의 전부 | 디자인 | 공통 디자인 패키지로 |
| 글꼴·기호 | ~~`assets/fonts/**`, `assets/circled-digits.woff`~~ → `terminal/fonts/` | 터미널 | **원격 것** | ✅ 1단계에서 옮김 (라이선스 원문은 그대로 `public/fonts/`) |

외부 라이브러리(원격만 씀): `xterm`, `@xterm/addon-fit`, `@xterm/addon-web-links`,
`pdfjs-dist`, `highlight.js`. 나비맨 쪽과 겹치는 것은 `react`뿐.

### 1-2. 백엔드

| 부품 | 위치 | 원격에서 쓰는 것 | 쓰는 파일 | 성격 |
|---|---|---|---|---|
| 응답 형식 | `models::ApiResponse` | 모든 API 응답 | 17 | 공통 규약 |
| 로그인 확인 | `auth` | `verified_claims`, `verified_user_id`, `extract_username`, `Claims` | 7 | **인증 핵심** |
| 계정 | `users` | `UserStore`, `find_user_by_name` | 9 | **인증 핵심** |
| 로그인 세션 | `jwt_sessions::SessionStore` | 공유 링크·티켓 검증 | 4 | **인증 핵심** |
| 최고관리자 판정 | `membership::actor` | 받아쓰기·Claude 설정 변경 | 3 | 권한 |
| 범위(테넌트) 권한 | `scope_access::permission`, `models::ScopeKind` | 범위 제한 계정의 서버 접근 | 3 | 권한 |
| 2단계 인증 정책 | `mfa_policy::permitted` | 1 | 1 | 권한 |
| 비밀 저장 | `secret_store` (`encrypt`, `decrypt`) | 서버 비밀번호, API 키 | 2 | 공통 (같은 `secret.key`) |
| 안전한 파일 쓰기 | `private_storage::write` | 설정·세션·대화 파일 | 5 | 범용 도구 |
| 역할별 허용 | `rbac.rs` | `/api/terminal`, `/api/remote-servers` 경로를 **문자열로** 규칙화 | — | 경로 규약 |
| 감사 기록 경로 가리기 | `audit_middleware.rs` | `/api/share/…`, `/api/terminal/share-links/…`의 토큰 가림 | — | 경로 규약 |
| 보안 헤더 | `security_headers.rs` | 마이크 허용 (`microphone=(self)`) | — | 배포 설정 |

### 1-3. 다국어 문구

원격 화면이 쓰는 문구 접두사: `remote`, `terminal`, `sftp`, `share`, `preview`,
`dictation`, `sshKey`, `deployKey` — 모두 원격 것이다. 파일은 `translations/terminal.ts`,
`sftp.ts`, `remote-protocols.ts`, `remote-base.ts`. (1단계 전에는 원격 문구 66개가
나비맨의 `security.ts`·`common.ts`에 섞여 있었고, 1단계에서 `remote-base.ts`로 옮겼다.)

나비맨 공통 문구는 21개만 쓴다:

- `common.*` 16개: actions, cancel, close, connect, delete, error, loading, name,
  online, refresh, run, running, save, settings, unknown, working
- `app.*` 3개: hideChrome, logout, showChrome
- `accounts.password`, `tab.terminal`

→ 분리할 때 이 21개만 원격 쪽에 복사하면 문구 의존은 끊긴다.

---

## 2. 나비맨 → 원격: 나비맨이 원격에 기대는 것 (거꾸로 된 의존)

분리를 막는 것은 주로 이쪽이다.

| 나비맨 쪽 | 원격의 무엇을 쓰나 | 문제 | 제안 |
|---|---|---|---|
| `auth.rs` (모든 요청의 인증) | `terminal_tickets::{TicketStore, redeem}`, `terminal::sftp::links::redeem` | 인증 미들웨어가 웹소켓 티켓과 파일 다운로드 링크를 직접 안다 | "URL 한 번용 입장권" 확인을 **인증 쪽 확장 지점**(등록식)으로 바꾸고, 원격이 자기 입장권을 등록 |
| `accounts.rs`, `cron.rs`, `processes.rs`, `stream_access.rs`, `mec/**` (SSE·스트림 9곳) | `terminal_tickets::ConnectionGrant` | 이름은 터미널이지만 **실제로는 나비맨 전체의 스트림 권한 검사** | `terminal_tickets`를 나비맨 공통(인증)으로 옮기고 이름을 `connection_tickets`로 |
| `backup.rs`, `backup_schedule.rs` | `terminal::session::data_dir()` | 데이터 폴더 위치를 터미널 모듈에서 얻는다 | 공통 `data_dir()` 하나로 (지금 `NABIMAN_DATA_DIR`를 직접 읽는 곳이 백엔드에 23개 파일 — 원격 쪽 7개: `remote_servers`, `remote_host_trust`, `ssh_auth`, `ssh_keys`, `terminal/session`, `dictation`, `claude_voice`) |
| `support_report*.rs` | `terminal::session::PtyStore`, `remote_servers.json` | 지원 보고서가 터미널 세션 목록과 원격 서버 파일을 직접 읽는다 | 원격이 "보고서 항목"을 제공하는 함수 하나를 내보내고, 보고서는 그것만 부름 |
| `membership.rs` | `remote_servers::find_server` | 범위 권한에 서버가 있는지 확인 | 범위(테넌트) 모델에서 서버를 "원격이 제공하는 자원 종류"로 등록 |
| `App.tsx` | `TerminalSettingsProvider`, `TerminalWindowsProvider`, `RemoteShell` | 앱 셸이 터미널 상태를 감싼다 (떠 있는 터미널 창이 모든 메뉴 위에 뜨기 때문) | 원격 제품에서는 원격 셸이 감싸고, 나비맨에서는 "원격 창 제공자"를 선택적으로 꽂게 |
| `PanelOutlet.tsx` | `RemoteWorkspace` | 메뉴 → 원격 화면 | 그대로 둬도 됨 (나비맨이 원격을 "포함"하는 지점) |
| `MultiServerDashboard`, `ScopeResourcesPanel` | `ProtocolBadges`, `protocols`, `HostTrustForm` | 나비맨 화면이 원격 부품을 씀 | 앞의 둘은 원격으로 옮기면 해결, `ScopeResourcesPanel`은 원격이 내보내는 부품으로 |
| `components/claude/useVoiceChat` | `terminal/dictation/{useDictation, sockets}` | Claude 음성이 받아쓰기를 씀 | §5 결정에 따름 |
| `index.tsx` | `terminal/SharePage` | `/share/<token>` 공개 페이지 | 원격의 공개 입구로 함께 이동 |

---

## 3. 데이터와 설정 파일 (`NABIMAN_DATA_DIR`)

| 원격 것 | 공통 (나비맨과 함께 써야 함) |
|---|---|
| `remote_servers.json` (서버 목록, 비밀번호 암호화) | `users.json` (계정) |
| `ssh/` (원격용 SSH 키), `ssh-host-trust/` (호스트 키 신뢰) | `login_sessions.json` (로그인) |
| `terminal_sessions.json` (tmux 세션), `tmux.conf` | `jwt_secret` (토큰 서명) |
| `terminal_settings.json` (계정별 터미널 설정: 글꼴 등) | `secret.key` (`secret_store` 암호화 키 — 원격 비밀번호도 이것으로 암호화) |
| `terminal_shares.json` (공유 링크) | `app_settings.json` (자동 로그아웃) |
| `dictation.json` (음성 서비스 키) | `audit.jsonl` (감사 기록 — 원격 동작도 여기 기록) |
| `claude-voice.json`, `claude-voice/` (§5) | |

**`secret.key`가 핵심이다.** 원격의 저장된 비밀번호·API 키가 나비맨과 같은 키로
암호화돼 있어서, 원격을 따로 배포하면 이 키를 함께 옮기거나(같은 서버) 원격 전용
키로 다시 암호화해야 한다.

브라우저 저장소(`localStorage`)의 원격 키는 모두 `nabiman_term_*`, `nabiman_sftp_*`,
`nabiman_remote_*`, `nabiman_chrome_hidden`이다. 로그인 토큰은 `sessionStorage`의
`nabiman_token`, `nabiman_refresh` (공통).

---

## 4. API 경로

| 원격 것 | 공통 |
|---|---|
| `/api/terminal` (웹소켓), `/api/terminal/**` (세션, 설정, 티켓, sftp, 공유 링크 관리, 받아쓰기, Claude) | `/api/auth/**` (로그인, 갱신, 로그아웃, 활동 기록) |
| `/api/remote-servers/**` (서버, 확인, 실행, SSH 키, 호스트 신뢰) | `/api/users/me` (현재 계정·권한) |
| `/api/terminal/files/{server}/share-link`, `/api/share/**` (공개 공유) | `/api/app-settings` (자동 로그아웃 시간) |

원격 API가 이미 두 접두사 아래에 모여 있어서, 별도 제품에서도 경로를 바꿀 필요는 없다.

---

## 5. 먼저 정해야 할 것

1. **Claude 음성 대화는 어디 소속인가.** 원격(받아쓰기·터미널과 한 몸) 또는
   나비맨(메뉴는 나비맨). 제안: 원격 쪽 — 코드·설정·API가 이미 원격 쪽에 있고,
   음성 서비스 키(`dictation.json`)를 함께 쓴다.
2. **로그인을 계속 공유할 것인가.** 같은 서버에서 나비맨과 원격을 함께 쓰는 동안은
   공유가 자연스럽다(지금 구조). 따로 파는 제품이 되면 원격이 자체 로그인(계정·세션·
   2단계 인증)을 가져야 하고, 이때 §1-2의 "인증 핵심"이 통째로 공통 라이브러리가 된다.
3. **범위(테넌트) 권한을 원격에 둘 것인가.** 지금 범위 제한 계정의 서버 접근은
   나비맨의 멤버십 모델(`membership`, `scope_access`)에 묶여 있다.

## 6. 제안하는 순서

| 단계 | 할 일 | 효과 |
|---|---|---|
| 1 ✅ (2026-10-21) | 원격 것인데 공용 자리에 있는 것 옮기기: `types`의 서버 타입, `MultiServerDashboard`, `SshKeyComponents`, 글꼴, 나비맨 번역 파일에 섞여 있던 원격 문구 66개 | 원격 → 나비맨 의존이 공통 부품만 남음 (§7) |
| 2 ✅ (2026-10-21) | 공통 `data_dir()` 하나로 모으기, `ConnectionGrant`를 나비맨 공통(`stream_access`)으로 (입장권 자체는 터미널 것이라 그대로) | 나비맨 → 원격의 거꾸로 된 의존 대부분 제거 (§8) |
| 3 ✅ (2026-10-21) | 인증의 공개 경로·한 번용 입장권, 지원 보고서 항목, 멤버십의 서버 확인을 등록식(`extension_points`)으로 — 원격은 `remote.rs`에서 등록 | 나비맨 코드가 원격 모듈을 부르지 않음 (§9) |
| 4 ✅ (2026-10-21) | 프런트엔드 공통 부품(API·자동 로그아웃·다국어 틀·공통 문구 24개·모달·테마·언어 선택·기본 CSS)을 `src/shared/`로, 경계 검사(`check-boundaries.js`)를 lint에 | 원격이 나비맨에서 가져오는 것은 `shared/`뿐 (§10) |
| 5 | 원격만 빌드하는 진입점(`/remote` 전용 번들)과, 원격 API만 등록하는 서버 설정 | 실제로 따로 배포해 보기 |

1·2단계는 동작 변화 없이 옮기고 이름만 바꾸는 일이라 위험이 낮다. 3단계부터는
인증 경로를 건드리므로 로그인·공유·스트림 테스트(`smoke:signin`, `smoke:share`,
`smoke:sessions`)를 매 단계 돌린다.

## 7. 1단계 결과 (2026-10-21)

옮긴 뒤 다시 뽑은 프런트엔드 원격 → 나비맨 의존은 이제 **공통 부품뿐**이다:

| 부품 | 원격에서 쓰는 파일 수 |
|---|---|
| `hooks/useApi` (API·토큰) | 20 |
| `i18n` (다국어 틀) | 49 |
| `common/ConfirmDialog`, `common/Modal`, `common/useLongPress` | 9, 4, 2 |
| `hooks/useChromeHidden`, `hooks/useIdleLogout` | 2, 1 |
| `theme`, `LanguageSelector` | 1, 1 (`/remote` 셸) |

나비맨이 쓰는 공통 문구는 22개(`common.*` 17, `app.*` 3, `accounts.password`, `tab.terminal`).

백엔드의 `models/remote.rs`(`RemoteServer` 등)는 이미 따로 된 파일이라 1단계에서는
그대로 두었다. 2단계에서 `terminal_tickets`, `data_dir()`와 함께 모듈 자리를 정리한다.

1단계에서 함께 고친 것: 오른쪽 버튼 메뉴가, 항목을 보이게 하려고 막 일어난 스크롤 때문에
열리자마자 닫히던 문제(150ms 안의 스크롤은 무시). `smoke:rail`이 가끔 실패하던 원인이었다.

## 8. 2단계 결과 (2026-10-21)

**데이터 폴더.** `NABIMAN_DATA_DIR`를 직접 읽던 23곳(18개 파일)이 모두
`nabiman_server::data_dir()`(`lib.rs`, API 서버와 권한 도우미가 함께 쓰는 바탕) 하나를
부른다. 백업·지원 보고서가 터미널 모듈에 데이터 폴더를 묻던 의존도 없어졌다.
기본값이 혼자 달랐던 `scope_settings`(`./data`)도 `/var/lib/nabiman`으로 맞췄다 —
모든 배포(systemd, Docker, 설치 스크립트, 운영 supervisor)가 변수를 직접 주므로 운영
동작은 같다.

**연결 권한.** 들여다보니 `terminal_tickets`에는 두 가지가 섞여 있었다:

- 한 번 쓰는 입장권(`/api/terminal/tickets`) — 터미널·받아쓰기 웹소켓만 여는 **터미널 것**.
  그대로 둔다(경로도 그대로).
- `ConnectionGrant` — 오래 열린 연결(SSE 9곳, 터미널, 받아쓰기)마다 계정·로그인·권한을
  다시 확인하는 **나비맨 공통 검사**. `stream_access.rs`로 옮겼다. 터미널 세션을 볼 수
  있는지 확인하는 부분(`can_access_session`)만 `terminal/session.rs`에 남겼다.

그래서 계획의 "이름 바꾸기(`connection_tickets`)" 대신 "나눠서 공통 부분만 옮기기"가 됐다.

**남은 거꾸로 된 의존 (3단계 대상):**

| 나비맨 쪽 | 원격의 무엇 |
|---|---|
| `auth.rs` | `terminal_tickets::{TicketStore, redeem}`, `terminal::sftp::links` (URL 한 번용 입장권 확인) |
| `support_report*.rs` | `terminal::session::PtyStore` (터미널 세션 수) |
| `membership.rs` | `remote_servers::find_server` (범위 권한의 서버) |

**확인한 것:** 백엔드 테스트 전체, API e2e(66+40), 스트리밍 e2e, 브라우저 smoke
(로그인 유지·공유·세션·받아쓰기·단독 화면·서버 목록·파일·프로토콜).

**덧붙여 본 것:** 로그아웃한 뒤 열려 있던 SSE가 끊기기까지 약 5.5초 걸린다
(`ConnectionGrant` 확인은 1초마다지만). 옮기기 전 코드에서도 5.7초로 같아서 이번
변경과는 무관하다. 따로 살펴볼 거리.

## 9. 3단계 결과 (2026-10-21)

**끼워 넣는 자리 하나.** 공통 쪽에 `extension_points.rs`를 두었다. 나비맨 위에 얹히는
제품이 등록할 수 있는 것은 네 가지다.

| 자리 | 쓰는 곳 | 원격이 등록하는 것 |
|---|---|---|
| `public_prefixes` | `auth.rs` — 로그인 없이 통과시킬 API 경로 | `/api/share/` (공유 링크는 자기 비밀로 확인) |
| `url_passes` | `auth.rs` — Authorization 헤더 없이 브라우저가 직접 여는 URL | 터미널·받아쓰기 웹소켓 입장권(`terminal_tickets::url_pass`), 다운로드·재생 링크(`sftp::links::url_pass`) |
| `report_parts` | `support_report.rs` — 보고서에 덧붙일 항목 | `terminal_registry`, `server_inventory` (수집 코드는 `remote/report.rs`로 이동) |
| `server_exists` | `membership.rs` — 서버 범위 멤버십의 서버 확인 | `remote_servers::find_server` |

원격 쪽 등록은 `remote.rs`의 `extension_points()` 한 곳에 있고, `main.rs`가 앱 데이터로
붙인다. **아무것도 등록하지 않은 나비맨도 그대로 돈다:** 공개 경로 없음, 입장권으로는
아무도 못 들어옴, 보고서에서 그 항목이 빠짐, 서버 범위 멤버십은 거부. 이 동작은
`extension_points` 테스트가 확인한다. 보고서 화면도 항목 이름을 고정해 두지 않고
받은 항목 전체를 보고 "일부 수집 실패"를 판단하게 바꿨다.

입장권 확인을 옮기면서 규칙 하나를 테스트로 더 단단히 했다: 터미널 URL에 계정 토큰
(`token=`)이 있으면 **올바른 Authorization 헤더가 함께 와도** 거부한다(전과 같은 동작,
전에는 헤더 없는 경우만 시험했음).

**이제 나비맨 코드에서 원격 모듈(`terminal`, `remote_*`, `ssh_*`, `terminal_tickets`)을
부르는 곳은 `remote.rs`와 `main.rs`(모듈 등록)뿐이다.**

**남은 것 — 경로 문자열로 된 정책표 (5단계에서 다룰 것):** 코드 의존은 아니지만 나비맨의
권한표가 원격 API 경로를 글자로 안다.

| 파일 | 내용 |
|---|---|
| `rbac.rs` | 역할별 허용: `/api/terminal/**`, `/api/remote-servers` |
| `scope_access.rs` | 범위 계정이 볼 수 있는 `/api/remote-servers/**` |
| `api_keys.rs` | API 토큰 범위 `servers:read` ↔ `/api/remote-servers` |
| `audit_middleware.rs` | 감사 기록에서 `/api/share/`, `/api/terminal/share-links/` 비밀 가리기 |
| `user_profile.rs` | 2단계 인증 정책 확인에 `/api/remote-servers` |

원격만 따로 띄울 때(5단계) 이 표들도 "원격이 자기 경로 규칙을 등록"하는 식으로 옮기면
된다. 같은 서버에서 함께 도는 지금은 동작에 문제가 없다.

**확인한 것:** 백엔드 테스트 전체(새 테스트 포함), jest 321개, API e2e(66+40), 스트리밍
e2e, 브라우저 smoke(공유 15·세션 9·받아쓰기 9·단독 화면 14·미디어 15·프로토콜 31·
파일 23·로그인 유지 13), CI 스크립트(지원 보고서, 인증 입장 제한, 로그인 세션, API 토큰).

## 10. 4단계 결과 (2026-10-21)

**`frontend/src/shared/`** — 나비맨과 원격이 함께 쓰는 것만 모은 폴더. 바깥(나비맨·원격)을
가져오지 않는다.

| 폴더 | 내용 | 전에는 |
|---|---|---|
| `shared/api/useApi.ts` | API 호출·토큰 갱신, `ApiResponse` 타입 | `hooks/useApi.ts`, `types` |
| `shared/auth/useIdleLogout.ts` | 무활동 자동 로그아웃 | `hooks/` |
| `shared/i18n/` | 다국어 **틀**(`I18nProvider`가 번역표를 받음, `useT`, `Lang`)과 **공통 문구 24개**(`strings.ts`) | `i18n/context.tsx`가 나비맨 번역표를 직접 가져옴, 문구는 `common.ts`·`security.ts` |
| `shared/ui/` | `Modal`, `ConfirmDialog`, `useLongPress`, `useChromeHidden`, `LanguageSelector` | `components/common/`, `hooks/`, `components/` |
| `shared/theme/` | 테마 제공자·선택기·프리셋 | `theme/` |
| `shared/styles/` | 토큰·리셋·패널·표·배지·버튼·입력·메시지·모달 CSS (`index.ts`가 순서대로 불러옴) | `index.css`, `App.css`, `components.css`, `design.css`에 섞여 있음 |

**다국어.** 나비맨의 `i18n/`은 이제 "공통 틀 + 나비맨의 모든 문구(공통 문구 포함)"를
묶는 얇은 연결이다. 원격 화면 53개 파일은 `useT`를 `shared/i18n`에서 가져온다. 원격이
나중에 따로 서면 `<I18nProvider translations={{ ...sharedStrings, ...원격문구 }}>` 하나로 된다.
원격 문구 파일(`terminal.ts`, `sftp.ts`, `remote-*.ts`, `claude.ts`)은 아직 `i18n/translations/`에
있다 — 5단계에서 원격 진입점과 함께 옮긴다.

**CSS.** 기준은 "원격·공통 부품이 쓰는 클래스의 규칙 + 토큰·리셋"이다. 나비맨과 함께
쓰는 규칙은 `shared/styles`로, 원격만 쓰는 8개 규칙(서버 카드, SSH 키, 서버 폼 격자)은
`components/remote/remote-base.css`로 옮겼고, 나머지(머리글·메뉴·사이드바·나비맨 화면)는
그대로 `App.css` 등에 남았다. 원래 순서를 지켜서 옮겼고, **옮기기 전후로 27개 화면의
모든 요소의 계산된 스타일(36가지 속성)을 비교해 차이 0**을 확인했다(나비맨 15개 메뉴,
원격 전체 현황 5개 탭, 서버 하나의 모든 탭, 서버 추가 폼, `/remote`, 로그인 화면).

**경계 검사.** `scripts/check-boundaries.js`가 `npm run lint`(CI)에서 돈다:
`shared/`는 `shared/`만, 원격(`components/remote`, `terminal`, `claude`)은 원격과
`shared/`만 가져올 수 있다. 테스트 파일은 나비맨의 전체 문구를 쓰므로 빼고 본다.

| 원격 → 나비맨 의존 | 1단계 후 | 4단계 후 |
|---|---|---|
| 테스트가 아닌 파일 | 9개 모듈(useApi, i18n, ConfirmDialog …) | **0** (`shared/`만) |

**확인한 것:** 타입 검사, lint와 경계 검사, jest 321개, 브라우저 smoke 전부(기본, 공유 15,
세션 9, 받아쓰기 9, Claude 14, 단독 화면 14, 프로토콜 31, 서버 목록 15, 화면 13, 이름 바꾸기 12,
확대 7, 붙여넣기 9, 모바일 27, 미리보기 36, 미디어 15, 열 18, 파일 23, 로그인 유지 13),
CI 스크립트(지원 보고서, 로그인 시도 제한).

