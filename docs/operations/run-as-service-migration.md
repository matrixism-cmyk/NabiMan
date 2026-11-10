# man.bebo.kr을 root 모드에서 service 모드로 옮기기 (supervisor)

상태: **계획만 있습니다. 아래 명령은 아직 하나도 실행하지 않았습니다.** 실행 전에 nabiman(총괄)의
승인을 받고, 사용자 확인이 필요한지는 nabiman이 정합니다. 무엇이 root를 필요로 하는지와 service 모드가
각각을 어떻게 하는지는 [권한 목록](../security/privilege-inventory.md)에 있습니다.

이 서버는 systemd가 아니라 supervisor(PID 1, root)로 돕니다. 그래서 `packaging/nabiman-service-mode.service`의
샌드박스(ProtectSystem 등)는 여기서 쓸 수 없고, `setpriv`로 권한을 내리는 실행 스크립트로 대신합니다.

## 0. 지금 상태 (2026-11-11 읽기만 해서 확인)

| 항목 | 지금 |
|---|---|
| supervisor | `supervisord -n -c /workspace/etc/supervisor/supervisord.conf` (root, PID 1). 이 파일은 **nabi 소유 0600** |
| nabiman 프로그램 | `[program:nabiman] command=/workspace/nabiman/bin/start-nabiman.sh`, `user=root` |
| 시작 스크립트 | `/workspace/nabiman/bin/start-nabiman.sh` **nabi 소유 0750** — root가 실행 |
| 바이너리 | `bin/nabiman-server` root 0755지만 `bin/`은 **nabi 소유 0775**(nabi가 바꿔치기 가능). `bin/nabiman-privileged-helper`는 nabi 소유(10-06 빌드) |
| 상위 디렉터리 | `/workspace`, `/workspace/nabiman`, `/workspace/var/lib` 모두 nabi 소유 → nabi가 하위 트리를 통째로 rename 가능 |
| 데이터 | `/workspace/var/lib/nabiman` **nabi:nabi 0775**, 파일은 root·nabi 섞여 있음(nabi 소유 0600 파일 17개) |
| 비밀 | `/workspace/nabiman/secrets` nabi 0775 (`admin_password`) |
| 듣는 주소 | `0.0.0.0:18080`. nginx `man.bebo.kr` → `127.0.0.1:18080`. 18080 연결 4개 모두 127.0.0.1, 18080을 쓰는 k8s Service·LB·스크립트 없음 |
| 부팅 | 이미지 entrypoint가 매 부팅 `chown -R nabi:nabi /workspace` 후 supervisord 실행. `[program:00-persist-links]`(root, priority 1)가 `/workspace/bin/persist-links.sh`로 `/etc` 복원. `/`는 휘발성 |
| nabi | `sudo -n true` 성공 = **암호 없는 sudo** |

## 1. 지금 바로 할 수 있는 저위험 조치: root가 실행하는 파일을 nabi가 못 고치게

**왜:** root(supervisor)가 nabi가 고칠 수 있는 파일을 실행합니다. nabi 권한만 얻으면(nabi로 도는 supervisor
프로그램 38개 — 인터넷에 열린 웹 서비스 포함 — 중 하나가 뚫리면) 다음 재시작 때 root가 됩니다.

**먼저 알아둘 한계 (중요):**

1. nabi는 지금 **암호 없는 sudo**가 있습니다. 그래서 이 조치만으로는 nabi→root 경계가 생기지 않습니다.
   효과가 나려면 nabi의 sudo를 암호 필요로 바꾸거나(사람이 쓰는 계정), nabi로 도는 서비스들을 각자 계정으로
   옮겨야 합니다. 그 결정은 이 문서 범위 밖이며 nabiman/사용자 판단입니다.
2. `/workspace`와 `/workspace/nabiman`이 nabi 소유라 root 소유 `bin/`도 통째로 rename될 수 있습니다.
   그래서 상위 디렉터리에 sticky bit를 겁니다(1-2).
