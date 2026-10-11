# NabiMan - Server Management Dashboard

Linux 서버를 웹 브라우저에서 통합 관리하는 대시보드.
프론트엔드는 React(TypeScript), 백엔드는 Rust(Actix-web)로 구성되어 있으며,
백엔드 단일 바이너리로 배포 가능합니다.

## 로그인 세션 관리

**최고관리자 → 로그인 세션**에서 회원의 로그인을 조회·종료할 수 있습니다.
일반 회원과 tenant 관리자는 **소속 관리 → 로그인 세션**에서 본인 로그인만 관리합니다.
현재 로그인을 유지하면서 본인의 다른 로그인을 모두 종료할 수 있습니다.

## 2단계 인증

**최고관리자 / 소속 관리 → 2단계 인증**에서 로컬 로그인용 인증 앱을 등록할 수 있습니다.
QR 코드나 설정 키를 등록한 뒤 복구 코드 10개를 다운로드하고, 인증 앱 번호로 확인하세요.
등록은 10분 안에 완료해야 하며 복구 코드는 각각 한 번만 사용할 수 있습니다.
등록·해제 시 모든 로그인이 종료됩니다. 복구 코드 재발급 시에는 다른 로그인을 종료하고
현재 로그인은 유지해 새 코드를 보관할 수 있게 합니다. 외부 SSO·LDAP 계정의 MFA는 인증 제공자에서 설정합니다.

**최고관리자 / 소속 관리 → tenant·서버 운영 설정 → 소속 회원의 2단계 인증 필수**에서 소속별 정책을 지정합니다.
활성 소속 중 하나라도 필수이면 해당 회원은 MFA 등록과 재로그인을 마칠 때까지 계정 보안 설정만 사용할 수 있습니다.
기존 로그인에도 적용되며 정책 적용 중에는 MFA를 해제할 수 없습니다. 소속 관리자는 먼저 본인의 MFA를 완료해야 정책을 켤 수 있습니다.
최고관리자의 복구 접근은 유지합니다. 외부 SSO·LDAP의 MFA 보장은 아직 검증하지 않으므로 필수 정책 대상 외부 계정은 접근이 차단됩니다.

소속 설정 저장에는 **변경 사유**가 필요합니다. 다른 관리자가 먼저 저장했다면 덮어쓰지 않고 충돌을 알립니다.
작성한 내용을 확인한 뒤 최신 설정을 다시 불러와 검토하세요. 같은 화면의 **최근 정책 변경 이력**에서
변경자·사유·변경 전후 값을 확인할 수 있습니다. 해당 소속 관리자와 최고관리자에게 최근 100회 저장을 제공합니다.
이전 화면/API 클라이언트는 새로고침하거나 `expected_revision`과 `change_reason`을 보내도록 갱신해야 합니다.

## 실행 버전 확인

로그인 후 **모니터링 → 네트워크 진단 → 실행 중인 서버 버전**에서 실행 파일 SHA-256,
인스턴스 ID와 시작 시각을 확인할 수 있습니다. 배포 기록의 바이너리 지문과 비교하면
업데이트 반영 여부를 확인할 수 있습니다. 이 정보는 실행 프로세스가 보고하는 식별 정보이며,
서명된 릴리스나 독립적인 무결성 증명을 대신하지 않습니다.

## 주요 기능

| 기능 | 설명 |
|------|------|
| **서버 상태** | CPU, 메모리, 디스크 사용률, Uptime, Load Average 실시간 모니터링 |
| **네트워크** | 인터페이스 목록, IP/MAC, RX/TX 트래픽, DNS 서버, 연결 수 |
| **컨테이너 관리** | Docker 컨테이너 목록, 시작/중지/재시작/삭제, 로그 조회, 이미지 관리 |
| **서비스 관리** | systemd 서비스 목록, 시작/중지/재시작, enable/disable, 상태 필터링 |
| **방화벽 관리** | ufw/firewalld/iptables 자동 감지, 규칙 조회/추가/삭제, 포트 관리 |
| **계정 관리** | 시스템 계정 조회, 생성, 삭제, 비밀번호 변경, 접속 상태 확인 |
| **프로세스 모니터** | CPU/메모리 상위 프로세스, PID/사용자별 필터, TERM/KILL 시그널 |
| **디스크/스토리지** | 파티션별 사용량, 마운트 포인트, Disk I/O (iostat/proc) |
| **설정 관리** | 레지스트리 기반 범용 설정 에디터, 자동 백업, 설정 검증, 서비스 재시작 |
| **트래픽 모니터** | 인터페이스별 실시간 RX/TX 속도, TCP 연결 상태(ESTABLISHED/LISTEN/TIME_WAIT) |
| **패키지 관리** | 설치된 패키지 목록, 검색, 설치, 제거 (apt/yum/dnf 자동 감지) |
| **로그 뷰어** | journalctl 기반, 유닛/우선순위 필터, 자동 갱신, 키워드 검색 |
| **스케줄 작업** | cron 작업 목록, 추가/삭제, 사용자별 관리 |
| **웹 터미널** | xterm.js 기반 풀 터미널, 256색/Truecolor, 복사/붙여넣기, 폰트 크기 조절 |
| **원격 서버 관리** | 원격 서버 등록/수정/삭제, SSH 프로브 상태 확인, 원격 커맨드 실행, 터미널 연동 |
| **SSH 접속** | 웹 터미널에서 원격 서버 SSH 접속 (host, port, user 지정) |
| **인증** | 토큰 기반 로그인, 모든 API 보호, 웹 UI 비밀번호 변경 |

## 프로젝트 구조

