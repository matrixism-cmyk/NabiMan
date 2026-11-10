# 데이터 디렉터리와 백업·복원

나비맨이 저장하는 상태는 모두 **데이터 디렉터리** 한 곳에 있습니다. 이 문서는 그 안에 무엇이 있는지, 서버가 시작할 때 데이터 디렉터리를 어떻게 다루는지, 백업과 복원을 어떻게 하는지, 버전을 올리거나 내릴 때 무엇을 지켜야 하는지 정리합니다.

## 위치

- 데이터 디렉터리: `NABIMAN_DATA_DIR` 환경 변수입니다. 지정하지 않으면 `/var/lib/nabiman`을 씁니다. 패키지(`nabiman.service`)와 컨테이너 이미지(`VOLUME /var/lib/nabiman`)도 이 경로를 씁니다.
- MEC 데이터베이스: `NABIMAN_MEC_DB`를 지정하면 그 파일을 씁니다. 지정하지 않으면 이제 `데이터 디렉터리/mec.db`를 씁니다. 예전에는 서버를 시작한 작업 디렉터리 기준 `./data/mec.db`에 있었습니다. 그래서 데이터 디렉터리 밖에 있었고 백업과 권한 점검에서도 빠졌습니다. 컨테이너에서는 볼륨 밖에 있었습니다. 옮기는 과정은 아래 「시작할 때 일어나는 일」에 있습니다.

## 데이터 디렉터리에 있는 것

| 이름 | 내용 | 형식 | 백업 |
|---|---|---|---|
| `users.json` | 회원: bcrypt 비밀번호 해시, 역할, TOTP 비밀, 소속, 접근 규칙 | JSON | 포함 |
| `password` | 첫 기동 때 쓰는 레거시 관리자 비밀번호 해시 | 텍스트 | 포함 |
| `jwt_secret` | 로그인 토큰 서명 키. 이 키가 같으면 복원 뒤에도 기존 로그인이 유지됩니다 | 텍스트 | 포함 |
| `login_sessions.json` | 로그인 세션(갱신 토큰 상태) | JSON | 포함 |
| `api_keys.json` | 공개 API 키(해시만 저장) | JSON | 포함 |
| `remote_servers.json` | 원격 서버 목록. 저장한 비밀번호는 `secret.key`로 암호화한 `v1:` 값입니다 | JSON | 포함 |
| `secret.key` | 서버 비밀번호와 감사 전송 인증 정보를 암호화하는 AES-256-GCM 키 | 16진 텍스트 | **기본 제외**(아래 참고) |
| `scope-settings.json`, `app_settings.json`, `ldap_config.json`, `oauth_config.json` | 권한 범위 정책, 앱 설정, LDAP·OAuth 설정 | JSON | 포함 |
| `license.json`, `install_timestamp` | 라이선스, 체험 기간 시작 시각 | JSON, 텍스트 | 포함 |
| `notifications.json`, `notification_templates.json`, `notification_history.json`, `alert_rules.json` | 알림 채널, 템플릿, 발송 기록, 경보 규칙 | JSON | 포함 |
| `backup_schedules.json` | 백업 예약 | JSON | 포함 |
| `audit.jsonl`, `audit.jsonl.1`~`.5` | 감사 기록(해시 체인)과 회전 파일 | JSON Lines | 포함 |
| `audit-forwarding.sqlite3` | 감사 기록 외부 전송 대상과 대기열 | SQLite | 포함 |
| `terminal_sessions.json`, `terminal_settings.json`, `terminal_shares.json`, `terminal_session_settings.json`, `session_order.json`, `sftp_columns.json` | 터미널 세션 목록, 사용자별 터미널 설정, 공유, 세션별 설정, 정렬, SFTP 열 배치 | JSON | 포함 |
| `console_shares.json`, `vnc_consoles.json`, `vnc_access.json` | 콘솔 공유, VM 콘솔 목록과 접근 권한 | JSON | 포함 |
| `recording.json`, `dictation.json`, `claude-voice.json`, `claude-voice/`, `claude-lead` | 녹화 설정, 받아쓰기 설정, Claude 음성 설정과 대화, 총괄 세션 표시 | JSON, 텍스트 | 포함 |
| `topology.sqlite3` | 작업 지도의 네트워크 구성(장비·연결·서버 위치) | SQLite | 포함 |
| `claude-events.sqlite3` | Claude 세션끼리 주고받은 메시지 기록(14일) | SQLite(WAL) | 포함 |
| `server-facts.sqlite3` | 서버를 마지막으로 점검했을 때 확인한 사양·상태 | SQLite | 포함 |
| `usage.sqlite3` | 사용량 원장: 1분마다 센 세션·계정·서버 수의 날짜별 합(800일). `/api/usage`가 읽음 | SQLite | 포함 |
| `mec.db` | MEC 테넌트·작업·감사·문서 | SQLite | 포함 |
| `ssh/` | 나비맨 SSH 키(`nabiman_ed25519`). `ssh/pw/`는 sshpass에 넘기는 임시 파일입니다 | 파일 | 포함(`ssh/pw/` 제외) |
| `ssh-ca/` | SSH 인증서 로그인용 나비맨 CA 키 | 파일 | 포함 |
| `ssh-host-trust/` | 서버별로 승인한 호스트 키 | 파일 | 포함 |
| `.lock`, `*.lock` | 데이터 디렉터리 잠금, 저장소별 잠금 | 빈 파일·PID | 제외 |
| `tmux.conf` | 터미널용 tmux 설정. 시작할 때마다 다시 씁니다 | 텍스트 | 제외 |
| `backups/` | 백업 파일 보관 위치 | 아카이브 | 제외 |
| `preview-cache/` | 오피스 문서 미리보기 캐시 | PDF | 제외 |
| `session-recordings/` | 세션 녹화(선택 기능). 용량이 커서 필요하면 따로 백업합니다 | 파일 | 제외 |
| `serial/` | 브라우저에서 연결한 시리얼 장치의 PTY 링크 | 링크 | 제외 |
| `guac-drive/` | RDP 파일 전송 드라이브(사용자 파일) | 파일 | 제외 |
| `db_backups/` | 데이터베이스 메뉴에서 만든 다른 DB의 덤프 | 파일 | 제외 |

