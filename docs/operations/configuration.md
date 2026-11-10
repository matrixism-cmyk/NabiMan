# 설치 환경 설정

나비맨은 어느 서버에 설치해도 같은 방식으로 동작해야 합니다. 이 문서는 설치마다 달라지는 값을 어디에 어떻게 적는지, 각 설정의 기본값이 무엇인지, 그리고 예전에 코드에 박혀 있던 "이 서버만의 값"(경로, 도메인, IP 대역, 계정)을 어떤 설정으로 바꿨는지 정리합니다.

## 설정 파일

설정은 **환경 변수**(`NABIMAN_*`)와 **설정 파일** 하나로 합니다. 둘 다 선택입니다. 아무것도 정하지 않아도 새 서버에서 안전한 기본값으로 동작합니다.

### 위치

서버가 시작할 때 아래 순서로 처음 찾은 파일 **하나만** 읽습니다.

1. `NABIMAN_CONFIG` 환경 변수가 가리키는 파일. 값이 비어 있거나 `none`이면 설정 파일을 읽지 않습니다. 가리킨 파일이 없으면 서버가 시작하지 않습니다(오타를 조용히 넘기지 않기 위해서).
2. `/etc/nabiman/nabiman.toml`
3. `데이터 디렉터리/nabiman.toml` (기본 `/var/lib/nabiman/nabiman.toml`). 컨테이너처럼 `/etc`가 다시 시작할 때 초기화되는 곳에서는 이 위치를 씁니다.

### 우선순위

**환경 변수 > 설정 파일 > 기본값** 입니다. 같은 설정이 환경 변수와 파일에 모두 있으면 환경 변수가 이깁니다. 그래서 systemd `Environment=`나 시작 스크립트에서 쓰던 기존 환경 변수는 이름 그대로 계속 동작합니다.

동작 방식: 시작하자마자(스레드가 하나뿐일 때) 파일을 읽고, 환경에 아직 없는 설정만 프로세스 환경에 넣습니다. 그 뒤로는 모든 코드가 환경 변수를 읽으므로 파일에 적은 값과 환경 변수로 준 값이 똑같이 취급됩니다. 서버가 실행하는 프로그램(셸 등)도 이 환경을 물려받으므로 **비밀번호·토큰 같은 비밀은 설정 파일에 적지 말고** 각자의 비밀 파일 설정(`NABIMAN_SECRET_KEY_FILE`, `NABIMAN_DNS_DB_DEFAULTS_FILE` 등)을 쓰세요.

### 형식

TOML입니다. 키 이름이 곧 환경 변수 이름입니다.

- `port = 18080` → `NABIMAN_PORT`
- 표(table) 안의 키는 `_`로 이어 붙입니다: `[dictation]` 아래 `url = "…"` → `NABIMAN_DICTATION_URL`, `[mec] lb_pools = "…"` → `NABIMAN_MEC_LB_POOLS`
- `-`는 `_`로 바뀝니다: `trusted-proxies` → `NABIMAN_TRUSTED_PROXIES`
- 이미 `NABIMAN_`으로 시작하는 대문자 키는 그대로 씁니다: `NABIMAN_HSTS = true`
- 값: 문자열, 숫자, 참/거짓. 목록(`["a", "b"]`)은 쉼표로 이어 붙인 값이 됩니다(항목 안에 쉼표 금지).

예시 파일: [`nabiman.example.toml`](nabiman.example.toml).

### 지켜야 할 것

- 파일은 root(또는 서버를 실행하는 계정)의 소유여야 하고, 그룹·다른 사용자가 쓸 수 없어야 합니다(`chmod 644` 또는 `600`). 이 파일은 서버가 실행할 프로그램 경로를 정하므로, 다른 사람이 고칠 수 있으면 서버가 시작을 거부합니다.
- 문법 오류, 같은 설정 두 번, 목록 안의 표처럼 읽을 수 없는 내용이 있으면 서버가 이유를 출력하고 시작하지 않습니다.
- `NABIMAN_CONFIG`는 파일 안에 쓸 수 없습니다. 데이터 디렉터리 안의 파일에는 `NABIMAN_DATA_DIR`도 쓸 수 없습니다.
- 나비맨이 모르는 이름(오타일 가능성)은 시작 로그에 경고로 남고, 값은 그대로 환경에 들어갑니다.
- 시작 로그에 `Settings: <파일> (N from the file, M overridden by the environment)` 한 줄이 남습니다. `GET /api/diagnostics/runtime`의 `settings`에도 파일 위치와 **이름만**(값 없음) 나옵니다.