```
NabiMan/
├── backend/                  # Rust (Actix-web)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # 서버 진입점, 인증 미들웨어, 정적 파일 서빙
│       ├── models.rs         # 데이터 모델 (요청/응답)
│       ├── auth.rs           # 토큰 인증 (로그인/로그아웃)
│       ├── server_status.rs  # 서버 상태 API
│       ├── network.rs        # 네트워크 상태 API
│       ├── accounts.rs       # 계정 관리 API
│       ├── config_manager.rs # 범용 서비스 설정 관리 API (레지스트리 패턴)
│       ├── processes.rs      # 프로세스 모니터 API
│       ├── disks.rs          # 디스크/스토리지 API
│       ├── remote_servers.rs # 원격 서버 인벤토리 관리 API
│       ├── traffic.rs        # 트래픽 모니터링 API
│       ├── packages.rs       # 패키지 관리 API
│       ├── containers.rs     # Docker 컨테이너 관리 API
│       ├── services.rs       # systemd 서비스 관리 API
│       ├── firewall.rs       # 방화벽 관리 API (ufw/firewalld/iptables)
│       ├── logs.rs           # 시스템 로그 조회 API (journalctl)
│       ├── cron.rs           # Cron 스케줄 작업 관리 API
│       └── terminal.rs       # WebSocket 터미널 (PTY + SSH)
└── frontend/                 # React (TypeScript)
    ├── package.json
    └── src/
        ├── App.tsx           # 메인 레이아웃 (탭 네비게이션)
        ├── App.css           # 다크 테마 스타일
        ├── types/index.ts    # TypeScript 타입 정의
        ├── hooks/useApi.ts   # API 호출 훅 (인증 토큰 자동 첨부)
        └── components/
            ├── LoginScreen.tsx      # 로그인 화면
            ├── ServerStatusPanel.tsx # 서버 상태
            ├── NetworkPanel.tsx      # 네트워크
            ├── AccountsPanel.tsx     # 계정 관리
            ├── ConfigPanel.tsx       # 설정 에디터
            ├── TrafficPanel.tsx      # 트래픽 모니터
            ├── PackagesPanel.tsx     # 패키지 관리
            ├── ContainersPanel.tsx   # 컨테이너 관리
            ├── ServicesPanel.tsx     # 서비스 관리
            ├── FirewallPanel.tsx     # 방화벽 관리
            ├── LogsPanel.tsx        # 로그 뷰어
            ├── CronPanel.tsx        # 스케줄 작업
            ├── ProcessesPanel.tsx   # 프로세스 모니터
            ├── DisksPanel.tsx       # 디스크/스토리지
            ├── RemoteServersPanel.tsx # 원격 서버 관리
            └── TerminalPanel.tsx     # 웹 터미널 + SSH
```

모든 소스 파일은 **400줄 이하**로 유지됩니다.

## 빠른 시작

### 요구사항

- Rust 1.88+
- Node.js 20.19+

### 빌드 및 실행

```bash
# 1. 프론트엔드 빌드
cd frontend
npm install
npm run build

# 2. 빌드 결과물을 백엔드 static 디렉토리로 복사
mkdir -p ../backend/static
cp -r build/* ../backend/static/

# 3. 백엔드 빌드 및 실행
cd ../backend
cargo build --release
read -rsp 'Initial admin password (12+ characters): ' NABIMAN_PASSWORD; echo
export NABIMAN_PASSWORD
./target/release/nabiman-server
```

브라우저에서 `http://서버IP:8080` 접속 후 위에서 설정한 비밀번호로 로그인합니다. 최초 설치에는 기본 비밀번호가 없습니다.

### 원라인 배포

```bash
read -rsp 'Initial admin password (12+ characters): ' NABIMAN_PASSWORD; echo
export NABIMAN_PASSWORD
cd frontend && npm install && npm run build && \
mkdir -p ../backend/static && cp -r build/* ../backend/static/ && \
cd ../backend && cargo build --release && \
./target/release/nabiman-server
```

### 릴리스 패키지

태그 릴리스는 Linux x86_64용 `nabiman-linux-x86_64.tar.gz`와 SHA-256 검사 파일을 게시합니다. 묶음에는 서버 실행 파일과 웹 화면의 `static` 파일이 함께 들어갑니다. 릴리스가 게시된 후 해당 릴리스의 `install.sh`를 실행하면 검사 파일로 무결성을 확인한 뒤 둘 다 설치합니다. 현재 설치 스크립트의 사전 빌드 패키지는 x86_64만 지원합니다.

## 환경변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `NABIMAN_PORT` | `8080` | 서버 포트 |
| `NABIMAN_PASSWORD` | 없음 | 최초 설치 시 필수인 12자 이상 관리자 비밀번호. 기존 `users.json`이 있으면 해당 계정 정보를 사용 |
| `NABIMAN_STATIC` | `./static` | 프론트엔드 정적 파일 경로 |
| `NABIMAN_DATA_DIR` | `/var/lib/nabiman` | 데이터 저장 경로 (원격 서버 목록 등) |

## API 엔드포인트

### 인증
| Method | Path | 설명 |
|--------|------|------|
| POST | `/api/auth/login` | 로그인 (password → token) |
| POST | `/api/auth/logout` | 로그아웃 |
| POST | `/api/auth/change-password` | 비밀번호 변경 (current_password, new_password) |

### 서버
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/server/status` | CPU, 메모리, 디스크, 업타임 |

### 네트워크
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/network/status` | 인터페이스, DNS, 연결 수 |

### 계정
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/accounts` | 계정 목록 |
| POST | `/api/accounts` | 계정 생성 |
| DELETE | `/api/accounts` | 계정 삭제 |
| POST | `/api/accounts/password` | 비밀번호 변경 |

### 설정 (레지스트리 기반 - 범용)
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/config/services` | 등록된 서비스 목록 (상태 포함) |
| GET | `/api/config/{service_id}` | 서비스 설정 파일 조회 |
| POST | `/api/config/{service_id}` | 서비스 설정 파일 수정 (자동 백업) |
| POST | `/api/config/{service_id}/restart` | 서비스 재시작 |
| POST | `/api/config/{service_id}/validate` | 설정 문법 검증 (apache/nginx/mysql/sshd/php-fpm) |