이름이 `.`으로 시작하는 파일(잠금, 원자적 저장 중의 임시 파일, 복원 작업 디렉터리)과 SQLite의 `-wal`·`-shm`·`-journal` 파일도 백업하지 않습니다. 데이터베이스는 아래 방식으로 통째로 복사하므로 이 파일들이 필요 없습니다.

## 저장 방식

- JSON 상태 파일은 모두 원자적으로 교체합니다. 같은 디렉터리의 임시 파일에 쓰고 fsync한 뒤 rename합니다. 쓰는 도중 서버가 죽거나 디스크가 차면 이전 파일이 그대로 남습니다. 반쯤 쓴 파일은 생기지 않습니다. 감사 기록만 append + fsync로 덧붙입니다.
- 웹 서버 사이트 설정(`/etc/nginx/sites-available/…`, `/etc/apache2/sites-available/…`)도 같은 방식으로 씁니다. 원래 권한은 유지하고 새 파일은 0644로 만듭니다.

## 시작할 때 일어나는 일

서버는 다른 저장소를 열기 전에 다음을 순서대로 합니다.

1. **잠금**: `데이터 디렉터리/.lock`에 배타적 advisory lock(flock)을 걸고 자기 PID를 적습니다. 다른 서버가 같은 디렉터리를 이미 쓰고 있으면 그 PID를 알려 주고 바로 종료합니다.
   ```
   NabiMan data directory /var/lib/nabiman is already in use by another server (PID 1234). Stop that server first, or give this one its own NABIMAN_DATA_DIR.
   ```
   잠금은 프로세스가 끝나면 커널이 풉니다. 비정상 종료해도 마찬가지입니다. 그래서 `.lock` 파일이 남아 있어도 지울 필요가 없습니다. 이 파일 디스크립터는 close-on-exec이므로 tmux 같은 자식 프로세스가 잠금을 물려받지 않습니다. 같은 디렉터리를 쓰는 서버가 둘이면 서로의 변경을 덮어씁니다. 잠금은 이것을 막습니다. 서버를 여러 대 두려면 서버마다 데이터 디렉터리를 따로 주어야 합니다.
