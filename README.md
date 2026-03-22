# NabiMan - Server Management Dashboard

Linux 서버를 웹 브라우저에서 통합 관리하는 대시보드.
프론트엔드는 React(TypeScript), 백엔드는 Rust(Actix-web)로 구성되어 있으며,
백엔드 단일 바이너리로 배포 가능합니다.

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

- Rust 1.70+
- Node.js 18+

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
./target/release/nabiman-server
```

브라우저에서 `http://서버IP:8080` 접속, 기본 비밀번호: `nabiman`

### 원라인 배포

```bash
cd frontend && npm install && npm run build && \
mkdir -p ../backend/static && cp -r build/* ../backend/static/ && \
cd ../backend && cargo build --release && \
NABIMAN_PASSWORD=mypassword ./target/release/nabiman-server
```

## 환경변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `NABIMAN_PORT` | `8080` | 서버 포트 |
| `NABIMAN_PASSWORD` | `nabiman` | 관리자 초기 비밀번호 (웹 UI에서 변경 가능) |
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

- 운영 환경에서는 반드시 `NABIMAN_PASSWORD`를 변경하세요
- HTTPS(TLS) 적용을 권장합니다 (Nginx/Caddy 리버스 프록시)
- 백엔드는 root 권한으로 실행해야 계정 관리, 패키지 관리 등이 동작합니다
- SSH 파라미터는 입력값 검증을 통해 명령어 인젝션을 방지합니다

## 기술 스택

- **Backend**: Rust, Actix-web 4, sysinfo, nix (PTY), actix-web-actors (WebSocket)
- **Frontend**: React 18, TypeScript, xterm.js, CSS custom properties (다크 테마)