> `service_id`: apache, tomcat, nginx, mysql, postgresql, redis, php-fpm, sshd
> 새 서비스 추가: `config_manager.rs`의 `service_registry()`에 정의만 추가

### 프로세스
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/processes` | 프로세스 목록 (CPU 정렬, 상위 200개) |
| POST | `/api/processes/kill` | 프로세스 시그널 전송 (TERM/KILL/HUP 등) |

### 디스크
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/disks/status` | 파티션 사용량 + Disk I/O |

### 원격 서버
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/remote-servers` | 등록된 원격 서버 목록 |
| POST | `/api/remote-servers` | 원격 서버 등록 |
| PUT | `/api/remote-servers/{id}` | 원격 서버 정보 수정 |
| DELETE | `/api/remote-servers/{id}` | 원격 서버 삭제 |
| POST | `/api/remote-servers/{id}/check` | SSH 프로브 상태 확인 (OS, CPU, Memory, Disk) |
| POST | `/api/remote-servers/{id}/exec` | 원격 커맨드 실행 |
| POST | `/api/remote-servers/check-all` | 전체 서버 일괄 상태 확인 |

### 트래픽
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/traffic/current` | 실시간 트래픽 (인터페이스별) |
| GET | `/api/traffic/summary` | 트래픽 요약 + TCP 상태 |

### 패키지
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/packages/installed` | 설치된 패키지 목록 |
| POST | `/api/packages/search` | 패키지 검색 |
| POST | `/api/packages/install` | 패키지 설치 |
| POST | `/api/packages/remove` | 패키지 제거 |

### 컨테이너
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/containers` | 컨테이너 목록 |
| GET | `/api/containers/images` | 이미지 목록 |
| POST | `/api/containers/start` | 컨테이너 시작 |
| POST | `/api/containers/stop` | 컨테이너 중지 |
| POST | `/api/containers/restart` | 컨테이너 재시작 |
| POST | `/api/containers/remove` | 컨테이너 삭제 |
| POST | `/api/containers/logs` | 컨테이너 로그 조회 |
| POST | `/api/containers/images/remove` | 이미지 삭제 |

### 서비스
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/services` | systemd 서비스 목록 |
| POST | `/api/services/action` | 서비스 제어 (start/stop/restart/enable/disable) |

### 방화벽
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/firewall/status` | 방화벽 상태 + 규칙 목록 |
| POST | `/api/firewall/add` | 규칙 추가 |
| POST | `/api/firewall/delete` | 규칙 삭제 |

### 로그
| Method | Path | 설명 |
|--------|------|------|
| POST | `/api/logs` | 로그 조회 (unit, priority, lines 필터) |
| GET | `/api/logs/units` | 사용 가능한 유닛 목록 |

### 스케줄 작업
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/cron` | Cron 작업 목록 |
| POST | `/api/cron/add` | Cron 작업 추가 |
| POST | `/api/cron/delete` | Cron 작업 삭제 |

### 터미널
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/terminal?token=xxx` | WebSocket 로컬 터미널 |
| GET | `/api/terminal?token=xxx&ssh_host=..&ssh_user=..&ssh_port=..` | SSH 원격 접속 |

## 웹 터미널 단축키

| 단축키 | 동작 |
|--------|------|
| `Ctrl+Shift+C` | 선택 영역 복사 |
| `Ctrl+Shift+V` | 클립보드 붙여넣기 |
| `Ctrl+=` | 폰트 크기 확대 |
| `Ctrl+-` | 폰트 크기 축소 |
| 우클릭 (선택 있음) | 복사 |
| 우클릭 (선택 없음) | 붙여넣기 |

## 서비스 설정 확장 방법

새로운 서비스(예: MongoDB, HAProxy 등)의 설정 관리를 추가하려면:

`backend/src/config_manager.rs`의 `service_registry()` 함수에 정의만 추가하면 됩니다.
프론트엔드 코드 변경은 필요 없습니다.

```rust
ServiceDefinition {
    id: "mongodb".into(),
    display_name: "MongoDB".into(),
    config_paths: vec!["/etc/mongod.conf".into()],
    systemd_names: vec!["mongod".into()],
    process_name: "mongod".into(),
    is_running: None,
    config_found: None,
},
```

설정 검증이 필요한 경우 `validate_config()` 함수에 해당 서비스의 검증 커맨드를 추가합니다.

현재 등록된 서비스:
- **Apache HTTP Server** - httpd.conf / apache2.conf
- **Apache Tomcat** - server.xml
- **Nginx** - nginx.conf
- **MySQL / MariaDB** - my.cnf / mysqld.cnf
- **PostgreSQL** - postgresql.conf
- **Redis** - redis.conf
- **PHP-FPM** - www.conf
- **SSH Server** - sshd_config

## 보안 참고사항

- 신규 설치 시 고유한 `NABIMAN_PASSWORD`를 설정하세요. Docker Compose는 실행 전에 이 환경변수를 요구하며, systemd에서는 `/etc/nabiman/nabiman.env`에 설정할 수 있습니다.
- 기존 관리자 계정이 공개된 이전 기본 비밀번호를 사용한다면 시작 전에 새 `NABIMAN_PASSWORD`를 설정하세요. 서버가 계정 해시를 교체한 뒤 시작합니다.
- `operator`는 서비스·컨테이너 등 운영 작업을 수행할 수 있지만 루트 파일, 웹 셸, cron, 백업 복원, 패키지 설치, 원격 명령 및 인증 설정에는 접근할 수 없습니다. `viewer`는 상태 조회용이며 파일 다운로드와 설정 본문 조회가 제한됩니다.
- HTTPS(TLS) 적용을 권장합니다 (Nginx/Caddy 리버스 프록시)
- 백엔드는 root 권한으로 실행해야 계정 관리, 패키지 관리 등이 동작합니다
- SSH 파라미터는 입력값 검증을 통해 명령어 인젝션을 방지합니다
- `NABIMAN_DATA_DIR`의 `users.json`과 `jwt_secret`은 인증 자료입니다. 백업을 제한된 권한으로 보관하세요.
- 이전 버전에서 저장한 `users.json`에 `password_hash`가 없다면 서버는 안전하게 시작을 중단합니다. 백업에서 계정 정보를 복원한 뒤 재시작하세요.