3. supervisor 설정 파일 자체가 nabi 소유입니다. nabi가 `user=root` 프로그램을 추가할 수 있으므로 같은 종류의
   구멍입니다(1-5, 영향이 커서 따로 승인).
4. 부팅 때 entrypoint가 `/workspace` 전체를 nabi로 chown 하므로, 매 부팅 `persist-links.sh`가 다시 걸어야
   유지됩니다(1-6). 그런데 `persist-links.sh`도 nabi가 고칠 수 있는 파일이라 경계가 아니라 "복원"일 뿐입니다.
   진짜 경계는 이미지 entrypoint를 바꿔야(전체 chown 대신 root 경로를 보호) 생깁니다.

이 조치들은 소유권만 바꾸며 **재시작이 필요 없고** 실행 중인 NabiMan에 영향이 없습니다.

```bash
# 1-1. root가 실행하는 것: bin/ 전체를 root 소유, 그룹·다른 사용자 쓰기 금지
sudo chown -R root:root /workspace/nabiman/bin
sudo chmod 0755 /workspace/nabiman/bin
sudo chmod -R go-w /workspace/nabiman/bin
sudo chmod 0750 /workspace/nabiman/bin/start-nabiman.sh      # root:root, nabi는 읽지도 못함

# 1-2. 상위 디렉터리: nabi는 계속 새 항목을 만들고 자기 것은 옮길 수 있지만,
#      root 소유 항목(bin/, static 링크)은 rename·삭제 못 하게 (sticky)
sudo chown root:nabi /workspace/nabiman
sudo chmod 1775 /workspace/nabiman

# 1-3. 데이터와 비밀: root만 (지금은 nabi가 users.json 등을 바꿔 관리자 계정을 가로챌 수 있음)
sudo chown -R root:root /workspace/var/lib/nabiman /workspace/nabiman/secrets
sudo chmod 0700 /workspace/var/lib/nabiman /workspace/nabiman/secrets
sudo chown root:nabi /workspace/var/lib && sudo chmod 1775 /workspace/var/lib

# 1-4. 확인 (nabi로): 모두 "denied"가 나와야 함
test -w /workspace/nabiman/bin/start-nabiman.sh && echo WRITABLE || echo denied
test -w /workspace/nabiman/bin && echo WRITABLE || echo denied
mv /workspace/nabiman/bin /workspace/nabiman/bin.x 2>/dev/null && echo MOVED || echo denied
test -r /workspace/var/lib/nabiman/users.json && echo READABLE || echo denied
```

1-5 (영향 큼, 별도 승인): supervisor 설정을 root 것으로. 이후 다른 서비스 설정을 고칠 때마다 `sudo`가 필요합니다.

```bash
sudo chown root:root /workspace/etc/supervisor /workspace/etc/supervisor/supervisord.conf
sudo chmod 0755 /workspace/etc/supervisor && sudo chmod 0600 /workspace/etc/supervisor/supervisord.conf
sudo chown root:nabi /workspace/etc && sudo chmod 1775 /workspace/etc
sudo chown root:nabi /workspace && sudo chmod 1775 /workspace     # /workspace 자체 (가장 넓은 영향)
```

1-6. 부팅마다 다시 걸기: `persist-links.sh` 끝(00-persist-links, entrypoint의 chown 뒤)에 위 1-1~1-3(승인되면 1-5)
명령을 그대로 넣습니다. `sudo -e /workspace/bin/persist-links.sh`.

**배포 절차가 바뀌는 점:** 1-1 뒤에는 nabi가 `bin/`에 직접 복사할 수 없습니다. 새 바이너리는
`sudo install -o root -g root -m 0755 <빌드 결과> /workspace/nabiman/bin/nabiman-server.new && sudo mv -f /workspace/nabiman/bin/nabiman-server.new /workspace/nabiman/bin/nabiman-server`
처럼 sudo로 넣습니다. 1-3 뒤에는 데이터 디렉터리를 보려면 `sudo`가 필요합니다. (배포 메모 `nabiman-deploy`도 고칩니다.)