## 설정 목록

"파일 키"는 설정 파일에 쓰는 이름입니다(최상위 키로 쓸 때). 같은 설정을 환경 변수로 주면 파일보다 우선합니다.

### 위치와 기본

| 파일 키 | 환경 변수 | 기본값 | 하는 일 |
|---|---|---|---|
| — | `NABIMAN_CONFIG` | `/etc/nabiman/nabiman.toml`, 없으면 `데이터 디렉터리/nabiman.toml` | 읽을 설정 파일. 환경 변수로만 지정 |
| `data_dir` | `NABIMAN_DATA_DIR` | `/var/lib/nabiman` | 데이터 디렉터리([데이터와 백업](data-and-backup.md)) |
| `static` | `NABIMAN_STATIC` | `./static` | 웹 화면(빌드한 프런트엔드) 폴더 |
| `listen` | `NABIMAN_LISTEN` | `127.0.0.1:<port>` | 서버가 듣는 주소. `127.0.0.1:18080`, `[::1]:8080`, 주소만(`0.0.0.0` → 포트는 `NABIMAN_PORT`), `localhost`. 기본은 루프백뿐이라 같은 서버의 TLS 프록시(nginx 등)를 거쳐야 닿습니다. 다른 서버에서 바로 닿게 하려면 `0.0.0.0`을 **명시**해야 하고, 그때 시작 로그에 경고가 남습니다(나비맨은 평문 HTTP). 호스트 이름(`localhost` 말고)이나 읽을 수 없는 값이면 서버가 시작하지 않습니다 |
| `port` | `NABIMAN_PORT` | `8080` | 서버가 듣는 포트. `listen`에 포트가 없을 때 씁니다(둘 다 포트를 주면 `listen`의 포트) |
| `secret_key_file` | `NABIMAN_SECRET_KEY_FILE` | `데이터 디렉터리/secret.key` | 저장 비밀번호 암호화 키 파일 |
| `helper_socket` | `NABIMAN_HELPER_SOCKET` | `/run/nabiman-helper/control.sock` | 권한 도우미 소켓 |
| `claude_socket` | `NABIMAN_CLAUDE_SOCKET` | `/run/nabiman/claude.sock` | Claude 세션 지휘 소켓 |
| `license_public_keys_file` | `NABIMAN_LICENSE_PUBLIC_KEYS_FILE` | `/etc/nabiman/license-public-keys.json` | 라이선스 서명 공개키 |

### 로그인과 웹

| 파일 키 | 환경 변수 | 기본값 | 하는 일 |
|---|---|---|---|
| — (비밀) | `NABIMAN_PASSWORD` | 없음 | 첫 관리자 비밀번호. 비밀이므로 파일에 쓰지 말고 시작 스크립트가 비밀 파일에서 읽어 넘기세요 |
| `api_tokens_disabled` | `NABIMAN_API_TOKENS_DISABLED` | 꺼짐 | `1`이면 공개 API 키 로그인 중지 |
| `cors_origins` | `NABIMAN_CORS_ORIGINS` | 없음(CORS 끔) | 다른 출처에서 API를 부를 수 있게 할 출처 목록 |
| `trusted_proxies` | `NABIMAN_TRUSTED_PROXIES` | `127.0.0.1,::1` | `X-Forwarded-*`를 믿을 프록시 주소 |
| `csp` | `NABIMAN_CSP` | 내장 정책 | `off`면 CSP를 보내지 않음, 다른 값이면 그 값으로 바꿈 |
| `hsts` | `NABIMAN_HSTS` | 신뢰 프록시가 HTTPS라고 할 때만 | `true` 항상, `false` 보내지 않음 |
| `product_name` | `NABIMAN_PRODUCT_NAME` | `NabiMan` | 브라우저 탭 제목(`GET /api/branding`) |
| `feedback_origin` | `NABIMAN_FEEDBACK_ORIGIN` | 없음(위젯 꺼짐) | 개선 리포트 위젯 패널의 출처(`https://…`). CSP `frame-src`에 이 출처만 더함 |
| `feedback_site` | `NABIMAN_FEEDBACK_SITE` | 없음(위젯 꺼짐) | 그 서비스에서 이 사이트의 id. 출처와 둘 다 있어야 위젯이 켜짐 |
| `feedback_script` | `NABIMAN_FEEDBACK_SCRIPT` | `/fixhub-admin.js`(내장 런처) | 위젯 런처 스크립트 주소. 다른 출처면 그 출처를 CSP `script-src`에 더함 |
| `noc_contact` | `NABIMAN_NOC_CONTACT` | 없음(표시 안 함) | MEC NOC 화면 경보 띠에 보이는 당직 연락처 |

