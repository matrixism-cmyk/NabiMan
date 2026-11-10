# AI 세션 오케스트레이션 채널 (`nabiman-claude`)

NabiMan 세션 안에서 도는 프로그램(주로 Claude Code)이 다른 타일을 열고, 살펴보고, 닫을 때 쓰는 도구입니다.
`nabiman-claude` 는 이 서버의 Unix 소켓 `NABIMAN_CLAUDE_SOCKET`(기본 `/run/nabiman/claude.sock`)으로
JSON 한 줄을 보내고 한 줄을 받습니다. 비밀번호나 키는 없습니다. 누가 부르는지는 커널(`SO_PEERCRED`)이 알려 줍니다.

- **원본 위치**: CLI 는 `scripts/nabiman-claude`, 안내문은 `docs/orchestration/guide-ko.md` 하나뿐입니다.
  안내문은 서버 바이너리에 빌드 때 들어가며(`include_str!`), CLI·소켓·대시보드가 모두 그 글을 보여 줍니다.
- **배포**: 서버를 새로 빌드해 올리고, `scripts/nabiman-claude` 를 `/workspace/bin/nabiman-claude` 로 복사합니다.

## 누구로 동작하나 (보이는 범위)

1. 부른 프로세스의 uid 가 이 서버의 세션 사용자(root, uid 1000 이상)여야 합니다.
2. 그 프로세스가 NabiMan 타일(tmux 세션) 안에서 돌아야 합니다. 그 타일의 **NabiMan 계정**이 "나"입니다.
3. `status`·`close` 는 그 계정의 대시보드 타일 목록과 **같은 규칙**(`session::visible_to`: 자기 타일과 공유받은
   타일, 원격 회원 규칙 포함)으로 보이는 타일만 후보로 삼습니다. 관리자라고 남의 타일이 보이지는 않습니다.
4. `close` 는 대시보드처럼 **자기 계정 소유 타일만** 닫습니다(공유받은 타일은 볼 수만 있음).

## 타일 지정 (`who`)

차례대로 맞춰 보고, 처음 맞는 방법으로 정합니다. 둘 이상 맞으면 닫지 않고 후보 목록과 함께 거절합니다.

1. 세션 id `pty_…` 2. Claude 주소 `이름 [ref]`·`bridge:…` 3. ref (`946528`, `[946528]`, 이름이 바뀐 `옛이름 [946528]`)
4. 타일 제목 5. Claude 이름(`review-3a`) 6. 대소문자를 무시한 제목

## 명령

### 목록 — `nabiman-claude list [--json]`
변경 없음. `[{address, name, server, cwd, busy, tile, reachable}]`

### 열기(이 서버) — `nabiman-claude [open] --dir D [--title T] [--fresh] [--ask-permissions] [--user U] [--no-wait] [--json] [첫 메시지]`
변경 없음. 요청 `{"dir":"/w/x","title":"x","resume":true,"skip_permissions":true,"wait":true,"prompt":"…"}`
→ `{"ok":true,"session_id":"pty_…","title":"x","claude":"x-12 [ab12cd]"}`

### 열기(저장된 서버) — `nabiman-claude open --server gpu-01 --dir '~/proj' --fresh "학습 시작"`
대시보드의 SSH 타일과 같은 코드(호스트 키 승인, 저장된 비밀번호·인증서, 원격 회원 규칙, Claude 탐지용 공유 연결)로
타일을 열고, ssh 에 명령을 주어 로그인 셸에서 `cd D && claude …` 를 실행합니다. Claude 가 끝나도 셸은 남습니다.
서버의 저장된 사용자로 돌며 `--user` 는 쓸 수 없습니다. `--dir` 은 서버 쪽 절대 경로나 `~`/`~/…`(따옴표로 감싸기).
요청 `{"op":"open","server":"gpu-01","dir":"~/proj","resume":false,"skip_permissions":true,"wait":true,"prompt":"…"}`
→ `{"ok":true,"session_id":"pty_…","title":"gpu-01:proj","server":"gpu-01","claude":"bridge:session_…"|"이름"|null}`

- `claude` 는 원격 탐지(15초 간격)가 그 타일 아래의 Claude 를 본 뒤의 주소이며, 40초 안에 못 보면 `null` 입니다.
  Remote Control 이 꺼져 있으면 `bridge:` 주소가 없어 SendMessage 로 보낼 수 없습니다.