**되돌리기:** `sudo chown -R nabi:nabi /workspace/nabiman/bin /workspace/var/lib/nabiman /workspace/nabiman/secrets && sudo chown nabi:nabi /workspace/nabiman /workspace/var/lib && sudo chmod 0775 /workspace/nabiman /workspace/var/lib` (1-5를 했으면 그 경로들도 `nabi:nabi`, `0755`).

## 2. 이 서버에서 service 모드로 바뀌는 기능 (먼저 결정할 것)

| 기능 | 지금(root) | service 모드 |
|---|---|---|
| 로컬 터미널 | root 셸 | `nabiman` 셸. `sudo`/`su` 안 됨 |
| **Claude 세션을 nabi로 시작**(`su --pty -l nabi`, 지휘 체계의 기본) | 됨 | **안 됨**(503/거절 메시지). 도우미에 "계정으로 세션 시작" 기능이 생기기 전까지 이 서버의 핵심 흐름이 멈춤 |
| 기존 tmux 세션 | root의 tmux 서버 | 이어지지 않음(다른 uid). 옛 세션은 root tmux에 남음 |
| OS 계정 관리(useradd 래퍼) | 됨 | 안 됨(503). 이 서버의 계정 관리가 NabiMan을 씀 |
| 방화벽, 패키지 업데이트 | 직접 | 도우미. 단 pod 안이라 방화벽 도구가 없거나 막혀 있을 수 있고, 패키지는 `/`가 휘발성이라 재부팅 때 사라짐 |
| 서비스 제어 | root라 거절(503) | 도우미 → systemctl. 이 서버엔 systemd가 없어 실패로 기록됨 |
| Office/한글 미리보기 | nobody로 실행 | 안 됨(503) |
| 사이트·인증서·설정 파일·cron·DB·IP 차단·세션 종료 | 됨 | 안 됨(503) |
| 시리얼 브리지(`NABIMAN_SERIAL_GROUP=nabi`) | 됨 | `nabiman`을 `nabi` 그룹에 넣으면 안 됨(그룹 쓰기 가능한 `/workspace`를 고칠 수 있게 됨). 별도 `serial` 그룹을 만들거나 끔 |
| RDP 드라이브(`<data>/guac-drive`) | guacd가 읽음 | 데이터가 nabiman 0700이라 guacd가 못 읽음 → guacd도 nabiman으로 돌리거나 끔 |
| 로그 보기(journalctl) | 됨 | 이 pod엔 journald가 없어 원래 제한적 |