### 한도

| 파일 키 | 환경 변수 | 기본값 | 하는 일 |
|---|---|---|---|
| `upload_max_mb` | `NABIMAN_UPLOAD_MAX_MB` | `10240` | 업로드 한 건의 최대 크기(MB) |
| `max_sessions_per_user` | `NABIMAN_MAX_SESSIONS_PER_USER` | `100` | 계정당 터미널 세션 수 |
| `sftp_max_connections` | `NABIMAN_SFTP_MAX_CONNECTIONS` | `256` | 동시 SFTP 연결 수 |

### 백업과 감사

| 파일 키 | 환경 변수 | 기본값 | 하는 일 |
|---|---|---|---|
| `default_backup_schedule` | `NABIMAN_DEFAULT_BACKUP_SCHEDULE` | `17 3 * * *`(UTC) | 새 설치의 기본 백업 예약. `off`면 만들지 않음 |
| `default_backup_keep` | `NABIMAN_DEFAULT_BACKUP_KEEP` | `7` | 기본 예약이 남길 백업 수 |
| `audit_ca_file` | `NABIMAN_AUDIT_CA_FILE` | 시스템 인증서 | 감사 기록 외부 전송에 쓸 CA 파일 |
| `audit_forwarding_disabled` | `NABIMAN_AUDIT_FORWARDING_DISABLED` | 꺼짐 | `1`이면 감사 기록 외부 전송을 멈춤 |

### 외부 프로그램과 서비스

| 파일 키 | 환경 변수 | 기본값 | 하는 일 |
|---|---|---|---|
| `soffice` | `NABIMAN_SOFFICE` | `PATH`의 `soffice.bin` → `/usr/lib/libreoffice/program/soffice.bin`, `/usr/lib64/…`, `/opt/libreoffice*/program/soffice.bin` → `PATH`의 `soffice` | Office 문서 미리보기(LibreOffice) |
| `rhwp` | `NABIMAN_RHWP` | `PATH`의 `rhwp` → `/usr/local/bin/rhwp`, `/opt/rhwp/bin/rhwp` | 한글(HWP/HWPX) 미리보기 |
| `preview_fonts` | `NABIMAN_PREVIEW_FONTS` | 없음(시스템 글꼴만) | 미리보기에 더할 글꼴 폴더. 안의 `fonts.conf`는 LibreOffice가 씀 |
| `claude` | `NABIMAN_CLAUDE` | `PATH` → `/usr/local/bin`, `/usr/bin`, `/opt/claude/bin` → 각 계정의 `~/.local/bin`, `~/.npm-global/bin`, `~/.claude/local` | Claude 음성 대화가 쓸 `claude`. 화면의 "claude 경로"가 있으면 그것이 우선 |
| `guacd_addr` | `NABIMAN_GUACD_ADDR` | `127.0.0.1:4822` | RDP 콘솔이 쓰는 guacd 주소 |
| `serial_group` | `NABIMAN_SERIAL_GROUP` | 없음(root만) | 브라우저 시리얼 장치를 함께 쓸 그룹 |
| `dictation_url` | `NABIMAN_DICTATION_URL` | 없음(받아쓰기 꺼짐) | 화면에서 받아쓰기 서비스 주소를 저장하지 않았을 때 쓸 주소. 비어 있으면 고객 음성이 어디에도 가지 않습니다 |
| `letsencrypt_email` | `NABIMAN_LETSENCRYPT_EMAIL` | 없음 | 새 인증서 발급에 쓰는 Let's Encrypt 계정 이메일. 없으면 발급을 거절하고 이유를 보여 줌(갱신은 됨) |
| `dns_check_domain` | `NABIMAN_DNS_CHECK_DOMAIN` | 없음(점검 안 함) | DNS 화면 상태 점검이 로컬 DNS에 물어볼 이름 |
| `dns_check_expect` | `NABIMAN_DNS_CHECK_EXPECT` | 없음(아무 주소나 답하면 정상) | 그 이름이 답해야 하는 주소 |
| `dns_db_defaults_file` | `NABIMAN_DNS_DB_DEFAULTS_FILE` | `데이터 디렉터리/nabidns-mysql.cnf` | nabidns DB 접속 정보 파일 |
| — (비밀) | `NABIMAN_DNS_DB_PASSWORD` | 없음 | nabidns DB 비밀번호. 위 파일을 쓰세요 |

