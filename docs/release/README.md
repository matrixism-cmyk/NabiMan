# 릴리스·설치·업데이트

## 설치 위치

`install.sh`, `make install`, 컨테이너 이미지가 모두 같은 위치를 씁니다. 문서와 예시도 이 위치를 기준으로 합니다.

| 경로 | 내용 |
|---|---|
| `/usr/local/bin/nabiman-server` | API·웹 서버 |
| `/usr/local/bin/nabiman-privileged-helper` | root helper(설치만 합니다. 서비스는 Phase 4에서 켭니다) |
| `/usr/local/bin/nabiman-recovery`, `/usr/local/bin/management-recovery.py` | 오프라인 복구 도구 |
| `/usr/local/share/nabiman/static` | 웹 화면 |
| `/var/lib/nabiman` | 데이터: 회원, 키, DB(0700, root) |
| `/etc/nabiman/nabiman.env` | 이 서버만의 설정(`NABIMAN_PORT=…` 등). unit 파일은 고치지 않습니다 |
| `/etc/nabiman/release.pub` | 릴리스 서명 공개 키(업데이트가 서명을 이 키로 확인합니다) |
| `/etc/systemd/system/nabiman.service` | 서비스 unit(패키지의 `nabiman.service`) |

## 실행에 필요한 패키지

`install.sh`가 확인하고, 없으면 배포판별 설치 명령을 알려 준 뒤 멈춥니다. `--install-deps`를 주면 직접 설치합니다(`--with-optional`을 더하면 선택 항목까지).

| 명령 | 용도 | apt (Debian/Ubuntu) | dnf (Fedora/RHEL) |
|---|---|---|---|
| `tmux` | 웹 터미널 세션 | `tmux` | `tmux` |
| `ssh`, `ssh-keygen` | 원격 서버 접속, 키 | `openssh-client` | `openssh-clients` |
| `sshpass` | 비밀번호 SSH 접속 | `sshpass` | `sshpass`(RHEL 계열은 EPEL) |
| `socat` | 포트 중계 | `socat` | `socat` |
| `guacd` (선택) | 브라우저 RDP/VNC 콘솔 | `guacd` | `guacd`(EPEL) |
| `soffice` (선택) | Office 문서 미리보기 | `libreoffice-core` | `libreoffice-core` |
| `ffmpeg` (선택) | 동영상 미리보기 | `ffmpeg` | `ffmpeg-free` |

## 서비스 unit

`nabiman.service`는 지금 `User=root`로 돕니다. API가 시작한 모든 프로세스, 곧 웹 터미널의 root 셸까지 unit의 제한을 그대로 물려받기 때문에, 서버 관리 자체를 막는 systemd 샌드박스 옵션(`ProtectSystem`, `ProtectHome`, `PrivateTmp`, `NoNewPrivileges` 등)은 넣지 않았습니다. 옵션마다 넣지 않은 이유는 unit 파일 주석에 있습니다. 넣은 것은 다음과 같습니다.

- `KillMode=process`: 재시작·업데이트 때 API만 멈추고 터미널의 tmux 세션은 남겨, 새 API가 같은 세션에 다시 붙습니다(기본값이면 재시작마다 모든 터미널이 끊깁니다).
- `StateDirectory=nabiman`(0700), `ConfigurationDirectory=nabiman`.
- `LimitCORE=0:infinity`: API 메모리(JWT 비밀, 복호화한 자격 증명)가 core 파일로 남지 않게 합니다. 터미널에서 필요하면 `ulimit -c`로 올릴 수 있습니다.

API를 root가 아닌 사용자로 옮기는 일은 Phase 4입니다([서비스 제어 경계](../commercialization/privileged-service-control.md), `packaging/nabiman-api-separated.service.example`). 그때 전체 샌드박스를 겁니다.