## 기술 스택

- **Backend**: Rust, Actix-web 4, sysinfo, nix (PTY), actix-web-actors (WebSocket)
- **Frontend**: React 18, TypeScript, xterm.js, CSS custom properties (다크 테마)


## 지원용 진단 보고서

최고관리자는 **모니터링 → 네트워크 진단 → 지원용 진단 보고서**에서 자료를 생성하고 전체 JSON을 검토한 뒤 다운로드할 수 있습니다. 실행 바이너리 지문, 자원·저장소 상태, 회원·접속·최근 작업의 집계만 포함하며 계정 이름, 서버 주소, 비밀번호·키, 원본 로그와 터미널 내용은 수집하지 않습니다. 외부 자동 전송은 없습니다.

항목별 수집 시점은 다를 수 있고 작업은 최근 1,000건까지만 집계합니다. `busy`, `unavailable`, `too_large`는 정상이나 0건을 뜻하지 않습니다. 연결 시험·DB 무결성 검사·보안 인증을 대신하지 않으며 수집 간격은 최소 5초입니다.

최고관리자용 **개선 리포트 (외부)** 버튼은 클릭할 때만 별도 출처의 프레임을 엽니다. 외부 스크립트를 나비맨 문서에 직접 실행하지 않으며, 로그인 토큰·진단 자료·화면 내용·현재 URL을 전달하지 않습니다. 화면 요소 자동 선택과 외부 요청에 의한 나비맨 화면 이동은 지원하지 않습니다.


## 로컬 파일 예약 백업

최고관리자는 **관리 → 백업 → 스케줄**에서 절대 경로와 UTC cron을 지정하고 **지금 실행**으로 결과를 확인할 수 있습니다. 숫자·목록·범위·`*/간격`을 지원하며, 일자와 요일을 모두 제한하면 둘 중 하나가 일치할 때 실행합니다. 예약 실행은 한 번에 하나이며 같은 분의 실행을 중복 접수하지 않습니다. 중단 중 놓친 예약은 자동 재실행하지 않습니다.

실행 접수와 완료를 구분하며 성공·실패·중단을 저장합니다. 이전 버전의 실행 시각만 있는 기록은 성공으로 간주하지 않습니다. 아카이브는 `0600` 권한으로 완료 후 공개하고, 실행당 5분·압축 결과 1 GiB 제한을 적용합니다. 비압축은 `.tar`, 압축은 `.tar.gz`입니다. 잘못된 저장 파일은 덮어쓰지 않고 예약 기능을 중단합니다. 운영자가 파일과 디스크 상태를 복구한 뒤 서비스를 재시작해야 합니다.

이는 선택한 **로컬 파일 백업**입니다. 실행 중인 DB나 전체 나비맨 관리 데이터의 일관된 복구, 원격 서버 백업, 외부 보관·암호화·보존 정책을 대신하지 않습니다. 생성 성공은 복원 검증 완료를 의미하지 않습니다. 화면의 복원 버튼은 단일 `.bak` 파일용이며, 아카이브는 별도 빈 디렉터리에서 내용과 권한을 검증한 후 적용해야 합니다. 전체 관리 데이터의 일관된 복구는 별도 미완료 과제입니다.


## 인증 암호화 백업과 검증 복원

**관리 → 백업 → 암호화 백업 · 검증 복원**에서 age v1 형식의 `.tar.gz.age` 파일을 생성합니다. 비밀번호는 12자 이상이어야 하며 별도로 안전하게 보관해야 합니다. 비밀번호를 서버 설정·로그·외부 명령 인수에 저장하지 않으며 분실한 비밀번호는 복구할 수 없습니다. 보호 수준은 비밀번호의 예측 불가능성에 영향을 받습니다.

일반 파일과 디렉터리를 암호화 스트림에 직접 넣으므로 생성 과정에 평문 임시 아카이브를 만들지 않습니다. 압축 해제 기준 1 GiB(아카이브 메타데이터 포함), 100,000항목·깊이 64까지 지원합니다. 링크·특수 파일·중복/겹친 경로는 거부하며 변경 중인 파일의 크기·수정 시각 변화도 검사합니다. 이는 파일시스템 snapshot이나 실행 중인 DB의 일관된 백업을 보장하지 않습니다.

복원은 전체 age 인증과 압축·경로 검사를 마친 후 **아직 존재하지 않는 새 디렉터리**에만 공개합니다. 잘못된 비밀번호·변조·잘림·상위 경로·링크·특수 파일·중복 항목은 거부하며 기존 파일을 덮어쓰지 않습니다. 복원한 일반 파일은 0600, 디렉터리는 0700이고 현재 API 실행 계정이 소유합니다. 원래 소유자·실행 권한·ACL·xattr는 복구하지 않으므로 검토 후 별도로 적용해야 합니다. 생성 성공이 원격 보관이나 재해 복구 완료를 뜻하지 않습니다.

복원 중 검증용 평문은 0700 작업 디렉터리의 0600 파일에 저장합니다. 정상 종료/오류 시 정리하지만 강제 종료·디스크 오류로 숨김 `.private-backup-*` 디렉터리가 남을 수 있습니다. 실행 중인 작업이 없는지 확인한 뒤 운영자가 정리해야 합니다. 물리적 보안 삭제를 보장하지 않습니다. 암호화/복원은 프로세스당 한 건씩 처리하며 scrypt work factor는 생성·복원 최대 18입니다. 더 큰 work factor 또는 다른 수신자 방식의 외부 age 파일은 이 API에서 지원하지 않습니다.

