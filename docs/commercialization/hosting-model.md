# 나비맨 서비스 호스팅 모델: 고객(조직) 분리와 운영 구조

2026-11-11 작성. 나비맨을 **고객이 설치하는 제품**과 **우리가 운영하는 서비스** 두 가지로 판매하기 위한 기반 설계다. 이 문서의 구현 범위(아래 「이번 단계에서 구현한 것」)는 기존 단일 조직 설치의 동작을 바꾸지 않는다. 가격·요금제 내용은 [billing.md](billing.md)에 있다.

## 결론

- **기본 모델은 (a) 고객마다 나비맨 인스턴스 하나**다. 우리가 고객별 VM(또는 고객 전용 컨테이너+전용 호스트)을 운영하고, 그 안에서 지금의 단일 조직 나비맨을 그대로 돌린다.
- **(b) 한 인스턴스 안에 여러 조직**은 이번 단계에서 구현하지 않는다. 나중에 **원격 서버 전용 요금제**(호스트 터미널·방화벽·패키지·파일 관리자 없이, 고객 서버에 SSH/RDP로만 접속하는 상품)를 싸게 팔아야 할 때만 다시 검토한다. 그때 필요한 저장소별 `org_id` 계획만 아래에 적는다.
- 이유: 나비맨의 핵심 기능 상당수가 **나비맨이 돌고 있는 호스트 자체에 root에 가까운 권한으로 작용**한다(호스트 터미널, 방화벽, 패키지, 서비스, 파일 관리자, cron, 디스크·스왑, 웹 서버 설정). 이런 기능이 있는 프로세스를 여러 고객이 공유하면 한 고객이 다른 고객의 데이터·세션·SSH 키를 읽을 수 있다. 코드 수준의 분리로 막을 수 없는 문제이므로 프로세스·호스트 단위로 분리해야 한다.

## 용어: 세 가지 "tenant"를 섞지 않는다

| 이름 | 무엇인가 | 코드 위치 |
|---|---|---|
| **고객 조직(organisation)** | 계약·청구·회원의 경계. 이 문서의 주제 | 아직 없음. (a)에서는 "인스턴스 = 조직" |
| **MEC 테넌트** | 5G MEC 테스트베드의 Kubernetes 실행 자원(namespace·Rancher 프로젝트·Harbor·GPU 할당) | `backend/src/mec/**`, `mec.db` |
| **소속 범위(scope)** | 한 인스턴스 안에서 회원에게 서버 또는 MEC 테넌트 단위로 권한을 위임하는 것 | `Membership{kind=server\|tenant, scope_id, role}`, `scope_access.rs`, `scope_settings.rs` |

MEC 테넌트는 한 고객(예: 테스트베드 운영 기관) 안의 실행 환경이다. 고객 조직과 같은 ID로 묶지 않는다. 이 방향은 [architecture.md](architecture.md)의 「조직과 자원 모델」과 같다.

## 지금 있는 것 (2026-11-11, fb497de 기준)

- **회원**: `users.json` 하나. 역할 `admin`(최고관리자) / `operator` / `viewer` / `remote`(원격 전용 회원). 상태 `active`·`pending`·`suspended`·`rejected`. 가입 신청은 최고관리자가 승인한다.
- **위임**: `memberships`(서버 또는 MEC 테넌트 범위의 역할, 만료, 명령 실행 허용)와 `scope_restricted`. 범위가 있는 계정은 `scope_access`의 허용 목록만 쓴다. 테넌트별 회원 수 제한은 `scope-settings.json`(`scope_settings::check_capacity`).
- **원격 전용 회원**: 자기 서버를 직접 등록할 수 있다(`remote_servers.json`의 `owner_id`). 최대 대수는 회원별 `remote_permissions.max_servers`. 규칙은 `member_servers.rs`.
- **라이선스**: Ed25519 서명 토큰(`NBL1.`), 공개키는 `NABIMAN_LICENSE_PUBLIC_KEYS_FILE`, 발급 도구는 `tools/license-issuer.py`. 등급 `free/pro/enterprise`, 기능 목록, `max_servers`가 있었지만 **아무것도 집행하지 않았다**(화면 표시만). 라이선스 파일이 없으면 30일 체험.
- **인스턴스 단위 운영 기반**: 설정 파일(`NABIMAN_CONFIG`, Phase 6), 데이터 디렉터리 잠금(한 디렉터리에 서버 하나), `/healthz`·`/readyz`, `/metrics`(`NABIMAN_METRICS_TOKEN`), 서명된 업데이트(`nabiman-update`, 실패 시 자동 되돌림), 상태 스냅숏(`nabiman-recovery snapshot`), run mode(`root`/`service`).
- **조직 개념은 없다.** 모든 저장소가 인스턴스 전체에 하나씩이다.

