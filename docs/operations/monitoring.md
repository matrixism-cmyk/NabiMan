# 모니터링과 운영

NabiMan 이 자기 상태를 알리는 방법: 상태 확인 주소, 로그, 지표(metrics), 자체 점검 알림.
설정 이름과 기본값은 [configuration.md](configuration.md) 의 "로그와 모니터링" 표에도 있습니다.

## 상태 확인 주소

로그인 없이 부를 수 있고, 로그인 시도 제한(rate limit)에 걸리지 않습니다.
답에는 경로·버전·오류 원문 같은 내부 정보가 들어가지 않습니다.

| 주소 | 뜻 | 정상 | 비정상 |
|---|---|---|---|
| `GET /healthz` | 살아 있음(liveness): 프로세스가 요청에 답함 | `200 ok` (text/plain) | 답 없음 |
| `GET /readyz` | 준비됨(readiness): 요청을 제대로 처리할 수 있음 | `200 {"status":"ready","reasons":[]}` | `503 {"status":"not_ready","reasons":[...]}` |

`/readyz` 가 보는 것과 `reasons` 에 나오는 낱말:

| 낱말 | 뜻 |
|---|---|
| `data_dir_not_writable` | 데이터 디렉터리에 파일을 만들고 쓰고 지울 수 없음 (디스크 가득, 읽기 전용, 권한) |
| `audit_journal_unavailable` | 감사 기록 저장이 멈춤 (`/api/audit/health` 에서 이유 확인) |
| `tmux_unreachable` | 터미널이 쓰는 tmux 가 5초 안에 답하지 않음, 또는 세션이 등록돼 있는데 tmux 서버가 없음 |
| `probe_failed` | 점검 자체가 실행되지 못함 |

- tmux 점검은 터미널과 같은 제한 시간(5초) 있는 호출로 서버 자신의 tmux 에 `has-session` 만 묻습니다.
  tmux 서버를 새로 띄우지 않습니다. 열린 세션이 하나도 없어 tmux 서버가 없는 것은 정상입니다.
- 결과는 5초 동안 재사용합니다. 자주 불러도 tmux 클라이언트가 그만큼 늘지 않습니다.

### Docker HEALTHCHECK

Dockerfile 에 넣을 줄 (이미지에 `curl` 이 있을 때):

```dockerfile
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1
```

`curl` 이 없으면 `wget -qO- http://127.0.0.1:8080/healthz` 를 씁니다. 포트는 `NABIMAN_PORT` 에 맞춥니다.
컨테이너 상태에는 `/healthz` 를 권합니다. `/readyz` 를 쓰면 tmux·디스크 문제로도 unhealthy 가 되어
오케스트레이터가 재시작할 수 있습니다 (원하는 동작이면 `/readyz`).

### nginx · 부하분산기

부하분산기의 상태 확인에는 `/readyz` 를 씁니다. 외부에 공개할 필요가 없으면 막아 둡니다.

```nginx
location = /readyz  { proxy_pass http://127.0.0.1:8080; access_log off; allow 10.0.0.0/8; deny all; }
location = /healthz { proxy_pass http://127.0.0.1:8080; access_log off; }
```

## 로그

### 형식

한 사건이 한 줄입니다: 시각(UTC), 수준, 대상(target), 메시지, `키=값` 필드.

```text
2026-11-11T01:02:03.456Z INFO  http: request method=GET route=/api/users/{id} status=200 ms=4 user=7f3a…
2026-11-11T01:02:05.001Z WARN  mec: dashboard poll failed; while it fails the same way, a reminder follows every 10 min error="…"
```

`NABIMAN_LOG_FORMAT=json` 이면 같은 내용을 JSON 한 줄로 씁니다 (`ts`, `level`, `target`, `msg`, 그리고 필드).

```json
{"ts":"2026-11-11T01:02:03.456Z","level":"info","target":"http","msg":"request","method":"GET","route":"/api/users/{id}","status":"200","ms":"4","user":"7f3a…"}
```

- `error`·`warn` 은 표준 오류(stderr), `info`·`debug` 는 표준 출력(stdout) 으로 갑니다.
  supervisor 는 `nabiman.err.log` 와 `nabiman.log` 로, journald 는 우선순위 없이 한 저널로 받습니다.
- 메시지 속 줄바꿈은 `\n` 으로 바뀌어 한 줄을 지킵니다 (가짜 줄을 끼워 넣을 수 없음).

### 수준과 거르기

`NABIMAN_LOG` 로 정합니다. 기본은 `info`.