### 이전 CBC `.enc` 파일 복구

이전 파일은 그대로 보존하지만 인증 태그가 없는 CBC 형식을 웹 API로 자동 복원하지 않습니다(422). 운영 데이터에는 이번 전환 시 `.enc` 파일이 없었습니다. 기존 파일을 보유한 설치는 **원본을 별도 보존하고**, 알려진 출처·기존 외부 체크섬을 확인한 뒤 격리된 비특권 계정에서 복구합니다. CBC 복호화 성공만으로 파일의 진위나 무결성을 증명할 수 없습니다.

```sh
umask 077
mkdir -m 700 legacy-review
openssl enc -d -aes-256-cbc -pbkdf2 -in old-backup.tar.gz.enc -out legacy-review/archive.tar.gz
# 비밀번호는 OpenSSL 프롬프트에 입력합니다. 명령 인수/환경 변수에 넣지 않습니다.
tar -tzf legacy-review/archive.tar.gz
mkdir -m 700 legacy-review/restored
# 목록과 출처를 검토한 일반 파일의 상대 경로만 지정합니다.
tar -xzf legacy-review/archive.tar.gz --no-same-owner --no-same-permissions --keep-old-files -C legacy-review/restored -- '검토한_일반_파일의_상대경로'
```

복구 내용과 권한을 검토한 후 새 age 백업을 만들고 실제 복원을 시험합니다. 기존 원본은 그 검증이 끝나기 전에 삭제하지 않습니다. 위 절차는 파일 복구이며 나비맨 전체 상태 복구 절차를 대신하지 않습니다.


## 관리 데이터 오프라인 복구

**관리 → 백업 → 관리 데이터 복구 · 운영자 도구**에서 운영 안내를 엽니다. 배포 패키지에는 `nabiman-recovery`와 `management-recovery.py`가 함께 들어갑니다. 운영자가 API를 정지한 뒤 auth 저장소·SQLite DB·실행 바이너리·정적 UI를 하나의 age 묶음으로 캡처할 수 있습니다. 도구는 서비스를 자동으로 정지하거나 복원본을 활성화하지 않습니다. 실행 중인 API의 저장소 소유 잠금을 획득할 수 없으면 캡처를 거부합니다.

```sh
umask 077
python3 /opt/nabiman/management-recovery.py capture \
  --auth /var/lib/nabiman --database /var/lib/nabiman-mec/mec.db \
  --runtime /opt/nabiman/nabiman-server --static /opt/nabiman/static \
  --output /var/backups/nabiman-recovery
python3 /opt/nabiman/management-recovery.py restore \
  --archive /var/backups/nabiman-recovery/실제파일명.tar.gz.age \
  --target /srv/nabiman-restored
python3 /opt/nabiman/management-recovery.py verify --target /srv/nabiman-restored
```

경로는 설치 환경에 맞게 지정합니다. 암호는 대화형 프롬프트 또는 이미 열린 파일 디스크립터로만 전달하고 인수·환경 변수에 넣지 않습니다. DB는 SQLite backup API로 복사하며 manifest의 파일 목록·해시·크기, DB quick_check, 회원 ID, 필수 인증 자료를 검증합니다. 원격 비밀번호가 있으면 함께 복원한 `secret.key`로 실제 해독 가능한지도 검사합니다. 검증을 모두 통과한 새 경로만 공개하며 기존 디렉터리는 덮어쓰지 않습니다. 서버 실행 파일만 0700으로 복구하고 일반 파일은 0600으로 정규화합니다.

캡처에는 비공개 평문 작업 디렉터리가 필요합니다. 정상 종료 시 정리하지만 강제 종료 후 `.management-capture-*` 또는 복원 staging이 남을 수 있습니다. 암호화 저장소나 안전한 삭제를 보장하지 않습니다. 전체 묶음은 확장 기준 1 GiB 제한을 따릅니다.

이는 나비맨이 관리하는 로컬 상태 복구입니다. 외부 공급자 설정·환경 변수·kubeconfig·OS SSH 자격 증명·TLS/프록시·원격 서버 데이터·기존 백업 아카이브·실행 중인 PTY는 별도 복구 대상입니다. 복원된 예약·알림·외부 연동을 검토한 다음 격리 환경에서 기동·로그인·소속 권한을 확인해야 합니다. 암호화 묶음과 복구 암호를 별도 안전한 외부 위치에 보관하고 다른 호스트에서 정기적으로 시험하는 절차는 별도로 필요합니다.


## 감사 기록 저장 상태·원본 검사·내보내기

최고관리자의 **보안 → 감사 로그**에서 저장 상태와 보관 파일 검사를 확인합니다. 이 전역 기록과 전송 설정은 일반 회원·서버/tenant 소속 관리자에게 공개하지 않습니다. 소속별 MEC 감사와 전역 HTTP 요청 기록은 서로 다른 범위입니다.

새 기록은 단일 writer에서 append와 fsync가 성공한 뒤 최근 목록에 반영합니다. 저장 오류가 발생하면 writer를 중단하고 상태 API와 `X-NabiMan-Audit: unavailable` 응답 헤더로 알립니다. 이미 완료된 작업은 되돌리지 않으며 모든 변경을 미리 차단하는 transaction audit 기능은 아닙니다. 저장소를 점검·복구한 뒤 서버를 재시작해야 기록을 재개합니다.