## (a)와 (b) 비교

| 항목 | (a) 고객별 인스턴스 | (b) 공유 인스턴스 + 조직 |
|---|---|---|
| 격리 | 프로세스·파일·OS 사용자·(VM이면) 커널까지 분리. 한 고객의 침해가 다른 고객으로 번지지 않음 | 모든 요청·저장·스트림에서 조직을 검증해야 함. 한 군데 빠지면 데이터 유출 |
| 호스트 root 기능 | 고객 자신의 VM이므로 그대로 제공 가능 | **제공 불가**. 공유 호스트의 터미널·방화벽·패키지·파일 관리자는 다른 고객 데이터에 닿음 |
| 코드 변경 | 거의 없음(이번 단계의 부트스트랩·사용량·라이선스 정도) | 모든 저장소·핸들러·WebSocket·tmux·감사·백업 분할. 수개월 + 보안 검증 |
| 운영비 | 고객마다 VM·메모리·디스크. 작은 고객에게 비쌈 | 고객당 비용 낮음 |
| 장애 범위 | 그 고객만 | 전체 고객 |
| 업그레이드 | 고객별 일정·canary·되돌림 가능(`nabiman-update`) | 모두 동시에. 한 고객 요구로 버전을 고정할 수 없음 |
| 백업·복구 | 인스턴스 스냅숏 그대로. 고객별 복원이 쉬움 | 조직 하나만 복원하는 도구가 새로 필요 |
| 감사 | 인스턴스별 해시 체인 감사 기록 그대로 | 조직별 체인·조회 분리 필요 |
| 데이터 반환·삭제 | 데이터 디렉터리 통째로 | 저장소마다 조직 데이터 추출·삭제 |
| 규제·계약 | "고객 전용 환경"이라고 설명 가능 | 논리 분리 증명 부담 |

### (b)를 하려면 나눠야 하는 것 (공유하면 안 되는 것)

모든 저장소, 터미널 세션, tmux 서버, SSH 키, 감사 기록, 백업이 조직별로 나뉘어야 한다.

| 저장소·자원 | 지금 | (b)에서 필요한 것 |
|---|---|---|
| `users.json` | 하나 | 사용자마다 `org_id`. 사용자 이름 유일성은 조직 안에서만 |
| `login_sessions.json`, JWT | 공통 `jwt_secret` | 토큰 claims에 `org`, 검증 시 조직 일치 |
| `api_keys.json` | 하나 | 키마다 `org_id`, 범위 검사에 조직 포함 |
| `remote_servers.json`, `server-facts.sqlite3` | 하나 | 서버마다 `org_id`. 목록·점검·실행·SFTP·VNC 모두 조직 검사 |
| `secret.key` | 인스턴스 하나 | 조직별 데이터 키(키 하나 유출 = 전체 유출 방지) |
| `ssh/`, `ssh-ca/`, `ssh-host-trust/` | 인스턴스 SSH 키·CA 하나 | 조직별 키·CA·호스트 신뢰 |
| `terminal_sessions.json`, tmux 서버 | 하나의 tmux 소켓 | 조직별 OS 사용자 + 조직별 tmux 소켓(같은 소켓이면 서로 attach 가능) |
| `terminal_shares.json`, `console_shares.json` | 하나 | 공유 링크에 `org_id` |
| `session-recordings/` | 하나 | 조직별 디렉터리·보존 정책 |
| `audit.jsonl`, `audit-forwarding.sqlite3` | 체인 하나 | 조직별 체인(또는 레코드에 `org_id` + 조직별 조회·내보내기·전송) |
| `backup_schedules.json`, `backups/` | 인스턴스 전체 | 조직 단위 내보내기·복원 |
| `notifications.json`, `alert_rules.json` | 하나 | 조직별 채널(다른 고객에게 알림이 가지 않게) |
| `scope-settings.json`, `mec.db` | 인스턴스 | MEC 테넌트에 상위 `org_id` |
| `topology.sqlite3`, `claude-events.sqlite3` | 하나 | `org_id` 열 |
| `license.json` | 인스턴스 하나 | 조직별 entitlement |
| `usage.sqlite3`(이번 단계) | 인스턴스 하나 | 일자 + `org_id` 기준 집계 |

