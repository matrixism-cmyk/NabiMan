# 설치 환경 설정

나비맨은 어느 서버에 설치해도 같은 방식으로 동작해야 합니다. 이 문서는 예전에 코드에 박혀 있던 "이 서버만의 값"(경로, 도메인, IP 대역, 계정)을 모두 찾아 정리하고, 각각을 어떤 설정으로 바꿨는지와 새로 설치한 서버의 기본값을 적습니다.

## 하드코딩 값 목록

2026-11-10 기준 `backend/src`, `frontend/src`, `frontend/public`, `scripts/`에서 `/workspace`, `bebo.kr`, `xos.kr`, `man.bebo`, `v.bebo`, `fix.bebo`, `172.20.`, `115.68.`, `121.147.`, `nabi` 계정, `webmaster@`, 고정 이메일·IP·호스트 이름을 찾은 결과입니다.

### 바꾼 것

| 위치 | 내용 | 새 설정 | 새 기본값 |
|---|---|---|---|
| `backend/src/terminal/office_preview.rs:34` | LibreOffice 경로 `/workspace/opt/libreoffice-26.2.7/…/soffice.bin` | `NABIMAN_SOFFICE` (기존 이름 유지) | `PATH`의 `soffice.bin`/`soffice`, 없으면 `/usr/lib/libreoffice/program/soffice.bin`, `/opt/libreoffice*/program/soffice.bin` |
| `backend/src/terminal/office_preview.rs:36` | rhwp 경로 `/workspace/opt/rhwp-0.8.7/bin/rhwp` | `NABIMAN_RHWP` (기존 이름 유지) | `PATH`의 `rhwp` |
| `backend/src/terminal/office_preview.rs:39` | 한글 글꼴 폴더 `/workspace/opt/fonts-noto-kr` | `NABIMAN_PREVIEW_FONTS` | 없음(시스템 글꼴만) |
| `backend/src/terminal/console_drive.rs:16` | 데이터 디렉터리 두 번째 기본값 `/workspace/var/lib/nabiman` | 없음 — 다른 곳과 같은 데이터 디렉터리(`NABIMAN_DATA_DIR`, 기본 `/var/lib/nabiman`) | `/var/lib/nabiman/guac-drive` |
| `backend/src/terminal/dictation/mod.rs:20` | 받아쓰기 서비스 기본 주소 `https://v.bebo.kr` (고객 음성이 우리 서비스로 감) | `NABIMAN_DICTATION_URL` (저장된 서비스 주소가 없을 때) | 비어 있음 — 받아쓰기 "설정 안 됨" |
| `backend/src/terminal/claude_voice/launch.rs:13` | Claude Code 위치 `/workspace/.npm-global/bin/claude` | `NABIMAN_CLAUDE` (화면의 "claude 경로"가 비어 있을 때) | `PATH`, `/usr/local/bin`, `/usr/bin`, `/opt/claude/bin`, 각 계정의 `~/.local/bin` 등 |
| `backend/src/terminal/claude_voice/mod.rs:20` | 목소리 설명이 한국어로 박혀 있음 | 없음 — 서버는 목소리 id만 보내고 화면이 ko/en/ja로 번역 | — |
| `backend/src/ssl_jobs.rs:83`, `backend/src/vhost.rs:277` | Let's Encrypt 계정 이메일 `webmaster@xos.kr` | `NABIMAN_LETSENCRYPT_EMAIL` | 없음 — 새 인증서 발급은 설정할 때까지 분명한 오류로 거절 (갱신은 그대로 됨) |
| `backend/src/dns.rs:121`, `:128` | DNS 상태 점검이 `xos.kr`가 `115.68.193.237`인지 확인 | `NABIMAN_DNS_CHECK_DOMAIN`, `NABIMAN_DNS_CHECK_EXPECT` | 없음 — 점검 건너뜀("설정 안 됨" 표시) |
| `backend/src/mec/handlers/network.rs:18`, `:28` | MEC LB IP 풀 `172.20.26.151-200`, `172.20.26.231-238` | `NABIMAN_MEC_LB_POOLS` | 없음 — 풀 목록 비어 있음 |
| `backend/src/terminal/rdp_tunnel.rs:15` | guacd 주소 `127.0.0.1:4822` (표준값이지만 고정) | `NABIMAN_GUACD_ADDR` | `127.0.0.1:4822` |
| `backend/src/security_headers.rs:35`, `:154` | CSP `frame-src https://fix.bebo.kr` | `NABIMAN_FEEDBACK_ORIGIN` 에서 만듦 | 위젯 꺼짐 — `frame-src 'self'` |
| `frontend/public/index.html:30` | 모든 화면에서 `/fixhub-admin.js` 로드 | `GET /api/branding` 의 `feedback_widget` 이 있을 때만 화면이 넣음 | 넣지 않음 |
| `frontend/public/fixhub-admin.js:5-6` | 위젯 주소 `https://fix.bebo.kr`, 사이트 id | `NABIMAN_FEEDBACK_ORIGIN`, `NABIMAN_FEEDBACK_SITE` (스크립트 태그의 `data-` 속성으로 전달) | 꺼짐 |
| `frontend/src/components/mec/MecNocPanel.tsx:21` | NOC 담당자 `the@aeokorea.com` | `NABIMAN_NOC_CONTACT` (`/api/branding`) | 없음 — 표시 안 함 |
| `frontend/src/components/terminal/dictation/DictationSettings.tsx:90` | 입력 예시 `https://v.bebo.kr` | 없음 — `https://speech.example.com` | — |
| `frontend/src/i18n/translations/dictation.ts:20`, `claude.ts:30` | 안내문에 `v.bebo.kr` | 없음 — "음성 서비스"로 중립 표현 | — |
| `frontend/src/components/mec/FirewallNatForm.tsx:131`, `:158` | 입력 예시 `121.147.13.246`, `172.20.26.199` | 없음 — 문서용 주소 `203.0.113.10`, `10.0.0.10` | — |
| `frontend/src/components/remote/ClaudeStartDialog.tsx:80` | 입력 예시 `/workspace/project` | 없음 — `/srv/project` | — |
| `backend/src/mec/handlers/settings.rs:129` | systemd 예시의 AXGATE 주소 `121.147.13.228`, `jcia.mec.local` | 없음 — `192.0.2.10`, `mec.example` | — |
| `scripts/deploy-reviewed-runtime.py:12-18` | `/workspace/nabiman`, `/workspace/var/lib/nabiman`, supervisor 설정, playwright chromium, `https://man.bebo.kr` | 명령행 인자 / `NABIMAN_DEPLOY_*` 환경 변수 | 없음 — 필요한 값이 없으면 시작하지 않음 |
| `scripts/smoke-os-accounts.py:89`, `:93` | chromium `/workspace/nabi/.cache/ms-playwright/…` | `NABIMAN_CHROMIUM` | `~/.cache/ms-playwright/chromium-*/chrome-linux64/chrome` 중 하나 |
| `scripts/nabiman-claude:7-9` | 도움말 예시 `/workspace/project` | 없음 — `/srv/project` | — |