현재 파일과 최대 5개의 약 10 MB 회전 파일을 검사합니다. 기존 형식 오류·해시 누락·해시 불일치·잘린 마지막 줄은 구분하여 표시하고 원본을 임의로 수정하지 않습니다. 과거의 파싱 불가능한 줄은 최근 목록에서 제외되지만 원본과 검증 결과에 남습니다. 새 레코드는 필드 경계를 구분하는 v2 해시를 사용하고 이전 해시와 이어서 검증합니다. 회전 파일별로 체인은 다시 시작하므로 삭제된 과거 파일·보존 기간 밖의 기록·root의 전체 재작성까지 증명하지 않습니다. 독립 서명 감사 저장소를 대신하지 않습니다.

파일별 내보내기는 당시 원본 SHA-256과 검사 결과 및 파싱 가능한 기록을 담습니다. 경로 쿼리와 공유 capability를 제거하고 개별 원래 해시를 없앤 JSON이므로 원본 체인을 재구성하는 증거 파일과 동일하지 않습니다. 형식 오류 줄은 원본에만 남습니다. 미리보기는 처음 20건이며 사용자명·IP·경로 등 민감 정보 주의사항을 확인한 뒤 다운로드합니다. 전역 웹 삭제 API는 410을 반환합니다. 법적 보존·삭제 정책, 독립 불변 원격 저장·조직별 전체 감사 집행은 별도 후속 작업입니다.


## HTTPS 감사 전송과 복원 격리

최고관리자는 **보안 → 감사 로그 → HTTPS 전송**에서 수신기를 등록합니다. 등록 직후에는 비활성이며 시험 전송은 대기열 접수만 뜻합니다. HTTP 2xx 확인 횟수와 마지막 오류를 확인한 후 활성화합니다. 활성화하면 전역 요청 기록의 사용자명·IP·경로·상태가 해당 수신기로 전송됩니다. URL 경로에도 비밀을 넣지 마십시오. Authorization만 `secret.key`로 암호화하여 보관하며 API에 다시 노출하지 않습니다. 키가 사라졌을 때 기존 암호문이 있으면 새 키로 대체하지 않습니다.

HTTPS 인증서와 호스트명을 검증하고 redirect를 따라가지 않습니다. 사설 CA는 서버 환경 변수 `NABIMAN_AUDIT_CA_FILE`에 PEM 파일 경로를 설정합니다. 전달할 프록시 환경 변수는 사용하지 않습니다. 수신기 최대 8개, SQLite 영속 대기열 최대 5,000건·payload 합계 32 MiB·개별 payload 16 KiB입니다. SQLite 파일 자체의 크기 한도와는 다릅니다. 실패 시 지수 지연으로 최대 8회 시도하고 차단 상태로 유지하며 관리자가 재시도를 선택합니다. 동일 배달의 Idempotency-Key를 재사용하지만 수신기도 중복을 제거해야 합니다. 2xx는 수신 응답 확인이지 수신 측 영구 보관 증명이 아닙니다.

대기열 저장 실패는 누락 카운터 또는 저장소 오류와 `X-NabiMan-Audit-Forwarding: not-queued`로 표시합니다. 완료된 관리 작업을 되돌리지 않으며 로컬 감사 기록을 자동 재수집하지 않습니다. 비활성화·삭제는 기존 대기를 취소하고 이미 진행 중인 HTTP 요청은 끝날 수 있습니다. 주소·자격 증명 변경은 비활성화 후 삭제·재등록합니다. 기존 `audit_forwarding.json`은 원본을 보존하지만 자동 활성화하지 않으므로 관리자 검토가 필요합니다.

복원 사본의 첫 기동에는 반드시 `NABIMAN_AUDIT_FORWARDING_DISABLED=1`을 설정하여 이전 대기열이 외부로 재전송되지 않게 합니다. 복원된 목적지·자격 증명·미완료 대기열을 검토하고 운영 전환 시에만 해제합니다. 이 변수는 신규 전송기의 발송만 멈추며 예약 작업이나 구버전 실행 파일의 외부 연동까지 막지 않으므로 격리 네트워크와 별도 검토가 필요합니다. 관리 데이터 복구 검증에는 전송 DB와 암호화된 Authorization의 해독 가능성도 포함합니다.

## 만료형 읽기 자동화 토큰

최고관리자의 **보안 → 엔터프라이즈 인증 → API 키 관리**에서 발급합니다. 현재 버전은 발급한 최고관리자 본인에게 연결된 읽기 토큰이며 독립 service account가 아닙니다. 역할 전체를 넘기는 대신 `status:read`(GET `/api/server/status`), `servers:read`(GET `/api/remote-servers`), `tenants:read`(GET `/api/mec/v1/tenants` 및 `/{id}`, `/{id}/resources`, `/{id}/quota-usage`)를 선택합니다. 그 외 URL·HTTP method는 허용하지 않습니다. 회원·설정 변경, 터미널·명령 실행·키 재발급에는 사용할 수 없습니다.

유효 기간은 필수 1~90일이며 기본 화면 값은 7일입니다. 자동화마다 별도로 발급하고 필요한 최소 범위를 선택합니다. HTTPS 요청에 `Authorization: Bearer <token>` 또는 `X-API-Key: <token>` 하나만 전달합니다. URL이나 로그·스크린샷에 비밀을 넣지 않습니다. 서버는 256-bit 난수의 SHA-256만 저장하며 평문은 발급 응답에서 한 번만 표시합니다. 목록에는 prefix·범위·소유자·만료·마지막 사용이 나옵니다. 기존 역할 기반 키는 `legacy_disabled`로 표시하고 자동 활성화하지 않습니다.

요청마다 소유자의 불변 ID·계정 상태·현재 권한과 발급 당시 인증/권한 지문을 확인합니다. 소유자의 비밀번호·MFA·역할·소속·접근 버전 변경은 재발급을 요구합니다. 같은 이름으로 재가입한 회원에게 이전 토큰이 승계되지 않습니다. 토큰 호출은 감사 기록에 token ID와 owner ID로 남기며 원문 비밀은 기록하지 않습니다. 마지막 사용은 인가가 통과한 시각이며 대상 API의 업무 성공을 뜻하지 않습니다.

