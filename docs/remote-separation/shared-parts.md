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
| API 호출·토큰 갱신 | `hooks/useApi` (`apiRequest`, `apiPost`, `useApi`, `fetchWithRefresh`) | 18개 파일 | **인증 핵심** | 공통 패키지로 (로그인 공유의 중심) |
| 다국어 | `i18n` (`useT`, `I18nProvider`) | 45개 파일 | 틀은 공통, 문구는 대부분 원격 것 | 틀은 공통 패키지로, 원격 문구 파일은 원격으로 (§1-3) |
| 확인 창 | `common/ConfirmDialog` (`confirmDialog`) | 9개 파일 | 범용 UI | 공통 UI 패키지로 |
| 모달 틀 | `common/Modal` | 4개 파일 | 범용 UI | 공통 UI 패키지로 |
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
| 3 | 인증 미들웨어의 "한 번용 입장권" 확인을 등록식으로, 지원 보고서·멤버십은 원격이 내보내는 함수만 부르게 | 나비맨이 원격 내부를 모르게 됨 |
| 4 | 프런트엔드 공통 부품(useApi, i18n 틀, Modal/ConfirmDialog/useLongPress, 테마·CSS)을 한 폴더(`shared/`)로 모으기 | 공통 패키지로 떼어 낼 준비 |
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