### 그대로 둔 것

| 위치 | 내용 | 이유 |
|---|---|---|
| `backend/src/mec/services/axgate_mock.rs`, `kube_mock.rs`, `kube_mock_fixtures.rs` | `121.147.13.x`, `172.20.26.x` | MEC 모의(mock) 모드의 예시 데이터. 실제 모드에서는 쓰지 않음 |
| `backend/src/mec/services/axgate_real/parser.rs`, `preprocess.rs`, `parser_tests.rs` | AXGATE 설정 예문 속 주소 | 파서 설명과 테스트 자료 |
| `backend/src/attack_watch.rs:269-286`, `terminal/topology.rs:226`, `terminal/claude_*_tests.rs`, `terminal/ai_agents.rs:61`, `terminal/claude_peers.rs:317`, `terminal/claude_socket.rs:355-358`, `terminal/serial_bridge.rs:310-314`, `claude_voice/mod.rs:228`, `claude_voice/launch.rs:133` | 테스트 안의 주소·경로·계정 이름(`nabi`, `jcia`) | 테스트 자료일 뿐 동작에 쓰이지 않음 |
| `frontend/src/**/*.test.ts(x)` | `/workspace/…` 경로 | 테스트 자료 |
| `backend/src/terminal/claude_socket.rs:11`, `claude_remote.rs:55`, `claude_peers_procs.rs:104`, `serial_bridge.rs:192` | 주석 속 예시 | 설명. 동작과 무관 |
| `frontend/src/components/terminal/dictation/uplink.ts:6`, `:25` | 주석에 음성 서비스 문서 위치 | 설명(문구만 중립으로 바꿈) |
| `nabi` 계정 | 코드에서 특정 계정을 가정하는 곳 없음 | 이 서버의 `NABIMAN_SERIAL_GROUP=nabi`는 시작 스크립트(저장소 밖)에서 설정 |