- POSIX 셸 서버만 됩니다(Windows 서버는 타일 안에 오류가 보입니다). 텔넷 서버는 거절합니다.

### 상태 — `nabiman-claude status <who> [--lines N] [--json]`
요청 `{"op":"status","who":"review","lines":40}` → `{"ok":true,"status":{…}}`

| 필드 | 뜻 |
|---|---|
| `session_id`, `title`, `label`, `owner` | 타일 id·이름·주소 라벨·소유 계정 |
| `server` | SSH 타일이면 저장된 서버 이름, 이 서버면 `null` |
| `alive`, `state` | `alive`·`ended`·`blocked`(서버 승인 변경)·`unknown` |
| `lead` | 운영자가 정한 총괄 타일인지 |
| `claude`, `name`, `busy`, `since` | 대표 Claude(작업 중인 것 우선) 주소·이름, 작업 중 여부, 그 상태가 된 시각(유닉스 초) |
| `model`, `version`, `cwd` | 마지막으로 답한 모델, Claude Code 버전, 작업 폴더(알 때만) |
| `peers` | 타일 안의 모든 Claude (`list` 의 항목과 같은 모양 + `ref`) |
| `lines`, `screen` | 화면 미리보기의 마지막 N 줄(기본 20, 최대 200, 빈 줄 제외, 보이는 화면 범위) |

### 닫기 — `nabiman-claude close <who> [--force] [--json]`
대시보드의 닫기와 같습니다: tmux 세션을 닫고 그 아래 프로세스에 SIGHUP, 끝난 것이 확인되면 기록을 지웁니다.
요청 `{"op":"close","who":"review-3a","force":false}`
→ `{"ok":true,"closed":{"session_id":"pty_…","title":"review","claude":"review-3a [946528]"},"audited":true}`
→ 거절 `{"ok":false,"error":"…","code":403|404|409|503,"audited":true}`

- 403: 자기 자신이 도는 타일(`--force` 로도 안 됨), 또는 남의 타일 · 404: 못 찾음/여럿 · 409: Claude 가 작업 중이거나 총괄 타일(`--force` 로 가능) · 503: 종료 확인 실패(기록 유지)
- 시도마다 감사 로그(`/api/audit`)에 남습니다: method `CLOSE`, path `claude.sock/sessions/<id>/close[?force=1]`,
  user `계정 (claude.sock: OS사용자 pid N in pty_호출타일)`, ip `local`.

### 안내 — `nabiman-claude guide`
요청 `{"op":"guide"}` → `{"ok":true,"guide":"…"}`. NabiMan 이 응답하지 않으면 `NABIMAN_GUIDE` 경로나
스크립트 옆 저장소의 `docs/orchestration/guide-ko.md` 를 읽습니다.

## HTTP (최고관리자 전용)

같은 핵심 함수를 씁니다. 브라우저는 타일 안에 있지 않으므로 "자기 타일" 거절은 없습니다. 감사 기록은 감사 미들웨어가 남깁니다.

- `GET /api/terminal/sessions/{id}/status?lines=N` → `{"success":true,"data":{…status…}}`, 못 찾으면 404
- `POST /api/terminal/sessions/{id}/close?force=1` → 200 `{"session_id","title"}`, 거절은 403/404/409/503

## 안내문 HTTP — `GET /api/orchestration/guide[?session=pty_…]`

로그인한 계정 누구나(원격 회원은 403). 대시보드 "AI 안내" 버튼용입니다.
→ `{"success":true,"data":{"text":"…","lead":{"address":"nabiman-5d [1eb0c4]","tile":"nabiman"}|null,"updated":"2026-11-10"}}`

- `lead` 는 운영자가 정한 총괄(`data_dir/claude-lead`) 타일의 Claude 주소입니다(그 타일에 Claude 가 보일 때만).
- `session` 을 주면 그 세션(이 계정에 보이는 것만, 아니면 404)에 보내는 2–3줄 머리말이 `text` 앞에 붙습니다.
- `updated` 는 안내문 마지막 줄의 `_갱신: YYYY-MM-DD_` 입니다. 안내문을 고치면 이 날짜도 고칩니다(60줄 이하).