호스트 터미널, 방화벽, 패키지, 서비스, cron, 디스크, 웹 서버 설정, 파일 관리자(로컬), 데이터베이스 메뉴는 (b)에서는 **제공하지 않는다**. run mode를 `service`로 고정하고 그 기능들의 경로를 막는 별도 "remote-only" 빌드 또는 설정이 필요하다.

## 권장: (a)와 그 이유

1. 나비맨 매출의 핵심 기능(호스트 관리)은 (b)에서 팔 수 없다.
2. (a)는 지금 코드를 거의 그대로 쓴다. 이미 인스턴스 단위로 설정 파일, 데이터 디렉터리 잠금, 상태 스냅숏, 서명 업데이트, 상태 점검이 있다.
3. 격리 증명이 쉽다. 고객별 VM이면 "다른 고객과 커널을 공유하지 않는다"고 말할 수 있다.
4. 비용 문제는 작은 VM(예: 1–2 vCPU, 2–4 GB)과 고객별 과금으로 먼저 대응하고, 실제로 원격 전용 소형 고객 수요가 확인되면 (b)를 다시 판단한다.

컨테이너로 운영하는 경우: 호스트 터미널은 **컨테이너 안의 셸**이 된다. 컨테이너에 privileged·hostPath·호스트 네트워크를 주지 않는다. 방화벽·패키지 같은 기능은 그 컨테이너 안에서만 의미가 있으므로 고객에게 "호스트 관리"를 약속하려면 VM이 맞다. 같은 노드의 컨테이너끼리는 커널을 공유하므로, 적대적일 수 있는 고객 사이라면 gVisor/Kata 같은 샌드박스 런타임이나 고객별 노드를 쓴다.

## (a)의 프로비저닝: 호스팅 컨트롤러 스케치

호스팅 컨트롤러는 나비맨 밖의 별도 프로그램이다(이번 단계에서 만들지 않음). 고객 인스턴스를 만들고, 지키고, 계량하고, 없앤다.

```text
주문/계약 ──► 호스팅 컨트롤러 ──┬─► 인프라 API (VM 또는 컨테이너 생성, 디스크, 네트워크)
                                 ├─► DNS / TLS (고객 도메인, ACME)
                                 ├─► 라이선스 발급기 (오프라인 키 보관소)
                                 ├─► 인스턴스: nabiman-server --bootstrap-admin
                                 ├─► 인스턴스: GET /api/usage  (매일, NABIMAN_USAGE_TOKEN)
                                 ├─► 인스턴스: /healthz /readyz /metrics
                                 ├─► 백업 저장소 (nabiman-recovery snapshot → 암호화 → 외부 보관)
                                 └─► 청구 시스템 (billing.md)
```

| 단계 | 컨트롤러가 하는 일 | 나비맨 쪽 기반 |
|---|---|---|
| 1. 인스턴스 생성 | VM(또는 전용 컨테이너) 생성, OS 사용자, 디스크 할당 | 설치 패키지, `install.sh`, `nabiman-update` |
| 2. 데이터 디렉터리 | `/var/lib/nabiman`(인스턴스마다 하나, 0700). 한 디렉터리에 서버 하나는 잠금이 보장 | `data_dir::lock` |
| 3. 설정 파일 | `/etc/nabiman/nabiman.toml`: 포트, `run_mode`, `trusted_proxies`, `metrics_token`, `usage_token`, `product_name` 등 | `NABIMAN_CONFIG` (Phase 6, [configuration.md](../operations/configuration.md)) |
| 4. 도메인·TLS | `<고객>.nabiman.example` 또는 고객 도메인. TLS는 앞단 프록시(인그레스)에서 종료, `trusted_proxies` 설정 | `security_headers`, `trusted_proxy` |
| 5. 관리자 부트스트랩 | 임의 비밀번호 생성 → 비밀 저장소 → `nabiman-server --bootstrap-admin <이름> --password-file -` (stdin) → 고객에게 일회용 전달 경로로 안내, 첫 로그인 후 변경·MFA 권장 | **이번 단계 구현** |
| 6. 라이선스 | 인스턴스마다 하나. `license_id`에 인스턴스 ID를 포함, `limits.deployment = "hosted"`, 계약 요금제의 한도. 공개키 파일 배포 | **이번 단계: plan 필드** |
| 7. 서비스 시작 | systemd `nabiman` 시작, `/readyz` 200 확인 | `observability::health` |
| 8. 백업 | 매일 `nabiman-recovery snapshot DATA_DIR OUT`(서버 실행 중에도 일관) → 고객별 키로 암호화 → 다른 지역 저장소. `secret.key`는 따로 보관. 복원 연습 주기 | `state_snapshot`, `backup_crypto` |
| 9. 업그레이드 | 내부 canary 인스턴스 → 일부 고객 → 전체. `nabiman-update --version vX` 실패 시 자동 되돌림. 고객 유지보수 시간대 존중 | `packaging/nabiman-update` |
| 10. 계량 | 매일(또는 매시간) `GET /api/usage?from=..&to=..` 수집 → 청구 원장 | **이번 단계 구현** |
| 11. 모니터링 | `/healthz`, `/readyz`, `/metrics`(scrape 토큰), 자체 점검 알림 | Phase 9 |
| 12. 정지·해지 | 미납 시: 라이선스 만료 → 유예 → 추가 중지(데이터 잠금 없음). 해지 시: 최종 스냅숏을 고객에게 반환 → 보관 기간 뒤 VM·디스크·백업 삭제(삭제 증명) | 라이선스 유예(이번 단계) |