### MEC

MEC 연동 설정의 자세한 뜻은 [MEC 운영 문서](../mec-management/07-runbook.md)에 있습니다. 비밀(토큰·비밀번호)은 설정 파일이 아닌 환경 변수로 주세요.

| 파일 키 | 환경 변수 | 기본값 | 하는 일 |
|---|---|---|---|
| `mec_mode` | `NABIMAN_MEC_MODE` | `auto` | `real`, `mock`, `auto` |
| `mec_db` | `NABIMAN_MEC_DB` | `데이터 디렉터리/mec.db` | MEC 데이터베이스 |
| `mec_docs` | `NABIMAN_MEC_DOCS` | `./data/mec_docs` | 만든 인수인계 문서 폴더 |
| `mec_read_only` | `NABIMAN_MEC_READ_ONLY` | 꺼짐 | 켜면 MEC 변경 요청을 막음 |
| `mec_skip_confirm` | `NABIMAN_MEC_SKIP_CONFIRM` | 꺼짐 | 변경 확인 단계를 건너뜀(시험용) |
| `mec_namespace` | `NABIMAN_MEC_NAMESPACE` | `default` | 기본 Kubernetes 네임스페이스 |
| `mec_kubeconfig` | `NABIMAN_MEC_KUBECONFIG` | 없음 | kubeconfig 파일 |
| `mec_lb_pools` | `NABIMAN_MEC_LB_POOLS` | 없음(빈 목록) | MetalLB 주소 풀. 쉼표로 나눈 `이름=시작-끝 [시작-끝…] [auto]`. 예: `tenant=10.0.0.151-10.0.0.200 auto,reserved=10.0.0.231-10.0.0.238` |
| `mec_dcgm_url` | `NABIMAN_MEC_DCGM_URL` | 없음 | GPU 지표(DCGM exporter) 주소 |
| `mec_rancher_url`, `mec_rancher_cluster`, `mec_rancher_insecure` | `NABIMAN_MEC_RANCHER_URL`, `NABIMAN_MEC_RANCHER_CLUSTER`, `NABIMAN_MEC_RANCHER_INSECURE` | 없음, `local`, 꺼짐 | Rancher API |
| — (비밀) | `NABIMAN_MEC_RANCHER_TOKEN` | 없음 | Rancher API 토큰 |
| `mec_harbor_url`, `mec_harbor_user`, `mec_harbor_insecure` | `NABIMAN_MEC_HARBOR_URL`, `NABIMAN_MEC_HARBOR_USER`, `NABIMAN_MEC_HARBOR_INSECURE` | 없음, 없음, 꺼짐 | Harbor 레지스트리 |
| — (비밀) | `NABIMAN_MEC_HARBOR_PASSWORD` | 없음 | Harbor 비밀번호 |
| `mec_axgate_host`, `mec_axgate_port`, `mec_axgate_user`, `mec_axgate_timeout` | `NABIMAN_MEC_AXGATE_HOST`, `NABIMAN_MEC_AXGATE_PORT`, `NABIMAN_MEC_AXGATE_USER`, `NABIMAN_MEC_AXGATE_TIMEOUT` | 없음, `2222`, 없음, `30`(초) | AXGATE 방화벽 CLI |
| — (비밀) | `NABIMAN_MEC_AXGATE_PASSWORD` | 없음 | AXGATE 비밀번호 |
| `mec_axgate_host_key_sha256`, `mec_axgate_legacy_until` | `NABIMAN_MEC_AXGATE_HOST_KEY_SHA256`, `NABIMAN_MEC_AXGATE_LEGACY_UNTIL` | 없음 | AXGATE 호스트 키 고정, 구식 키 허용 기한 |
| `mec_axgate_vpn`, `mec_axgate_debug`, `mec_axgate_dump_config` | `NABIMAN_MEC_AXGATE_VPN`, `NABIMAN_MEC_AXGATE_DEBUG`, `NABIMAN_MEC_AXGATE_DUMP_CONFIG` | 꺼짐 | VPN 세션 조회, 디버그 출력, 설정 덤프(값이 있으면 켜짐) |