2. **MEC 데이터베이스 이전**: `NABIMAN_MEC_DB`가 없고 `데이터 디렉터리/mec.db`도 없는데 예전 위치(작업 디렉터리 기준 `./data/mec.db`)에 파일이 있으면 한 번 옮깁니다. 먼저 예전 파일의 소유 잠금이 비어 있는지 확인합니다. 잠금이 잡혀 있으면 예전 서버가 아직 도는 것이므로 시작하지 않습니다. 다음으로 WAL을 본 파일에 합칩니다. 같은 파일 시스템이면 rename으로 옮깁니다. 다른 파일 시스템이면 `VACUUM INTO`로 일관되게 복사하고 예전 파일 이름을 `mec.db.moved-to-data-dir`로 바꿉니다. 새 위치에 이미 파일이 있으면 그 파일을 쓰고, 예전 파일은 읽지 않는다고 로그에 남깁니다. 어느 경우든 무엇을 했는지 로그에 남습니다.
3. **권한 점검**: 데이터 디렉터리 안의 파일과 하위 디렉터리에서 그룹·기타 사용자 권한을 없앱니다(파일 0600, 디렉터리 0700에 해당). 데이터 디렉터리 자체는 0711까지 허용합니다. 목록을 보거나 쓰는 권한만 막고 경로 통과 권한은 남겨 둡니다. 다른 계정과 일부러 공유하는 `serial/`(NABIMAN_SERIAL_GROUP)과 `guac-drive/`(guacd)가 이 디렉터리 아래에 있기 때문입니다. 이 두 디렉터리는 건드리지 않습니다. 링크·소켓·공개 키(`*.pub`)도 건드리지 않습니다. 서버 프로세스가 소유한 파일만 고칩니다. 다른 계정 소유 파일은 권한과 소유자를 경고로 알리기만 합니다. 고치려면 `chmod go-rwx 파일`을, 데이터 디렉터리 자체는 `chmod go-rw`를 실행합니다.
4. **MEC 데이터베이스 열기**와 스키마 마이그레이션(아래).
5. **새 설치 기본값**: 회원 파일도 예약 파일도 없는 새 데이터 디렉터리이면 기본 백업 예약을 만듭니다(아래).

## SQLite 스키마 버전과 마이그레이션

데이터베이스마다 마이그레이션 단계 목록이 있고, 적용한 단계 수를 `PRAGMA user_version`에 기록합니다. 데이터베이스를 열 때 빠진 단계를 한 트랜잭션(`BEGIN IMMEDIATE`)으로 적용합니다. 모두 적용되거나 하나도 적용되지 않습니다.

| 데이터베이스 | 현재 버전 |
|---|---|
| `mec.db` | 1 |
| `topology.sqlite3` | 1 |
| `claude-events.sqlite3` | 1 |
| `server-facts.sqlite3` | 1 |
| `audit-forwarding.sqlite3` | 1(예전 `forwarding_schema` 행도 그대로 유지) |

버전 1은 버전 관리를 도입하기 전의 스키마와 똑같습니다(`CREATE … IF NOT EXISTS`). 그래서 기존 데이터베이스(버전 0, 테이블과 데이터 있음)는 데이터 변경 없이 버전 1이 됩니다. 이후 스키마를 바꿀 때는 새 단계를 목록 끝에 추가합니다. 이미 배포한 단계는 고치지 않습니다.

코드가 아는 버전보다 높은 데이터베이스는 열지 않습니다. 파일은 바꾸지 않고 다음과 같이 알립니다.

```
mec.db has schema version 2, but this NabiMan knows only up to 1: it was written by a newer NabiMan. …
```

권한 서비스 도우미의 원장(`/var/lib/nabiman-helper/operations.db`)은 별도 서비스가 원래부터 `user_version`으로 관리하므로 대상이 아닙니다.

## 백업

### 무엇이 들어가나

백업은 데이터 디렉터리 스냅숏 하나(`nabiman-state-<UTC 시각>.tar.gz`, 0600)입니다. 위 표에서 「포함」인 항목이 모두 들어갑니다.

- JSON 등 상태 파일은 그대로 복사합니다. 모든 저장소가 원자적으로 교체하므로 복사본은 항상 파일의 온전한 한 버전입니다.
- SQLite 데이터베이스는 `VACUUM INTO`로 복사합니다. 실행 중인 WAL 데이터베이스도 한 트랜잭션 시점으로 일관되게 담깁니다. 파일을 바이트 단위로 그대로 복사하지 않습니다.
- `NABIMAN_MEC_DB`로 MEC 데이터베이스를 데이터 디렉터리 밖에 두었으면 그 파일도 `mec.db`로 넣습니다.
- `nabiman-state.json` manifest에 파일별 SHA-256·크기, 데이터베이스별 스키마 버전, 제외한 항목과 이유, `secret.key` 포함 여부를 기록합니다.

서버가 실행 중일 때도 스냅숏을 만들 수 있습니다.

### secret.key를 백업에 넣을까

**기본값은 넣지 않는 것입니다.** `secret.key`가 없는 백업에서는 서버 비밀번호와 감사 전송 인증 정보가 암호문으로만 남습니다. 백업 파일이 복사되거나 유출되어도 이 자격 증명은 드러나지 않습니다. 대신 키를 따로 보관해야 합니다.