### 나비맨에 필요한 변경

| 필요 | 상태 |
|---|---|
| 헤드리스 관리자 부트스트랩(CLI) | **구현**: `--bootstrap-admin` |
| 인스턴스별 설정 파일 | 있음(Phase 6) |
| 인스턴스별 라이선스, 요금제 한도, 유예 | **구현**: `info.limits`, 부드러운 집행 |
| 사용량 보고 엔드포인트 | **구현**: `GET /api/usage`, `NABIMAN_USAGE_TOKEN` |
| 인스턴스 식별 | 있음: `/api/usage`의 `instance.instance_id`(RuntimeIdentity) |
| 라이선스를 특정 인스턴스에 묶기(복사 방지) | 미구현. 호스팅은 우리가 파일을 넣으므로 급하지 않음. 설치형에는 후속 과제 |
| 컨트롤러가 라이선스를 API로 넣기 | 지금은 최고관리자 로그인 후 `POST /api/license/activate` 또는 데이터 디렉터리에 `license.json` 배치. 토큰 기반 활성화는 후속 |
| 사용량 push(외부 송신) | 미구현. 지금은 컨트롤러가 pull. 설치형 고객의 외부 송신은 계약·동의 사항 |

## 이번 단계에서 구현한 것

### 1. 헤드리스 관리자 부트스트랩

```sh
# 비밀번호는 파일 또는 stdin(-)으로만. 명령줄 인자·환경 변수로 받지 않는다.
printf '%s\n' "$PASSWORD" | nabiman-server --bootstrap-admin owner --password-file -
nabiman-server --bootstrap-admin owner --password-file /run/secrets/nabiman-admin
```

- 데이터 디렉터리는 설정 파일·`NABIMAN_DATA_DIR`의 것을 쓴다(설정 파일을 먼저 읽는다).
- `users.json`이 없을 때만 최고관리자 한 명을 만들고 종료한다. 서버는 시작하지 않는다. 이후 서버는 `NABIMAN_PASSWORD` 없이 시작한다.
- 종료 코드: `0` 생성, `1` 잘못된 인자·이름·비밀번호(웹 폼과 같은 규칙: 이름 2–32자, 비밀번호 12자 이상 72바이트 이하, 공개된 기본 비밀번호 거부), `2` 이미 회원이 있음(아무것도 바꾸지 않음), `3` 다른 서버가 디렉터리를 잡고 있거나 쓸 수 없음.
- 비밀번호는 bcrypt 해시로만 저장되고 출력되지 않는다. 비밀번호 파일의 마지막 줄바꿈 하나만 제거한다.

### 2. 사용량 보고 `GET /api/usage`

권한: 최고관리자, 또는 `Authorization: Bearer <NABIMAN_USAGE_TOKEN>`(설정 파일 키 `usage_token`). 토큰은 `/api/usage`만 연다. metrics 토큰과 별개다. 조회 기간: `?from=YYYY-MM-DD&to=YYYY-MM-DD`(UTC, 양 끝 포함, 기본 최근 30일, 최대 400일).