| 값 | 뜻 |
|---|---|
| `info` | 기본. 요청 한 줄씩과 시작·변경 알림 |
| `warn` | 경고와 오류만 (요청 줄 없음) |
| `debug` | 상태 확인 주소(`/healthz`, `/readyz`, `/metrics`) 요청 줄까지 |
| `off` | 아무것도 쓰지 않음 |
| `warn,http=info` | 대상별: 전체는 warn, 요청 줄(`http`)은 info |
| `info,mec=error` | MEC 관련은 오류만 |

대상 이름: `http`(요청), `mec`, `mec.db`, `mec.membership`, `mec.axgate`, `terminal`, `terminal.claude`,
`serial`, `sftp`, `sessions`, `users`, `auth`, `audit`, `audit.forwarding`, `backup`, `settings`,
`data_dir`, `dns`, `remote`, `storage`, `external_tools`, `selfcheck`. `mec` 는 `mec.db` 같은 하위 대상도 덮습니다.

### 요청 로그

요청마다 `info` 한 줄: 메서드, **경로 패턴**(`/api/users/{id}` — 실제 id 가 들어간 경로가 아님), 상태 코드,
걸린 시간(ms), 로그인한 계정의 id(없으면 `-`). 쿼리 문자열과 본문은 절대 쓰지 않습니다.
정적 파일은 `<static>`, 등록되지 않은 API 경로는 `<unmatched>` 로 적습니다.

### 비밀 값

모든 메시지와 필드는 쓰기 전에 걸러집니다. 이름에 password·secret·token·api_key·authorization·cookie·
credential·private_key 등이 들어간 필드는 값 대신 `[redacted]`, 본문 속 `password=…`, `"token":"…"`,
`Authorization: Bearer …`, `nbm_…` API 키, JWT, `scheme://user:비밀번호@host` 의 비밀번호도 가립니다.
(테스트: `backend/src/observability/log_tests.rs`)

### 반복되는 실패는 한 번만

같은 실패가 계속될 때(예: 쿠버네티스 권한이 없어 대시보드 폴링이 10초마다 실패) 처음 한 번 쓰고,
같은 실패가 이어지는 동안은 10분마다 횟수와 함께 한 줄, 회복하면 `… recovered failures=N failed_for_secs=S` 한 줄을 씁니다.
다른 실패로 바뀌면 새 소식으로 보고 바로 씁니다. 적용된 곳: MEC 대시보드 폴링, 관리형 멤버십 점검.

마지막 오류는 로그 대신 MEC 진단에서 봅니다: `GET /api/mec/v1/health/summary` 의 `dashboard_poll`
(`failing`, `last_error`, `failing_since`, `consecutive_failures`, `last_success`).

> 2026-11-11 이 서버의 `nabiman.err.log` 는 `[mec] dashboard poll failed … nodes is forbidden` 이 시간당 360줄
> (10초마다, 2026-11-03 이후 62,734줄) 로 거의 전부였습니다. 위 방식으로 실패가 이어지는 동안 시간당 6줄이 됩니다.
> 원인(서비스 계정 `nabi:default` 에 nodes list 권한 없음)은 그대로이니 RBAC 을 주거나 MEC 를 끄는 것이 근본 해결입니다.

## 지표 (`/metrics`)

Prometheus 텍스트 형식(0.0.4) 입니다. 기본은 **최고관리자만** 읽을 수 있고(다른 관리 API 와 같은 로그인),
수집기용으로 `NABIMAN_METRICS_TOKEN` 을 정하면 `Authorization: Bearer <그 값>` 으로도 읽습니다.
이 토큰은 `/metrics` 말고 아무것도 열지 않습니다. 32자 이상의 무작위 값을 쓰세요 (`openssl rand -hex 32`).

| 이름 | 종류 | 뜻 |
|---|---|---|
| `nabiman_http_requests_total{route,status}` | counter | 경로 패턴·상태 코드별 요청 수 |
| `nabiman_http_request_duration_seconds` | histogram | 응답 머리까지 걸린 시간 (5ms~30s 구간) |
| `nabiman_terminal_sessions` | gauge | 등록된(열린) 터미널 세션 수 |
| `nabiman_websocket_connections` | gauge | 지금 열린 WebSocket 연결 수 |
| `nabiman_websocket_connections_total` | counter | 열린 WebSocket 연결 누계 |
| `nabiman_child_process_timeouts_total` | counter | 제한 시간에 걸려 멈춘 자식 프로세스 수 |
| `nabiman_audit_write_failures_total` | counter | 로컬 감사 기록에 쓰지 못한 건수 |
| `nabiman_backup_last_success_timestamp_seconds` | gauge | 마지막으로 성공한 예약 백업 시각 (Unix 초, 없으면 0) |
| `nabiman_login_failures_total` | counter | 거절된 로그인 (비밀번호, LDAP, 2단계 인증의 401·403) |
| `nabiman_rate_limited_total` | counter | 요청 제한으로 거절(429)된 요청 수 |