**권고:** "Claude 세션을 계정으로 시작" 도우미 기능(목록 #9)이 생기기 전에는 이 서버를 전환하지 않습니다.
1장(소유권)과 `listen` 기본값(127.0.0.1)은 전환과 무관하게 먼저 적용할 수 있습니다.

## 3. 전환 절차 (승인 후, 위 조건이 갖춰졌을 때)

### 3-1. 계정 (고정 UID, 매 부팅 복원)

이 서버의 계정 저장소(`/workspace/etc/accounts`)는 UID 1001 이상만 복원합니다. 시스템 계정은 `persist-links.sh`에
따로 넣습니다. UID를 고정해야 데이터 소유권과 도우미 정책의 `api_uid`가 재부팅 뒤에도 맞습니다.

```bash
getent passwd 990 || echo "UID 990 free"           # 비어 있는 UID 확인
sudo groupadd --system --gid 990 nabiman
sudo useradd --system --uid 990 --gid 990 --home-dir /workspace/var/lib/nabiman --no-create-home --shell /usr/sbin/nologin nabiman
# persist-links.sh 에 추가 (매 부팅):
#   getent group nabiman >/dev/null || groupadd --system --gid 990 nabiman
#   getent passwd nabiman >/dev/null || useradd --system --uid 990 --gid 990 --home-dir /workspace/var/lib/nabiman --no-create-home --shell /usr/sbin/nologin nabiman
```

### 3-2. 데이터와 실행 디렉터리

```bash
sudo supervisorctl stop nabiman                     # 여기서부터 중단 시간
sudo cp -a /workspace/var/lib/nabiman /workspace/var/lib/nabiman.pre-service-$(date +%Y%m%d-%H%M%S)   # 되돌리기용
sudo chown -R nabiman:nabiman /workspace/var/lib/nabiman
sudo chmod 0700 /workspace/var/lib/nabiman
sudo install -d -o nabiman -g nabiman -m 0755 /run/nabiman            # claude.sock, tmux
sudo install -d -o root -g root -m 0755 /run/nabiman-helper           # 도우미 소켓 (root 디렉터리)
sudo install -d -o root -g root -m 0700 /var/lib/nabiman-helper       # 원장·감사 (휘발성 / 에 있음: 재부팅 때 새로 시작)
sudo install -d -o root -g root -m 0755 /etc/nabiman
```

데이터 디렉터리는 entrypoint가 매 부팅 nabi로 chown 하므로 `persist-links.sh`에 `chown -R nabiman:nabiman /workspace/var/lib/nabiman; chmod 0700 /workspace/var/lib/nabiman`을 넣습니다.
도우미 원장은 상위 디렉터리가 모두 root 소유여야 해서(nabi 소유 `/workspace` 아래는 거부) 휘발성 `/var/lib/nabiman-helper`에 둡니다. 재부팅하면 서비스 제어 기록이 사라집니다.

### 3-3. 도우미 정책과 바이너리 (root 소유 복사본)

```bash
sudo install -o root -g root -m 0755 /workspace/nabiman/bin/nabiman-privileged-helper /usr/local/sbin/nabiman-privileged-helper
printf '{\n  "api_uid": 990,\n  "services": []\n}\n' | sudo install -o root -g root -m 0600 /dev/stdin /etc/nabiman/privileged-services.json
printf '{\n  "api_uid": 990,\n  "firewall": false,\n  "package_upgrade": false\n}\n' | sudo install -o root -g root -m 0600 /dev/stdin /etc/nabiman/privileged-system.json
```

이 pod에서는 방화벽·패키지가 의미가 적어 둘 다 `false`로 시작합니다. `/etc`·`/usr/local/sbin`은 휘발성이라
같은 복사를 `persist-links.sh`에도 넣습니다(원본이 nabi가 고칠 수 있는 곳에 있다는 한계는 1장과 같음).

### 3-4. supervisor 프로그램

API는 root 실행 스크립트가 `setpriv`로 권한을 내려 시작하고, 시작된 PID를 도우미에 알립니다(pin).
supervisor의 `user=nabiman`만 쓰면 pin을 할 root 단계가 없어서 도우미가 모든 요청을 거절합니다(기능은
"도우미 없음"으로 표시될 뿐 API는 동작).

`/usr/local/sbin/start-nabiman-service.sh` (root:root 0755, `persist-links.sh`가 매 부팅 설치):

```bash
#!/bin/bash
set -euo pipefail
PW_FILE=/workspace/nabiman/secrets/admin_password
NABIMAN_PASSWORD=$(cat "$PW_FILE"); export NABIMAN_PASSWORD
export NABIMAN_RUN_MODE=service
export NABIMAN_PORT=18080
export NABIMAN_LISTEN=127.0.0.1:18080
export NABIMAN_STATIC=/workspace/nabiman/static
export NABIMAN_DATA_DIR=/workspace/var/lib/nabiman
export NABIMAN_HELPER_SOCKET=/run/nabiman-helper/control.sock
export TMUX_TMPDIR=/run/nabiman
export HOME=/workspace/var/lib/nabiman
unset NABIMAN_SERIAL_GROUP                         # 2장: nabi 그룹을 주지 않음
export PATH="/workspace/opt/tmux-3.8/bin:/usr/local/bin:/usr/bin:/bin:/workspace/opt/bin"
# root 단계 (예전 start-nabiman.sh 에서 옮김)
SSH_COMPAT_CONFIG=/workspace/nabiman/data/ssh_config.d/syc.conf
if [[ -r "$SSH_COMPAT_CONFIG" ]]; then
    mkdir -p /etc/ssh/ssh_config.d
    install -o root -g root -m 0644 "$SSH_COMPAT_CONFIG" /etc/ssh/ssh_config.d/nabiman-syc.conf
fi
install -d -o nabiman -g nabiman -m 0755 /run/nabiman
# 권한을 내려 시작: uid/gid 990, 보조 그룹 없음, 새 권한 금지, capability 없음
setpriv --reuid=nabiman --regid=nabiman --clear-groups --no-new-privs \
        --inh-caps=-all --bounding-set=-all -- /workspace/nabiman/bin/nabiman-server &
API=$!
trap 'kill -TERM "$API" 2>/dev/null' TERM INT
# exec 가 끝나 uid 가 nabiman 이 된 뒤에 pin (몇 번 시도)
for _ in $(seq 1 20); do
    /usr/local/sbin/nabiman-privileged-helper --pin-api "$API" && break
    sleep 0.5
done
unset NABIMAN_PASSWORD
wait "$API"
```

`supervisord.conf` 변경 (`sudo -e`):

```ini
[program:nabiman-helper]
command=/usr/local/sbin/nabiman-privileged-helper
user=root
priority=900
autostart=true
autorestart=true
startsecs=2
stdout_logfile=/workspace/var/log/nabiman-helper.log
stderr_logfile=/workspace/var/log/nabiman-helper.err.log

[program:nabiman]
command=/usr/local/sbin/start-nabiman-service.sh
directory=/workspace/var/lib/nabiman
user=root                 ; 실행 스크립트가 setpriv 로 nabiman 이 됨 (pin 때문에 root 로 시작)
autostart=true
autorestart=true
startsecs=5
stopwaitsecs=10
stopasgroup=false         ; tmux 서버(터미널)는 API 재시작에도 남김
stdout_logfile=/workspace/var/log/nabiman.log
stderr_logfile=/workspace/var/log/nabiman.err.log
```

```bash
sudo supervisorctl reread && sudo supervisorctl update
sudo supervisorctl start nabiman-helper nabiman
```

### 3-5. 확인

```bash
ps -o user,pid,args -C nabiman-server             # user = nabiman
ss -ltn | grep 18080                               # 127.0.0.1:18080 만
grep -E "Run mode|WARNING|Settings" /workspace/var/log/nabiman*.log | tail
curl -s -o /dev/null -w '%{http_code}\n' https://man.bebo.kr/            # 200
# 관리자로 로그인 → GET /api/diagnostics/runtime 의 privileges: run_mode "service", 기능별 상태
# 503 확인: POST /api/accounts → "not available ... run_mode = service"
sudo tail -n 5 /var/lib/nabiman-helper/audit.jsonl   # 도우미 호출 기록
```

## 4. 되돌리기 (root 모드로)

```bash
sudo supervisorctl stop nabiman nabiman-helper
# supervisord.conf: [program:nabiman] 을 원래대로 (command=/workspace/nabiman/bin/start-nabiman.sh, user=root),
#                   [program:nabiman-helper] 삭제 또는 autostart=false
sudo supervisorctl reread && sudo supervisorctl update
# 데이터: root 는 nabiman 소유 파일도 읽으므로 그대로 시작됨. 정리하려면:
sudo chown -R root:root /workspace/var/lib/nabiman && sudo chmod 0700 /workspace/var/lib/nabiman
sudo supervisorctl start nabiman
# 전환 중 데이터가 망가졌다면 3-2 의 /workspace/var/lib/nabiman.pre-service-* 로 되돌림 (그 뒤의 변경은 잃음)
```

root 모드로 돌아가면 nabiman으로 연 터미널 세션은 이어지지 않습니다(이번엔 반대 방향). persist-links.sh에 넣은
service 모드 줄도 빼야 다음 부팅에 다시 nabiman 소유로 바뀌지 않습니다.