- 설치 직후와 키를 바꾼 뒤에 `secret.key`를 백업과 다른 곳(오프라인 매체, 비밀 관리 도구)에 복사해 둡니다. 키는 처음 만든 뒤 바뀌지 않습니다.
- 키를 잃으면 저장한 서버 비밀번호와 전송 인증 정보를 복구할 수 없습니다. 이 경우 다시 입력해야 합니다. 나머지 상태는 키 없이도 복원됩니다.
- 키까지 한 파일에 담아야 하는 환경에서는 명시적으로 선택합니다. 예약은 `include_secret_key: true`, 명령은 `--with-secret-key`입니다. 그런 백업은 데이터 디렉터리 전체와 같은 수준으로 보호해야 합니다.

`users.json`(비밀번호 해시, TOTP 비밀), `jwt_secret`, SSH 키와 CA 키는 키를 넣지 않은 백업에도 평문으로 들어갑니다. 백업 파일은 0600이고 `backups/`는 0700입니다. 외부로 옮길 때는 암호화된 저장소를 쓰거나 아래 오프라인 복구 묶음(age 암호화)을 사용합니다.

### 자동 백업 예약

새 설치(회원 파일과 `backup_schedules.json`이 모두 없는 데이터 디렉터리)는 기본 예약 하나로 시작합니다.

- 이름: `NabiMan 데이터 자동 백업`, 종류 `state`
- 시각: 매일 03:17 UTC(`17 3 * * *`). 정각을 피해 다른 cron 작업과 겹치지 않게 했습니다.
- 보관: 이 예약이 만든 스냅숏 중 최근 7개. 백업이 성공하면 더 오래된 것을 지웁니다.
- `secret.key`: 넣지 않음
- 위치: `데이터 디렉터리/backups/`

설정:

| 환경 변수 | 기본값 | 의미 |
|---|---|---|
| `NABIMAN_DEFAULT_BACKUP_SCHEDULE` | `17 3 * * *` | 새 설치의 기본 예약 시각(UTC 5필드 cron). `off`이면 만들지 않습니다 |
| `NABIMAN_DEFAULT_BACKUP_KEEP` | `7` | 기본 예약의 보관 개수(1–365) |

이 값은 새 데이터 디렉터리에서 처음 시작할 때만 적용합니다. 이미 쓰던 설치에는 예약을 자동으로 추가하지 않습니다. 예약 파일이 빈 목록(`[]`)이어도 마찬가지입니다. 만들어진 예약은 다른 예약처럼 **관리 → 백업**이나 API에서 끄거나 지울 수 있습니다.

기존 설치에서는 관리자 권한으로 API를 호출해 같은 예약을 만듭니다.

```sh
curl -X POST https://나비맨/api/backup/schedules -H 'Content-Type: application/json' \
  -d '{"name":"NabiMan 데이터 자동 백업","kind":"state","paths":[],"cron_expr":"17 3 * * *","keep":7}'
```

`kind`는 `paths`(기존 방식: 지정한 경로를 tar로 묶음) 또는 `state`(데이터 디렉터리 스냅숏)입니다. `state`에서는 `keep`(1–365, 기본 7)과 `include_secret_key`(기본 false)를 쓸 수 있습니다. `/api/backup/schedules/{id}/run`으로 바로 실행합니다.

`backups/`는 데이터 디렉터리와 같은 디스크에 있습니다. 디스크 장애에 대비하려면 스냅숏을 정기적으로 다른 호스트나 저장소로 복사해야 합니다.

### 명령으로 스냅숏 만들기

```sh
umask 077
nabiman-recovery snapshot /var/lib/nabiman /var/backups/nabiman            # secret.key 제외
nabiman-recovery snapshot /var/lib/nabiman /var/backups/nabiman --with-secret-key
nabiman-recovery snapshot /var/lib/nabiman /var/backups/nabiman --mec-db /srv/mec/mec.db
```

서버를 멈추지 않아도 됩니다. 출력은 `{"filename": "...", "secret_key_included": false}`입니다.

## 복원

복원은 항상 **새 빈 디렉터리**에 합니다. 기존 데이터 디렉터리는 덮어쓰지 않습니다.

```sh
umask 077
nabiman-recovery restore-state /var/backups/nabiman/nabiman-state-20261110T031700Z.tar.gz \
  /var/lib/nabiman-restored --secret-key /안전한/위치/secret.key
```