| 필드 | 뜻 | 출처 |
|---|---|---|
| `generated_at` | 응답 시각(RFC 3339) | |
| `period.from`, `period.to`, `period.days` | 조회 기간 | 요청 |
| `instance` | `package_version`, `git_commit`, `executable_sha256`, `instance_id`, `started_at` | RuntimeIdentity |
| `users.total/active/pending/suspended/rejected` | 상태별 계정 수 | `users.json` |
| `users.admins/operators/viewers/remote_members` | 활성 계정의 역할별 수 | `users.json` |
| `servers.saved`, `servers.member_owned` | 저장한 원격 서버 수, 그중 원격 전용 회원 소유 | `remote_servers.json` |
| `servers.peak` | 기간 중 최대 서버 수 | 사용량 원장 |
| `terminal_sessions.open`, `.attached` | 지금 열린 세션, 브라우저가 붙어 있는 세션 | 터미널 레지스트리 |
| `terminal_sessions.peak_open` | 기간 중 동시에 열린 최대 세션 수 | 사용량 원장 |
| `terminal_sessions.session_hours` | 기간 중 세션이 열려 있던 시간의 합(세션 수 × 시간) | 사용량 원장 |
| `terminal_sessions.attached_hours` | 그중 브라우저가 붙어 있던 시간의 합 | 사용량 원장 |
| `metering.sample_interval_secs` | 표본 간격(60초) | |
| `metering.sampled_days` | 기간 중 표본이 있는 날 수 | 사용량 원장 |
| `metering.first_sampled_day` | 원장의 첫 날. 이 날 이전은 측정하지 않은 것(0이 아님) | 사용량 원장 |
| `metering.instance_hours` | 기간 중 서버가 실행된 시간 | 사용량 원장 |
| `metering.peak_active_users` | 기간 중 최대 활성 계정 수(좌석 과금용) | 사용량 원장 |
| `storage.bytes`, `.files` | 데이터 디렉터리 파일 크기 합, 파일 수(링크는 따라가지 않음) | 파일 시스템 |
| `storage.complete` | 항목 상한(20만)·읽을 수 없는 디렉터리 때문에 덜 셌으면 false | |
| `storage.largest[]` | 데이터 디렉터리 최상위 항목 중 큰 것 8개 `{name, bytes}`(예: `backups`, `session-recordings`) | |
| `licence.license_id/tier/validation_state/expires_at` | 설치된 라이선스 | `license.json` |
| `licence.plan/deployment/max_users/max_servers/plan_features` | 요금제가 있는 라이선스의 한도(없으면 null·빈 목록, `max_servers`는 기존 값) | `license.json` |

**사용량 원장** `usage.sqlite3`(0600, 스키마 버전 관리, 800일 보존): 서버가 1분마다 열린 세션 수·붙은 세션 수·활성 계정 수·서버 수를 보고 UTC 날짜별로 더한다. 두 표본 사이가 2분보다 길면(정지·중단) 2분으로만 센다. 감사 기록은 메모리에 최근 1,000건만 두고 파일이 회전하며, 터미널 레지스트리는 닫힌 세션을 잊기 때문에 "지난달 세션-시간"을 정확히 계산할 수 없다. 그래서 원장을 따로 둔다. 원장은 이 버전이 배포된 날부터 쌓인다. Claude 메시지 기록(14일 보존)은 과금 기준으로 쓰지 않는다.

### 3. 요금제·한도·유예가 있는 라이선스

라이선스 `info`에 선택 필드 `limits`를 추가했다. **`limits`가 없는 라이선스(기존 라이선스 전부)와 체험·라이선스 없음·검증 실패는 이전과 똑같이 동작한다**: 아무것도 거부하지 않고 경고도 없으며, 만료되면 유예 없이 `license_expired`다.

```json
{
  "license_id": "acme-hosted-2026-001",
  "product": "nabiman",
  "info": {
    "tier": "enterprise", "holder": "ACME", "issued_at": "2026-11-11", "expires_at": "2027-11-10",
    "max_servers": 50,
    "features": ["basic_monitoring", "management", "alerts", "backups"],
    "limits": {
      "plan": "business", "deployment": "hosted",
      "max_users": 25, "max_servers": 50,
      "features": ["orchestration", "graph_3d", "recording"],
      "grace_days": 14
    }
  }
}
```