회수는 저장 완료 후 응답하며 다음 요청부터 거부합니다. 이미 인가된 요청을 취소하지 않습니다. 토큰 저장 실패가 발생하면 토큰 인증/변경을 중지하고 운영자가 저장 상태를 점검한 뒤 재시작해야 합니다. JWT 회원 로그인과 복구 권한은 별도입니다. 최대 1,000개 레코드·2 MiB 저장소 한도를 적용합니다.

복원 사본에는 `NABIMAN_API_TOKENS_DISABLED=1`로 토큰 인증을 중지하고 로그인은 기존 JWT 흐름으로 검증합니다. 과거 백업에는 이후 회수된 토큰도 남을 수 있으므로 외부 노출 전 목록을 검토·회수한 뒤 이 설정을 해제합니다. 이전 버전 실행 파일에는 해당 환경 변수가 적용되지 않습니다. 독립 서비스 계정, tenant 관리자 발급, 쓰기 action별 토큰, 단기 workload 인증서는 후속 작업입니다.

## 배포 구성 명세와 패키지 검증

**모니터링 → 진단**의 실행 정보 아래에서 의존성 SBOM(CycloneDX 1.6)과 배포 구성 명세를 확인합니다. 현재 실행 파일 SHA-256과 명세의 서버 해시를 비교하며, 다르면 정상 일치로 표시하지 않습니다. 명세는 패키징 당시 Cargo.lock·package-lock.json 기준으로 개발·선택·다른 플랫폼 의존성을 포함합니다. 실제 바이너리/브라우저 번들에 들어간 모든 구성의 측정 결과가 아니며 OS 라이브러리·npm peer graph·라이선스 검토·CVE 검사도 완전하지 않습니다.

`package-release.sh`는 `release-bom.cdx.json`과 `release-inventory.json`을 프런트 빌드 디렉터리에 생성하고, 패키지 내부의 모든 일반 파일(자기 자신인 manifest 제외)을 `release-manifest.json`의 해시·크기·권한과 대조합니다. Python 3.11 이상 또는 Python 3.10 + tomli가 명세 생성에 필요합니다. 패키지와 별도로 SBOM·manifest·inventory JSON 및 `verify-release.py`를 생성합니다. 릴리스 CI는 공식 CycloneDX 스키마의 고정 해시를 확인한 뒤 스키마/format 검증도 수행합니다.

```sh
# 검증 스크립트는 신뢰하는 소스에서 확보합니다. 아카이브 안의 코드를 먼저 실행하지 않습니다.
python3 scripts/verify-release.py nabiman-linux-x86_64.tar.gz
python3 scripts/verify-release.py nabiman-linux-x86_64.tar.gz --extract-to ./verified-release
```

검증은 누락·추가·중복·변조 파일, 경로 이탈, 링크·특수·sparse 파일, 특권 mode, 20,000개 항목/확장 2 GiB 초과를 거부합니다. 추출 대상은 새 디렉터리여야 하며 Linux의 atomic no-replace rename을 사용합니다. 추출 중에도 파일 해시를 재검사합니다. 기존 디렉터리를 덮어쓰지 않습니다.

새 installer는 다운로드 체크섬 확인에 더해 릴리스와 함께 배포되는 검증기로 검사한 뒤 설치합니다. 새 manifest와 verifier가 없는 구형 릴리스에는 적용되지 않으며 무검증 추출로 되돌아가지 않습니다. 이 installer를 게시하려면 같은 릴리스에 새 형식의 패키지와 `verify-release.py` 자산도 게시해야 합니다. 이 작업에서 GitHub 태그/공개 릴리스를 발행한 것은 아닙니다.

동일 출처의 체크섬·검증기·manifest는 **배포자의 암호학적 신원을 증명하지 않습니다**. 이들은 파일 일관성 검사이며 서명, 독립 attestation, source-to-binary 연결 또는 재현 빌드 증거가 아닙니다. 서명된 릴리스·신뢰 루트·CVE 승인·라이선스 검토는 별도 출시 요건으로 유지합니다.


### 의존성 보안 갱신과 남은 항목 (2026-10-06)

TLS·인증서·XML·SSH 및 npm 전이 의존성을 갱신했습니다. 고정 RustSec DB 검사에서 취약점 항목은 12개에서 2개, npm audit은 4개 패키지에서 0개로 감소했습니다. 이는 해당 검사 시점/데이터베이스의 결과이며 전체 보안 인증이 아닙니다. Cargo 빌드·시험은 `--locked`, 프런트 CI/릴리스에는 실패 시 중단하는 `npm audit`을 적용합니다. Rust audit은 아래 잔여 항목 때문에 여전히 실패 상태이며 이를 0건으로 숨기거나 무시하지 않습니다.