1. 아카이브를 같은 상위 디렉터리의 비공개 작업 디렉터리에 풉니다. 파일은 0600, 디렉터리는 0700입니다.
2. manifest와 비교해 파일 목록·크기·SHA-256을 확인하고 데이터베이스마다 `PRAGMA quick_check`를 실행합니다.
3. 따로 보관한 `secret.key`를 넣습니다. 아카이브에 키가 들어 있으면 같은 키인지 확인합니다. 저장된 서버 비밀번호와 전송 인증 정보가 이 키로 실제로 풀리는지도 확인합니다. 키가 틀리면 아무것도 만들지 않고 중단합니다.
4. 모두 통과하면 새 디렉터리 이름으로 원자적으로 공개합니다(rename, 덮어쓰기 없음). 원래 manifest는 `.restored-from-snapshot.json`으로 남습니다.

출력 예:

```json
{"created_at":"…","nabiman_version":"0.1.0","files":41,"databases":{"mec.db":1,"topology.sqlite3":1,…},"secret_key":"supplied","encrypted_credentials":3}
```

`secret_key`가 `missing`이면 키 없이 복원한 것입니다. 이 경우 `encrypted_credentials`개의 자격 증명을 아직 풀 수 없습니다. 나중에 키를 `새 디렉터리/secret.key`(0600)에 넣거나 해당 비밀번호를 다시 입력합니다.

복원한 상태를 운영에 반영하는 절차:

```sh
systemctl stop nabiman
mv /var/lib/nabiman /var/lib/nabiman.before-restore     # 지우지 말고 보관
mv /var/lib/nabiman-restored /var/lib/nabiman
systemctl start nabiman                                  # 로그에서 잠금·권한·마이그레이션 결과 확인
```

로그인, 서버 목록과 저장한 비밀번호로 접속, 작업 지도, MEC 테넌트를 확인한 다음에 보관해 둔 이전 디렉터리를 정리합니다. 다른 호스트에서 복원 연습을 정기적으로 해 두기를 권합니다.

### 오프라인 복구 묶음과의 관계

`management-recovery.py capture`(README의 「관리 데이터 오프라인 복구」)는 서버를 멈춘 상태에서 데이터 디렉터리, MEC DB, 실행 파일, 정적 UI를 passphrase로 암호화한 age 묶음 하나로 만듭니다. 바이너리까지 함께 되돌려야 할 때나 외부 반출용으로 씁니다. 이제 `mec.db`는 기본적으로 데이터 디렉터리 안에 있으므로 `--database`에는 `/var/lib/nabiman/mec.db`를 줍니다. 데이터 디렉터리 스냅숏(`snapshot`/`restore-state`)은 서버를 멈추지 않고 매일 자동으로 남기는 용도입니다.

## 업그레이드와 다운그레이드

- **업그레이드**: 업그레이드 전에 스냅숏을 하나 만듭니다(`nabiman-recovery snapshot`). 새 버전은 시작할 때 필요한 마이그레이션을 데이터베이스마다 한 트랜잭션으로 적용합니다. 마이그레이션이 실패하면 그 데이터베이스는 이전 버전 그대로 남습니다. `mec.db`이면 서버가 시작하지 않습니다. 작업 지도, Claude 이벤트, 서버 정보, 감사 전송 데이터베이스이면 해당 기능만 쓸 수 없게 되고 원인이 로그에 한 번 남습니다. 이번 버전으로 처음 올리면 `./data/mec.db`를 데이터 디렉터리로 한 번 옮깁니다. 이전 위치를 직접 지정해 쓰고 있었다면 `NABIMAN_MEC_DB`를 계속 지정해도 됩니다.
- **다운그레이드**: 데이터베이스의 스키마 버전이 바이너리가 아는 버전보다 높으면 이전 바이너리는 그 데이터베이스를 열지 않습니다. 파일은 바꾸지 않습니다. 이전 버전으로 돌아가려면 업그레이드 전에 만든 스냅숏을 새 디렉터리에 복원해서 씁니다. 현재 모든 데이터베이스가 버전 1이므로 이 기능 이전 바이너리로도 열 수 있습니다. 단, 이전 바이너리는 `mec.db`를 `./data/mec.db`에서 찾으므로 `NABIMAN_MEC_DB=데이터 디렉터리/mec.db`를 지정해야 합니다.
- 스냅숏 형식(`nabiman-state` 버전 1)보다 새로운 형식은 복원 도구가 거부합니다.
- 업그레이드 중에도 데이터 디렉터리 잠금은 그대로 적용됩니다. 새 서버는 이전 서버가 완전히 끝난 뒤에야 시작합니다. 후보 바이너리를 먼저 띄워 보는 배포 스크립트는 복사한 별도 디렉터리를 쓰므로 영향이 없습니다.