## 새로 설치한 서버에서 달라지는 점

예전 빌드는 이 서버의 값을 기본으로 가졌습니다. 새 빌드를 새 서버에 설치하면:

- **받아쓰기·음성 대답이 꺼져 있습니다.** 음성 서비스 주소와 API 키를 넣기 전에는 마이크 버튼이 나오지 않고, 설정 화면에 "아직 음성 서비스가 설정되지 않았습니다"가 보입니다. 고객 음성이 우리 서비스(`v.bebo.kr`)로 가지 않습니다.
- **새 인증서 발급**(SSL 화면, 가상 호스트 "자동 SSL")은 `NABIMAN_LETSENCRYPT_EMAIL`을 정하기 전까지 거절되고, 이유를 보여 줍니다. 가상 호스트는 만들어지고 인증서만 건너뜁니다.
- **DNS 상태 점검**은 `NABIMAN_DNS_CHECK_DOMAIN`을 정하기 전까지 "점검 안 함"으로 보입니다.
- **MEC LB 주소 풀** 목록이 비어 있습니다.
- **개선 리포트 위젯**(오른쪽 아래 "개선 리포트 (외부)")이 나오지 않고, CSP도 외부 frame을 허용하지 않습니다.
- **MEC NOC 경보 띠**에 당직 연락처가 나오지 않습니다.
- LibreOffice, rhwp, Claude Code는 표준 위치와 `PATH`에서 찾습니다. 없으면 그 기능만 꺼지고 진단 화면에 표시됩니다.

## 외부 프로그램 점검

서버는 시작할 때 한 번 선택 프로그램을 확인하고 로그에 한 줄로 남깁니다(`External programs: tmux 3.8, ssh, …; not available: …`). 진단 화면(`네트워크 진단` 아래 "외부 프로그램")과 `GET /api/diagnostics/runtime`의 `tools`에도 같은 결과가 나옵니다. 요청을 처리하는 동안에는 확인하지 않습니다. "다시 확인"은 백그라운드에서 다시 확인하고 잠시 뒤 결과가 바뀝니다.

| 프로그램 | 확인 방법 | 없으면 꺼지는 기능 |
|---|---|---|
| `tmux` | `PATH`, 버전(`tmux -V`) | 터미널 |
| `ssh` | `PATH` | 원격 서버 SSH, 파일(SFTP) |
| `sshpass` | `PATH` | 비밀번호로 로그인하는 서버 |
| `ssh-keygen` | `PATH` | SSH 키 만들기, SSH 인증서 로그인 |
| `socat` | `PATH` | 브라우저 시리얼 장치 |
| `guacd` | `NABIMAN_GUACD_ADDR`에 TCP 연결(1초) | RDP 콘솔 |
| `soffice` | `NABIMAN_SOFFICE` 또는 표준 위치 | Office 문서 미리보기 |
| `rhwp` | `NABIMAN_RHWP` 또는 `PATH` | 한글 문서 미리보기(LibreOffice H2Orestart로 대신할 수 있으면 그것으로) |
| `ffmpeg` | `PATH` | 이 버전에서는 쓰는 기능 없음(표시만) |
| `claude` | `NABIMAN_CLAUDE` 또는 `PATH`·표준 위치 | Claude 음성 대화 |

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
| `frontend/src/**/*.test.ts(x)` | `/workspace/…` 경로, `spark.bebo.kr` | 테스트 자료 |
| `backend/src/mec/docgen/text.rs:48`, `docx.rs:61`, `docs/mec-management/**` | `rancher.jcia.mec.local`, `121.147.13.228` | MEC 모듈이 만드는 그 고객사(JCIA) 인수인계 문서와 그 운영 문서. MEC 연동 설정을 문서에 넣는 일은 다음 단계 |
| `backend/src/terminal/claude_socket.rs:11`, `claude_remote.rs:55`, `claude_peers_procs.rs:104`, `serial_bridge.rs:192` | 주석 속 예시 | 설명. 동작과 무관 |
| `frontend/src/components/terminal/dictation/uplink.ts:6`, `:25` | 주석에 음성 서비스 문서 위치 | 설명(문구만 중립으로 바꿈) |
| `nabi` 계정 | 코드에서 특정 계정을 가정하는 곳 없음 | 이 서버의 `NABIMAN_SERIAL_GROUP=nabi`는 시작 스크립트(저장소 밖)에서 설정 |