카운터는 프로세스가 다시 시작하면 0 부터 셉니다 (Prometheus 의 `rate()` 가 이를 처리합니다).

### 수집 설정 예

```yaml
scrape_configs:
  - job_name: nabiman
    metrics_path: /metrics
    scheme: https
    authorization:
      type: Bearer
      credentials_file: /etc/prometheus/nabiman-metrics-token
    static_configs:
      - targets: ["nabiman.example.com"]
```

경보 예: `time() - nabiman_backup_last_success_timestamp_seconds > 2 * 86400`,
`rate(nabiman_login_failures_total[5m]) > 1`, `increase(nabiman_audit_write_failures_total[10m]) > 0`.

## 자체 점검 알림

NabiMan 이 자신을 1분마다 점검하고, 문제가 생기면 **알림 채널**(알림 규칙이 쓰는 그 채널: 메일·Slack·웹훅 등)로 보냅니다.

| 점검 | 실패 조건 |
|---|---|
| `disk` | 데이터 디렉터리가 있는 파일시스템의 남은 공간이 10% 미만 |
| `audit` | 감사 기록 저장이 멈췄거나, 지난 점검 뒤 쓰기 실패가 있음 |
| `backup` | 켜진 예약 백업의 마지막 성공이 예약 주기의 2배보다 오래됨 (한 번도 성공 못 하고 실패로 끝난 것 포함) |
| `tmux` | 터미널의 tmux 가 답하지 않음 (`/readyz` 와 같은 판단) |
| `timeouts` | 최근 15분 동안 자식 프로세스 제한 시간 초과가 5번 이상 |

- 실패하기 시작하면 한 번 알리고, 계속 실패하면 `NABIMAN_SELF_CHECK_REPEAT_HOURS`(기본 6시간)마다 다시 알리며,
  회복하면 "recovered" 를 한 번 보냅니다. 같은 내용이 `selfcheck` 대상의 `warn` 로그로도 남습니다.
- `NABIMAN_SELF_CHECKS=off` 로 끄고, `NABIMAN_SELF_CHECKS=disk,backup` 처럼 일부만 켤 수 있습니다.
- 켜진 알림 채널이 없으면 로그에만 남습니다.

## 감사 기록의 "무엇이 바뀌었나"

주요 관리 변경(계정·역할, 서버 추가·수정·삭제, 소속·범위 설정, 앱 설정, 방화벽 규칙, 인증서 발급·갱신, 가상호스트와 그 설정,
Claude 제어 채널로 닫은 세션)의 감사 기록에는 `change` 가 붙습니다: 자원 종류, id, 동작, 바뀐 필드의 이전 → 이후 값.
비밀번호·키 같은 비밀 필드는 값 없이 "바뀜" 만 남습니다. `change` 는 해시 사슬에 포함되며(`v3:` 해시),
그 전 기록(`v2:`·옛 형식)은 그대로 검증됩니다. 감사 로그 화면의 "변경 내용" 칸에 보입니다.

## 로그 보관 (로테이션)

NabiMan 은 로그 파일을 직접 돌리지 않습니다. 실행 관리자에게 맡깁니다.

### supervisor

```ini
[program:nabiman]
stdout_logfile=/var/log/nabiman/nabiman.log
stdout_logfile_maxbytes=50MB
stdout_logfile_backups=5
stderr_logfile=/var/log/nabiman/nabiman.err.log
stderr_logfile_maxbytes=50MB
stderr_logfile_backups=5
environment=NABIMAN_LOG="info",NABIMAN_LOG_FORMAT="json"
```

요청 로그가 `info` 로 stdout 에 쌓이므로 `stdout_logfile_maxbytes` 를 꼭 정하세요 (기본 50MB·10개).
요청 줄이 필요 없으면 `NABIMAN_LOG=warn` 또는 `info,http=warn`.

### systemd / journald

stdout·stderr 는 저널로 갑니다. 크기는 `/etc/systemd/journald.conf` 에서 정합니다.

```ini
[Journal]
SystemMaxUse=1G
SystemMaxFileSize=100M
MaxRetentionSec=30day
```

`journalctl -u nabiman -o cat` 으로 원래 줄을, JSON 형식이면 `journalctl -u nabiman -o cat | jq` 로 볼 수 있습니다.
`RateLimitIntervalSec`/`RateLimitBurst` 기본값(30초에 1만 줄)은 바쁜 서버의 요청 로그를 버릴 수 있으니,
그럴 때는 단위 파일에 `LogRateLimitIntervalSec=0` 을 두거나 `NABIMAN_LOG=info,http=warn` 으로 줄입니다.