- `h2 0.3.27`의 [RUSTSEC-2026-0258](https://rustsec.org/advisories/RUSTSEC-2026-0258.html): Actix 의존성에 남습니다. 현재 서버는 HTTP/1 `.bind()`를 사용하며 h2c/TLS 자동 HTTP/2 수신을 켜지 않았습니다. 프록시나 클라이언트 등 전체 환경의 HTTP/2 안전성을 보증하지 않으며, Actix 호환 수정 또는 의존성 제거가 남습니다.
- `rsa 0.10.0-rc.18`의 [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html): 수정 버전이 아직 없습니다. 실제 Rust SSH 경로는 서버 공개키 검증 후 비밀번호 인증을 사용하고 RSA 개인키 복호화/서명을 제공하지 않습니다. 원격 터미널의 OpenSSH 개인키 처리는 별도 구현입니다. 향후 Rust 개인키 인증 도입 시 다시 평가해야 하며, RSA 버전 상승만으로 해결됐다고 간주하지 않습니다.

SSH는 `russh 0.64.1`로 갱신했고 기존 신뢰 지문과 제한된 구형 알고리즘 예외를 유지합니다. 암호화 백업은 ML-KEM 의존성 충돌을 피하기 위해 상류 수정 릴리스 `age 0.11.5`를 고정합니다. 서비스가 사용하는 age v1 passphrase/scrypt 형식을 유지하며 PQ recipient 기능을 제공한다고 주장하지 않습니다. `backoff`, `derivative`, `instant`, `proc-macro-error2`, `rustls-pemfile`의 유지보수 중단 경고 5개도 남아 있습니다. 잔여 위험과 상용 출시 판단은 별도로 추적합니다.


검증된 백업 호환 범위: 직전 운영 age 0.12.1 복구 도구와 새 0.11.5 도구 사이의 양방향 관리 데이터 복원, Go age v1.2.1과의 양방향 passphrase 파일 호환을 실제 실행 파일로 확인했습니다. 복원 후 관리자/하위 회원 로그인·소속/정책·암호화 키·DB가 유지됐습니다. 임의의 다른 age 기능/버전까지 호환된다는 뜻은 아닙니다. 패키지 smoke는 격리된 HTTP/1 백엔드에서 HTTP/2 preface가 400으로 거부되는지도 검사합니다.


운영 반영: 2026-10-06 06:11 KST `dependency-security-20261006-v1`. backend 444개·frontend 170개 시험, 실제 공개 브라우저 회귀와 실행 digest/SBOM 일치 검증을 통과했습니다. 새 구성 목록은 1,377개이며 **모니터링 → 진단**에서 확인합니다. 운영 서버 6개·등록 터미널 16개·tmux 17개·기존 인증 키를 보존했습니다. 자세한 잔여 항목과 복구 검증 범위는 [실행 기록](docs/commercialization/execution.md)을 참고하십시오.


### 인증 시도 제한

로그인·LDAP 로그인·MFA 확인은 계정 이름(소문자 정규화)의 해시 기준으로 5분 동안 실패/진행 중 요청 5개를 허용합니다. 대소문자만 다른 이름은 같은 제한 그룹을 사용합니다. 같은 계정을 여러 주소에서 시도해도 제한을 공유하며, 같은 프록시의 다른 계정은 별도 계정 한도를 사용합니다. 인증된 비밀번호/MFA 설정은 검증된 사용자 ID, 가입 신청은 실제 소켓 접속 주소별로 각각 5분 5개 제한입니다. 성공한 인증은 해당 요청의 계정 예약만 제거하고 과거 실패를 지우지 않습니다. 가입 신청은 성공해도 신청 수에 포함됩니다.

추가로 모든 대상 인증 요청을 실제 접속 주소별 1분 60개·프로세스 전체 1분 300개로 제한합니다. 성공·잘못된 본문·차단된 계정 재시도도 이 총량에 포함됩니다. 전달 헤더는 신뢰하지 않으므로 reverse proxy 뒤의 사용자는 주소별 총량을 공유합니다. 계정 잠금으로 다른 계정이 바로 차단되는 문제를 줄였지만 전체 과부하나 특정 계정을 노린 일시적인 접근 방해까지 제거하지는 않습니다.

동시 처리 16개, 해시 bucket 4,096개, 인증 본문 32 KiB·읽기 3초 한도를 적용합니다. 만료 전 기록을 다른 요청 때문에 퇴출하지 않으며, 한도 초과에는 `429`, `Retry-After`, `Cache-Control: no-store`를 반환합니다. 로그인·가입 화면은 재시도까지 남은 시간을 표시하고 자동으로 비밀번호를 다시 전송하지 않습니다. 상태는 worker 간 공유되는 단일 프로세스 메모리이며 재시작 시 초기화됩니다. 다중 노드 제한·영속 공격 이력·신뢰 프록시 기반 개별 IP 구분·외부 IdP/네트워크 DDoS 방어는 별도 범위입니다.


인증 제한 운영 반영: 2026-10-06 06:28 KST `auth-admission-20261006-v1`. 공개 HTTPS에서 존재하지 않는 임시 이름으로 제한 응답·Retry-After·브라우저 남은 시간/제출 차단·새로고침 후 제한 유지와 정상 관리자 로그인을 확인했습니다. 실제 운영 계정에 잘못된 비밀번호를 반복하거나 MFA 설정을 바꾸지 않았습니다. backend 451개·frontend 172개 시험과 기존 가입/회원/터미널 회귀 통과, 브라우저 오류 0건입니다.

로컬 OS 계정은 **보안 → 사용자 계정**에서 관리합니다. 나비맨 가입 회원/tenant 소속과 별개이며, OS 변경은 최고관리자만 가능합니다. 삭제는 홈 보존이 기본이고 홈 삭제에는 별도 선택과 계정명 확인이 필요합니다. 관찰한 UID가 바뀐 계정, root/시스템/공유 UID, 기존 홈 재사용은 변경을 거부합니다. 자세한 검증 범위와 제약은 [상용화 실행 기록](docs/commercialization/execution.md)을 참고하세요.

**시스템 → 예약 작업**에서는 본 서버의 사용자 crontab을 관리합니다. 추가 전 기존 설정과 실행 계정을 검토하고, 삭제는 표시된 작업을 확인해야 합니다. 검토 이후 설정이 바뀌거나 기존 설정을 읽지 못하면 변경을 거부합니다. 저장 후 실제 crontab을 다시 읽어 확인하지만 예약 명령의 향후 실행까지 보장하지는 않습니다.


예약 작업 편집을 2026-10-06 07:57 KST 운영에 반영했다. 시스템 → 예약 작업에서 수정 전·후와 기존 환경을 검토하고, 확인한 revision에 대해 해당 행만 수정한다. 검증 및 배포 증거는 [상용화 실행 기록](docs/commercialization/execution.md)을 따른다.
