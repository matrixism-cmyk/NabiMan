# NabiMan - Server Management Dashboard

Linux 서버를 웹 브라우저에서 통합 관리하는 대시보드.
프론트엔드는 React(TypeScript), 백엔드는 Rust(Actix-web)로 구성되어 있으며,
백엔드 단일 바이너리로 배포 가능합니다.

## 주요 기능

| 기능 | 설명 |
|------|------|
| **서버 상태** | CPU, 메모리, 디스크 사용률, Uptime, Load Average 실시간 모니터링 |
| **네트워크** | 인터페이스 목록, IP/MAC, RX/TX 트래픽, DNS 서버, 연결 수 |
| **계정 관리** | 시스템 계정 조회, 생성, 삭제, 비밀번호 변경, 접속 상태 확인 |
| **설정 관리** | Apache/Tomcat 설정 파일 웹 에디터, 자동 백업, 서비스 재시작 |
| **트래픽 모니터** | 인터페이스별 실시간 RX/TX 속도, TCP 연결 상태(ESTABLISHED/LISTEN/TIME_WAIT) |
| **패키지 관리** | 설치된 패키지 목록, 검색, 설치, 제거 (apt/yum/dnf 자동 감지) |
| **웹 터미널** | xterm.js 기반 풀 터미널, 256색/Truecolor, 복사/붙여넣기, 폰트 크기 조절 |
| **SSH 접속** | 웹 터미널에서 원격 서버 SSH 접속 (host, port, user 지정) |
| **인증** | 토큰 기반 로그인, 모든 API 보호 |

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
│       ├── config_manager.rs # Apache/Tomcat 설정 관리 API
│       ├── traffic.rs        # 트래픽 모니터링 API
│       ├── packages.rs       # 패키지 관리 API
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
| `NABIMAN_PASSWORD` | `nabiman` | 관리자 비밀번호 |
| `NABIMAN_STATIC` | `./static` | 프론트엔드 정적 파일 경로 |

## API 엔드포인트

### 인증
| Method | Path | 설명 |
|--------|------|------|
| POST | `/api/auth/login` | 로그인 (password → token) |
| POST | `/api/auth/logout` | 로그아웃 |

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

### 설정
| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/config/apache` | Apache 설정 조회 |
| POST | `/api/config/apache` | Apache 설정 수정 |
| POST | `/api/config/apache/restart` | Apache 재시작 |
| GET | `/api/config/tomcat` | Tomcat 설정 조회 |
| POST | `/api/config/tomcat` | Tomcat 설정 수정 |
| POST | `/api/config/tomcat/restart` | Tomcat 재시작 |

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

## 보안 참고사항

- 운영 환경에서는 반드시 `NABIMAN_PASSWORD`를 변경하세요
- HTTPS(TLS) 적용을 권장합니다 (Nginx/Caddy 리버스 프록시)
- 백엔드는 root 권한으로 실행해야 계정 관리, 패키지 관리 등이 동작합니다
- SSH 파라미터는 입력값 검증을 통해 명령어 인젝션을 방지합니다

## 기술 스택

- **Backend**: Rust, Actix-web 4, sysinfo, nix (PTY), actix-web-actors (WebSocket)
- **Frontend**: React 18, TypeScript, xterm.js, CSS custom properties (다크 테마)