| 필드 | 규칙 |
|---|---|
| `plan` | 필수. 소문자·숫자·`-`·`_`, 1–32자 |
| `deployment` | 선택. `installed` 또는 `hosted` |
| `max_users` | 선택(없으면 무제한), 1 이상. **활성 계정** 수 기준 |
| `max_servers` | 선택(없으면 무제한), 0 이상. 저장한 원격 서버 수. `info.max_servers`와 같아야 함(요금제가 있으면 등급별 고정 한도 free=1, pro≤10은 적용하지 않음) |
| `features` | 요금제 기능. 이 버전이 아는 것: `orchestration`, `graph_3d`, `recording`, `mec`. 모르는 이름(새 발급기의 것)은 허용하되 아무 권한도 주지 않음. 중복 거부, 최대 32개 |
| `grace_days` | 선택(기본 14), 0–90. 만료일 다음 날부터 이 일수 동안 유예 |

집행(부드럽게):

- 사용자 추가(`POST /api/users`)와 가입 승인·정지 해제(비활성 → 활성)는 활성 계정이 `max_users`에 도달하면 거부한다. 서버 추가(`POST /api/remote-servers`)는 `max_servers`에서 거부한다. 기존 계정·서버·데이터는 잠그거나 숨기거나 지우지 않는다. 한도를 넘은 상태(다운그레이드 등)에서도 기존 것은 모두 그대로 쓴다.
- 요금제에 `recording`이 없으면 세션 녹화를 **켜는 것**만 거부한다. 이미 켜진 녹화와 끄기는 그대로다. `orchestration`·`graph_3d`·`mec`는 이번 단계에서 화면 표시만 하고 막지 않는다(집행 위치는 후속 결정).
- 만료 후 유예 기간: `validation_state = "grace"`, 모든 것이 그대로 동작, 화면에 유예 종료일 표시.
- 유예가 끝나면: `license_expired`. 새 사용자·서버 추가만 멈춘다. 나머지 기능과 데이터는 그대로다.
- 공개키 파일이 없거나 서명이 틀리면 요금제를 믿을 수 없으므로 한도를 적용하지 않는다(이전과 같음). 설치형 고객은 root를 가지므로 이 집행은 계약 준수 보조 장치이지 복제 방지가 아니다.

화면: 라이선스 화면에 요금제·제공 방식·사용/한도·요금제 기능·유예 종료일, 경고 목록. 최고관리자에게는 헤더 아래에 한 줄 배너(한도 도달·초과, 만료 30일 전, 유예 중, 유예 종료). 한국어·영어·일본어. `GET /api/license` 응답에 `limits`, `grace_ends_at`, `grace_days_remaining`, `usage{users,servers}`(요금제가 있을 때만), `notices[]`가 추가됐다.

발급: `tools/license-issuer.py issue`가 `limits`를 같은 규칙으로 검사해 서명한다. `limits` 없는 claims는 이전과 바이트 단위로 같은 토큰이 나온다. 주의: 이전 버전 나비맨은 `limits`가 있는 라이선스를 "알 수 없는 필드"로 거부한다. 요금제 라이선스는 이 버전 이상에만 발급한다.

## 기존 설치와 이 서버에 미치는 영향

- 라이선스 파일에 `limits`가 없으면 화면·API·집행 모두 이전과 같다. `/api/license` 응답에 null 필드와 빈 `notices`가 늘어날 뿐이다.
- 새 파일 `usage.sqlite3`가 데이터 디렉터리에 생기고 1분마다 한 줄을 갱신한다(백업에 포함, 수 KB 수준).
- `--bootstrap-admin`은 명령줄에서 요청할 때만 동작한다.
- `NABIMAN_USAGE_TOKEN`을 정하지 않으면 `/api/usage`는 최고관리자만 읽는다.

## (b)를 다시 검토할 때의 조건

원격 서버 전용 요금제를 공유 인스턴스로 운영하려면 최소한: 위 표의 저장소별 `org_id`, 조직별 OS 사용자와 tmux 소켓, 조직별 SSH 키·CA·`secret.key`, 조직 검증을 빠뜨리면 실패하는 테스트(목록·검색·SSE·WebSocket·내보내기·감사), run mode `service` 고정과 호스트 기능 경로 차단, 조직 단위 백업·복원·삭제가 필요하다. 그 전에 (a)의 소형 VM 원가와 원격 전용 고객 수요를 측정해 (b)의 개발비(수개월)와 비교한다.
