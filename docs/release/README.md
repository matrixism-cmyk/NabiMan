# 릴리스·설치·업데이트

이 문서는 나비맨을 고객 서버에 내보내는 전 과정을 다룹니다: 릴리스 만들기(태그 → CI → 서명된 패키지), 설치(명령 하나), 검증, 업데이트, 되돌리기.

- 패키지: `nabiman-linux-x86_64.tar.gz`, `nabiman-linux-aarch64.tar.gz`(Linux, glibc). 각각 `.minisig`(서명), `.sha256`, `.cdx.json`(SBOM), `.manifest.json`, `.inventory.json`, `.licenses.json`이 함께 붙습니다.
- 서명: [minisign](https://jedisct1.github.io/minisign/) (Ed25519). 공개 키는 저장소의 [`nabiman-release.pub`](nabiman-release.pub)와 `install.sh` 안에 같은 값으로 들어 있습니다.
- 무중단 배포 검토: [zero-downtime.md](zero-downtime.md).

## 고객 서버에 설치

```sh
curl -fsSLO https://github.com/matrixism-cmyk/NabiMan/releases/latest/download/install.sh
sudo bash install.sh --install-deps
```

`install.sh`는 반드시 **릴리스에 붙은 것**을 씁니다. 그 안에 서명 공개 키가 들어 있습니다(저장소 브랜치의 `install.sh`는 키가 들어가기 전이면 설치를 거부합니다). `curl … | sudo bash`도 동작하지만, 받은 뒤 아래처럼 `install.sh` 자체의 서명을 확인하고 실행하기를 권합니다.

설치 스크립트가 하는 일:

1. 실행에 필요한 패키지(tmux, ssh, ssh-keygen, sshpass, socat)를 확인합니다. 없으면 배포판에 맞는 설치 명령을 보여 주고 멈춥니다. `--install-deps`면 apt/dnf로 설치합니다(`--with-optional`: guacd, LibreOffice, ffmpeg까지).
2. 패키지와 `.minisig`를 받아 **스크립트에 들어 있는 키와 검증 코드로** 서명을 확인합니다. 서명이 없거나, 다른 키이거나, 다른 파일(예: 다른 아키텍처)에 대한 서명이면 설치하지 않습니다. 받은 파일의 코드는 서명이 확인된 뒤에만 실행합니다.
3. manifest로 모든 파일의 해시·크기·권한을 확인하고 새 디렉터리에 풉니다.
4. 처음 설치라면 관리자 초기 비밀번호(12자 이상)를 묻습니다(`NABIMAN_PASSWORD`로 줄 수도 있음).
5. 아래 [설치 위치](#설치-위치)에 놓고, systemd unit을 설치·enable합니다. 시작은 `sudo systemctl start nabiman`.

선택 사항:

| 옵션 | 뜻 |
|---|---|
| `--version v1.2.0` | 최신 대신 특정 릴리스 |
| `--base-url https://mirror.example/nabiman/v1.2.0` | 사내 미러에서 받기(서명은 그대로 확인) |
| `--archive FILE [--signature FILE]` | 인터넷이 없는 서버: 미리 옮겨 둔 패키지로 설치(기본 서명 파일은 `FILE.minisig`) |
| `--insecure-skip-verify` | 서명 확인 생략. 다른 방법으로 검증한 경우에만, 운영 서버에는 쓰지 않습니다 |
| `NABIMAN_REPO=owner/repo` | 다른 GitHub 저장소의 릴리스 |

## 받은 파일 직접 검증하기

minisign이 있으면:

```sh
minisign -Vm nabiman-linux-x86_64.tar.gz -p nabiman-release.pub
minisign -Vm install.sh -p nabiman-release.pub
```

minisign 없이(Python 3 표준 라이브러리만):

```sh
python3 verify-release.py nabiman-linux-x86_64.tar.gz \
  --public-key-file nabiman-release.pub --require-signature \
  --extract-to ./nabiman-verified
```

`nabiman-release.pub`은 저장소(`docs/release/`)나 이미 설치된 서버의 `/etc/nabiman/release.pub`처럼 **릴리스와 다른 곳**에서 가져옵니다. 같은 릴리스에서 받은 키로 확인하면 의미가 없습니다. 출력의 `publisher_authenticated: true`와 `signed_version`을 확인합니다. 서명의 trusted comment에는 `version:<태그> file:<파일 이름>`이 들어 있고, 검증기는 파일 이름이 맞는지까지 봅니다.

`verify-release.py`는 `cryptography` 패키지가 있으면 그것으로, 없으면 스크립트 안의 RFC 8032 구현으로 Ed25519를 확인합니다.

## 릴리스 만들기

1. `backend/Cargo.toml`과 `frontend/package.json`의 `version`을 새 버전으로 올립니다(같은 값).
2. `CHANGELOG.md`의 `Unreleased`를 그 버전 제목으로 옮깁니다.
3. 커밋하고 태그를 올립니다:
   ```sh
   git tag -a v1.2.0 -m "NabiMan 1.2.0"
   git push origin v1.2.0
   ```
4. `.github/workflows/release.yml`이 돕니다.
   - **build** (x86_64: `ubuntu-24.04`, aarch64: `ubuntu-24.04-arm`, 둘 다 네이티브): 서명 키 설정 확인, 태그와 패키지 버전 일치 확인, Docker 이미지 digest 고정 확인, 프런트 audit·타입·린트·테스트·빌드, Rust 1.97.1로 `cargo test/build --locked`(빌드 커밋은 태그의 커밋), 라이선스 검사, 패키징, 패키지 서버를 띄우는 smoke test, SBOM 스키마 검증. 이 job에는 비밀이 없고 쓰기 권한도 없습니다.
   - **publish** (`release` environment): 두 아키텍처의 산출물을 모아 minisign으로 하나씩 서명하고, 저장소의 공개 키로 모두 다시 확인한 다음 GitHub 릴리스에 붙입니다(`install.sh`와 그 서명 포함).
5. 릴리스 페이지에서 `.minisig`가 모든 파일에 붙었는지 보고, 위의 검증을 한 번 해 봅니다.

태그가 패키지 버전과 다르거나, 서명 키가 아직 placeholder이거나, 출하되는 의존성의 라이선스가 정책 밖이면 릴리스는 실패합니다.

### aarch64 빌드

GitHub 호스팅 arm64 러너(`ubuntu-24.04-arm`)로 네이티브 빌드해서, x86_64와 똑같이 패키지 서버 smoke test까지 돌립니다. 이 러너는 공개 저장소에서는 무료이고, **비공개 저장소에서는 arm64 larger runner가 되는 요금제**가 필요합니다. 쓸 수 없으면 matrix의 aarch64 항목을 x86_64 러너 + [`cross`](https://github.com/cross-rs/cross)(`cross build --release --locked --target aarch64-unknown-linux-gnu`)로 바꾸고 그 항목의 smoke test를 뺍니다(교차 빌드한 실행 파일은 x86_64 러너에서 실행할 수 없습니다).

### 서명 키 만들기와 GitHub 설정 (처음 한 번)

개인 키는 저장소에 절대 넣지 않습니다. 오프라인 컴퓨터에서 만들고 사본을 안전한 곳(금고, 비밀번호 관리자)에 둡니다.

```sh
minisign -G -p nabiman-release.pub -s nabiman-release.key   # 암호를 묻습니다
# CI에서 암호 없이 쓰려면: minisign -G -W -p … -s …
```

1. `nabiman-release.pub`의 두 줄로 `docs/release/nabiman-release.pub`을 바꾸고, 둘째 줄(키)을 `install.sh`의 `NABIMAN_RELEASE_PUBKEY='…'`에 넣습니다. `python3 scripts/check-release-key.py`가 통과해야 합니다.
2. GitHub → Settings → Environments → **release**를 만들고 Deployment branches and tags를 `v*` 태그로 제한, Required reviewers를 지정합니다.
3. 그 environment의 secret으로 `MINISIGN_SECRET_KEY`(= `nabiman-release.key` 파일 내용 전체)와, 암호를 걸었다면 `MINISIGN_PASSWORD`를 넣습니다.
4. Settings → Rules에서 `v*` 태그를 보호합니다(만들기·지우기를 관리자만).

키를 바꾸면(분실·유출) 새 키로 위 1~3을 다시 하고, 이미 설치된 서버의 `/etc/nabiman/release.pub`도 새 키로 바꿔야 `nabiman-update`가 새 릴리스를 받아들입니다. 유출이면 예전 키로 서명된 릴리스를 내리고 고객에게 알립니다.

### 라이선스

패키징 때 `scripts/release-inventory.py licenses`가 Cargo(`cargo metadata --locked`)와 npm(`package-lock.json`, `node_modules`) 의존성의 SPDX 라이선스를 모아 `packaging/licenses.json` 정책과 비교합니다. 패키지에는 `LICENSE`, `NOTICE`(의존성의 NOTICE 포함), `THIRD_PARTY_LICENSES.md`(목록과 라이선스 원문), `licenses.json`이 들어갑니다. GPL/LGPL 같은 항목은 패키지·버전별로 `reviewed`에 검토 사유를 적어야 통과하고, 모르는 라이선스는 실패합니다. 로컬에서 보려면:

```sh
python3 scripts/release-inventory.py licenses --source . --out /tmp/nabiman-licenses --report-only
```

### 컨테이너 이미지

```sh
docker build --build-arg NABIMAN_GIT_COMMIT=$(git rev-parse HEAD) -t nabiman .
```

기반 이미지는 digest로 고정되어 있습니다(`Dockerfile` 첫머리에 갱신 방법). CI는 digest 없는 `FROM`을 실패로 봅니다(`scripts/check-docker-pins.sh`). 이미지 안의 경로도 아래 설치 위치와 같습니다.

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

API를 root가 아닌 사용자로 옮기는 일은 Phase 4입니다([서비스 제어 경계](../commercialization/privileged-service-control.md), `packaging/nabiman-service-mode.service`, `install.sh --mode service`). 그 단위는 전체 샌드박스를 겁니다.

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
