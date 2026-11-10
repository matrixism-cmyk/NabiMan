# 릴리스·설치·업데이트

## 설치 위치

릴리스마다 `/usr/local/lib/nabiman/<버전>/`에 따로 풀어 두고, 아래의 고정 경로는 `current`를 가리키는 링크입니다. systemd unit과 문서의 경로는 버전이 바뀌어도 그대로입니다.

```
/usr/local/lib/nabiman/v1.2.0/        검증한 패키지 그대로(실행 파일, static, 도구, unit)
/usr/local/lib/nabiman/v1.1.0/
/usr/local/lib/nabiman/current  -> v1.2.0     지금 쓰는 버전
/usr/local/lib/nabiman/previous -> v1.1.0     --rollback이 돌아갈 버전
```

`install.sh`, `make install`, 컨테이너 이미지가 모두 같은 위치를 씁니다. 문서와 예시도 이 위치를 기준으로 합니다.

| 경로 | 내용 |
|---|---|
| `/usr/local/bin/nabiman-server` | API·웹 서버 |
| `/usr/local/bin/nabiman-privileged-helper` | root helper(설치만 합니다. 서비스는 Phase 4에서 켭니다) |
| `/usr/local/bin/nabiman-recovery`, `/usr/local/bin/management-recovery.py` | 오프라인 복구 도구 |
| `/usr/local/bin/nabiman-update` | 업데이트·되돌리기(아래) |
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

## 업데이트와 되돌리기

```sh
sudo nabiman-update                    # 최신 릴리스로
sudo nabiman-update --version v1.3.0   # 특정 릴리스로
sudo nabiman-update --archive ./nabiman-linux-x86_64.tar.gz   # 인터넷 없는 서버(옆에 .minisig)
sudo nabiman-update --rollback         # 바로 전 버전으로
nabiman-update --list                  # 설치된 버전, current/previous 표시
```

`nabiman-update`가 하는 일:

1. 패키지와 `.minisig`를 받습니다(`--base-url`로 사내 미러 지정 가능).
2. **지금 돌고 있는 버전의** `verify-release.py`와 `/etc/nabiman/release.pub`로 서명과 manifest를 확인합니다. 받은 파일 안의 코드나 키는 믿지 않습니다. 서명이 없거나 맞지 않으면 멈춥니다(`--insecure-skip-verify`는 시험용).
3. 현재보다 낮은 버전이면 멈춥니다(`--allow-downgrade`, 또는 `--rollback`).
4. `/usr/local/lib/nabiman/<버전>/`에 풀고 `current`를 바꾼 뒤(`previous`는 직전 버전) 서비스를 재시작합니다. root helper 서비스가 켜져 있으면 그것도 재시작합니다.
5. 상태를 확인합니다: systemd의 MainPID가 새 실행 파일인지, 그리고 `GET /api/healthz`가 200이거나(이 경로가 생기면) `GET /`이 웹 화면을 주고 `GET /api/server/status`가 401/200으로 API가 직접 답하는지. 기본 60초(`--health-timeout`), 주소는 `/etc/nabiman/nabiman.env`의 `NABIMAN_PORT`(`--health-url`로 바꿀 수 있음).
6. 확인에 실패하면 **스스로 이전 버전으로 되돌리고** 재시작·확인한 뒤 실패로 끝납니다. 실패한 버전은 조사용으로 남깁니다.
7. 성공하면 최근 N개(`--keep`, 기본 3)와 current/previous만 남기고 오래된 버전을 지웁니다. 데이터(`/var/lib/nabiman`)는 건드리지 않습니다.

`--rollback`은 `previous`로 바꾸고 재시작·확인합니다. 그것도 답하지 않으면 그대로 두고 실패를 알립니다(두 버전 사이를 오가지 않습니다).

예전 `install.sh`로 깐 서버(실행 파일이 `/usr/local/bin`에 직접 있는 경우)에는 새 `install.sh`를 한 번 실행합니다. 기존 파일은 `/usr/local/lib/nabiman/legacy-<시각>/`으로 옮겨 하나의 버전으로 남기므로 `--rollback`으로 돌아갈 수 있습니다.

데이터 형식이 바뀌는 버전으로 올리기 전에는 [데이터 백업](../operations/data-and-backup.md)을 먼저 합니다. 되돌리기는 프로그램만 바꾸고 데이터는 되돌리지 않습니다.

## 개발 서버의 지난 배포 정리

이 개발 서버(`/workspace/nabiman`)는 `deployments/<이름>/`과 `bin/*.bak.*`, `static.bak.*`에 배포마다 사본을 남겨 왔습니다. `scripts/prune-deployments.sh`가 종류별로 최근 N개만 남기고 나머지를 보여 줍니다.

```sh
scripts/prune-deployments.sh --keep 5 --dry-run   # 목록과 크기만(기본 동작)
scripts/prune-deployments.sh --keep 5 --apply     # 실제 삭제: 서버 관리자의 승인 후에만
```

`static` 링크가 가리키는 배포와 그 `static.previous`(한 단계 되돌리기), 실행 중인 nabiman 프로세스가 쓰는 파일, `bin/`의 `.bak.`이 아닌 파일은 개수와 관계없이 지우지 않습니다. 컷오버 전 인증·데이터 사본(`cutover.auth`, `data.before`)을 담은 배포는 목록에 따로 표시합니다.
