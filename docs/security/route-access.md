# API 경로별 접근 권한

> 이 문서는 `backend/src/route_access/` 의 표에서 만들어집니다. 직접 고치지 말고 표를 고친 뒤
> `NABIMAN_WRITE_ROUTE_DOC=1 cargo test route_access` 로 다시 만드세요.
> 표와 이 문서가 다르거나, 표에 없는 경로가 등록되어 있으면 테스트가 실패합니다.

NabiMan 의 API 는 허용 목록 방식입니다. 등록된 모든 경로가 아래 표에 등급과 함께 올라 있고,
표에 없는 경로는 최고관리자 말고는 아무도 부를 수 없습니다. 새 기능을 만들면 표에 올리기 전까지
운영자·조회자·원격 회원에게 닫혀 있습니다.

- **최고관리자**(활성 admin)는 이 표로 판단하지 않습니다. 다만 `/api/backup`, `/api/audit`, `/api/apikeys`
  는 admin 전용이고, `/api/diagnostics/support-report` 는 admin 의 POST 만 됩니다.
- **범위가 정해진 계정**(소속·tenant 로 제한된 운영자·조회자, 새로 만든 계정 모두)은 `scope_access` 의
  별도 허용 목록을 따릅니다. 아래 운영자·조회자 열은 범위 제한이 없는 (이전부터 있던) 계정의 권한입니다.
- **API 키**(`nbm_…`)는 키에 준 범위의 GET 만 됩니다 (`api_keys`).
- 처리기 안에서 한 번 더 확인하는 곳(소유자, 2단계 인증, 최고관리자 확인 등)은 이 표보다 좁을 수 있습니다.

## 등급

| 등급 | 운영자(operator) | 조회자(viewer) |
|---|---|---|
| 공개 | 로그인 없이 (처리기가 비밀번호·공유 비밀을 확인) | 같음 |
| 관리자 전용 | — | — |
| 조회 | GET | GET |
| 운영 | 모든 메서드 | GET |
| 계정 | 모든 메서드 (자기 로그인·비밀번호·2단계 인증) | 모든 메서드 |

원격 회원(remote) 열: `—` 는 닫힘, 메서드 목록은 그 메서드만, "본인 것만" 은 처리기가 회원 본인의
세션·콘솔인지 확인, "서버 규칙" 은 `member_servers` 가 경로의 서버가 회원 본인 것이거나 배정된
것인지(쓰기는 허용된 경우만) 확인합니다. 관리자가 끈 기능(받아쓰기, 서버 등록, 서버 간 복사)은
목록에 있어도 막힙니다.

## 요약

- 등록된 API 경로: 334개
- 공개 31 · 관리자 전용 97 · 조회 52 · 운영 148 · 계정 6
- 원격 회원이 닿는 경로: 63개 (메서드 지정 25 · 본인 것만 17 · 서버 규칙 21)

## 검토가 필요해 보이는 권한 (바꾸지 않고 그대로 옮김)

범위 제한이 없는 운영자 계정 기준입니다. 새로 만드는 운영자는 범위 제한 계정이라 해당하지 않습니다.

- `POST /api/updates/upgrade`, `/api/updates/upgrade-package`: 운영자가 시스템 전체 업그레이드를 할 수 있습니다.
  처리기에 별도 확인이 없고, 반면 `/api/packages/install`·`remove` 는 운영자에게 막혀 있어 서로 맞지 않습니다.
- `POST /api/mec/v1/users/{id}/reset-password`, tenant 생성·가져오기·삭제, 노드 cordon·taint 등 MEC 쓰기:
  운영자에게 열려 있고, 일부 처리기는 확인 헤더 말고는 역할을 보지 않습니다.
- `/api/firewall/add`·`delete`, `/api/ipblock/ban`·`unban`, `/api/services/action`, `/api/sessions/kill`,
  `/api/ssl/issue`·`renew`, `/api/license/activate`, `/api/notifications/*`(외부 발송 채널), `/api/alerts/rules*`:
  운영자가 바꿀 수 있습니다. 운영 업무로 의도한 것인지 확인이 필요합니다.
- 조회자도 읽을 수 있는 것: `/api/logs`(시스템 저널), `/api/cron`(root crontab), `/api/users`(전체 계정 목록),
  `/api/notifications/channels`, `/api/database/backups`.

## 경로 표

| 경로 | 등급 | 운영자 | 조회자 | 원격 회원 | 비고 |
|---|---|---|---|---|---|
| `/api/accounts` | 조회 | GET | GET | — |  |
| `/api/accounts/options` | 조회 | GET | GET | — |  |
| `/api/accounts/password` | 조회 | GET | GET | — |  |
| `/api/alerts/rules` | 운영 | 모든 메서드 | GET | — |  |
| `/api/alerts/rules/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/apikeys` | 관리자 전용 | — | — | — |  |
| `/api/apikeys/{id}` | 관리자 전용 | — | — | — |  |
| `/api/app-settings` | 운영 | 모든 메서드 | GET | GET |  |
| `/api/audit` | 관리자 전용 | — | — | — |  |
| `/api/audit/clear` | 관리자 전용 | — | — | — |  |
| `/api/audit/export` | 관리자 전용 | — | — | — |  |
| `/api/audit/forwarding` | 관리자 전용 | — | — | — |  |
| `/api/audit/forwarding/{id}/test` | 관리자 전용 | — | — | — |  |
| `/api/audit/forwarding/{id}/{action}` | 관리자 전용 | — | — | — |  |
| `/api/audit/health` | 관리자 전용 | — | — | — |  |
| `/api/audit/verify` | 관리자 전용 | — | — | — |  |
| `/api/auth/2fa/disable` | 계정 | 모든 메서드 | 모든 메서드 | POST |  |
| `/api/auth/2fa/recovery-codes` | 운영 | 모든 메서드 | GET | — | POST 는 모든 계정 |
| `/api/auth/2fa/setup` | 계정 | 모든 메서드 | 모든 메서드 | POST |  |
| `/api/auth/2fa/status` | 운영 | 모든 메서드 | GET | — | GET 는 모든 계정 |
| `/api/auth/2fa/verify` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/auth/2fa/verify-setup` | 계정 | 모든 메서드 | 모든 메서드 | POST |  |
| `/api/auth/activity` | 계정 | 모든 메서드 | 모든 메서드 | POST |  |
| `/api/auth/change-password` | 계정 | 모든 메서드 | 모든 메서드 | POST |  |
| `/api/auth/ldap/config` | 조회 | GET | GET | — |  |
| `/api/auth/ldap/login` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/auth/ldap/provider` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/auth/ldap/test` | 조회 | GET | GET | — |  |
| `/api/auth/login` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/auth/logout` | 계정 | 모든 메서드 | 모든 메서드 | POST |  |
| `/api/auth/oauth/authorize` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/auth/oauth/callback` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/auth/oauth/config` | 조회 | GET | GET | — |  |
| `/api/auth/oauth/provider` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/auth/refresh` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/auth/register` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/auth/sessions` | 조회 | GET | GET | — | GET 는 모든 계정 |
| `/api/auth/sessions/revoke` | 조회 | GET | GET | — | POST 는 모든 계정 |
| `/api/auth/sessions/revoke-all` | 조회 | GET | GET | — | POST 는 모든 계정 |
| `/api/auth/sessions/revoke-others` | 조회 | GET | GET | — | POST 는 모든 계정 |
| `/api/backup` | 관리자 전용 | — | — | — |  |
| `/api/backup/create` | 관리자 전용 | — | — | — |  |
| `/api/backup/create-compressed` | 관리자 전용 | — | — | — |  |
| `/api/backup/create-encrypted` | 관리자 전용 | — | — | — |  |
| `/api/backup/delete` | 관리자 전용 | — | — | — |  |
| `/api/backup/restore` | 관리자 전용 | — | — | — |  |
| `/api/backup/restore-encrypted` | 관리자 전용 | — | — | — |  |
| `/api/backup/schedules` | 관리자 전용 | — | — | — |  |
| `/api/backup/schedules/{id}` | 관리자 전용 | — | — | — |  |
| `/api/backup/schedules/{id}/run` | 관리자 전용 | — | — | — |  |
| `/api/config/services` | 조회 | GET | GET | — |  |
| `/api/config/{service_id}` | 관리자 전용 | — | — | — |  |
| `/api/config/{service_id}/diff` | 관리자 전용 | — | — | — |  |
| `/api/config/{service_id}/restart` | 관리자 전용 | — | — | — |  |
| `/api/config/{service_id}/rollback` | 관리자 전용 | — | — | — |  |
| `/api/config/{service_id}/validate` | 관리자 전용 | — | — | — |  |
| `/api/config/{service_id}/versions` | 관리자 전용 | — | — | — |  |
| `/api/containers` | 운영 | 모든 메서드 | GET | — |  |
| `/api/containers/available` | 운영 | 모든 메서드 | GET | — |  |
| `/api/containers/images` | 운영 | 모든 메서드 | GET | — |  |
| `/api/containers/images/remove` | 운영 | 모든 메서드 | GET | — |  |
| `/api/containers/logs` | 운영 | 모든 메서드 | GET | — |  |
| `/api/containers/remove` | 운영 | 모든 메서드 | GET | — |  |
| `/api/containers/restart` | 운영 | 모든 메서드 | GET | — |  |
| `/api/containers/start` | 운영 | 모든 메서드 | GET | — |  |
| `/api/containers/stop` | 운영 | 모든 메서드 | GET | — |  |
| `/api/cron` | 조회 | GET | GET | — |  |
| `/api/cron/add` | 조회 | GET | GET | — |  |
| `/api/cron/delete` | 조회 | GET | GET | — |  |
| `/api/cron/snapshot` | 조회 | GET | GET | — |  |
| `/api/cron/update` | 조회 | GET | GET | — |  |
| `/api/cshare/{token}` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/cshare/{token}/dictation` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/cshare/{token}/ws` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/database/available` | 조회 | GET | GET | — |  |
| `/api/database/backup` | 조회 | GET | GET | — |  |
| `/api/database/backups` | 조회 | GET | GET | — |  |
| `/api/database/query` | 조회 | GET | GET | — |  |
| `/api/database/restore` | 조회 | GET | GET | — |  |
| `/api/database/status` | 조회 | GET | GET | — |  |
| `/api/diagnostics/nslookup` | 운영 | 모든 메서드 | GET | — |  |
| `/api/diagnostics/ping` | 운영 | 모든 메서드 | GET | — |  |
| `/api/diagnostics/port-check` | 운영 | 모든 메서드 | GET | — |  |
| `/api/diagnostics/runtime` | 운영 | 모든 메서드 | GET | — |  |
| `/api/diagnostics/support-report` | 관리자 전용 | — | — | — |  |
| `/api/diagnostics/traceroute` | 운영 | 모든 메서드 | GET | — |  |
| `/api/disks/status` | 운영 | 모든 메서드 | GET | — |  |
| `/api/dns/available` | 운영 | 모든 메서드 | GET | — |  |
| `/api/dns/status` | 운영 | 모든 메서드 | GET | — |  |
| `/api/files/browse` | 관리자 전용 | — | — | — |  |
| `/api/files/chmod` | 관리자 전용 | — | — | — |  |
| `/api/files/delete` | 관리자 전용 | — | — | — |  |
| `/api/files/download` | 관리자 전용 | — | — | — |  |
| `/api/files/mkdir` | 관리자 전용 | — | — | — |  |
| `/api/files/read` | 관리자 전용 | — | — | — |  |
| `/api/files/upload` | 관리자 전용 | — | — | — |  |
| `/api/files/write` | 관리자 전용 | — | — | — |  |
| `/api/firewall/add` | 운영 | 모든 메서드 | GET | — |  |
| `/api/firewall/delete` | 운영 | 모든 메서드 | GET | — |  |
| `/api/firewall/status` | 운영 | 모든 메서드 | GET | — |  |
| `/api/ipblock/ban` | 운영 | 모든 메서드 | GET | — |  |
| `/api/ipblock/banned` | 운영 | 모든 메서드 | GET | — |  |
| `/api/ipblock/log` | 운영 | 모든 메서드 | GET | — |  |
| `/api/ipblock/status` | 운영 | 모든 메서드 | GET | — |  |
| `/api/ipblock/unban` | 운영 | 모든 메서드 | GET | — |  |
| `/api/license` | 운영 | 모든 메서드 | GET | — |  |
| `/api/license/activate` | 운영 | 모든 메서드 | GET | — |  |
| `/api/license/features` | 운영 | 모든 메서드 | GET | — |  |
| `/api/logs` | 운영 | 모든 메서드 | GET | — |  |
| `/api/logs/units` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mail/available` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mail/status` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/audit/logs` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/audit/logs/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/dashboard/activity` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/dashboard/live` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/dashboard/live/stream` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/dashboard/summary` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/docs/downloads/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/docs/generate` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/docs/generated` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/docs/templates` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/firewall/nat-rules` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/firewall/nat-rules/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/firewall/public-ips` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/firewall/security-policies` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/firewall/sync` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/gpu/allocations` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/gpu/overview` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/gpu/usage` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/health/summary` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/jobs` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/jobs/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/jobs/{id}/recovery` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/jobs/{id}/stream` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/network/ingresses` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/network/ingresses/{namespace}/{name}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/network/lb-pools` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/network/services` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/nodes` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/nodes/{name}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/nodes/{name}/cordon` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/nodes/{name}/gpu-mode` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/nodes/{name}/labels` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/nodes/{name}/pods` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/nodes/{name}/taints` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/nodes/{name}/uncordon` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/settings/snapshot` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/storage/harbor/projects` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/storage/harbor/projects/{project}/images` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/storage/overview` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/storage/pvcs` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/discover` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/access-delegation` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/delegated-access-context` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/delete-preflight` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/guide.docx` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/guide.txt` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/import` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/managed-memberships` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/managed-memberships/{operation_id}/revoke` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/members` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/members/{binding_id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/members/{user_id}/access-plan` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/membership-operations` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/membership-operations/{operation_id}/associate` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/membership-operations/{operation_id}/reconcile` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/pending-managed-grants` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/quota-usage` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/resources` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/scoped-managed-memberships` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/tenants/{id}/starter-kit` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/users` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/users/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/mec/v1/users/{id}/reset-password` | 운영 | 모든 메서드 | GET | — |  |
| `/api/network/status` | 운영 | 모든 메서드 | GET | — |  |
| `/api/notifications/channels` | 운영 | 모든 메서드 | GET | — |  |
| `/api/notifications/channels/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/notifications/history` | 운영 | 모든 메서드 | GET | — |  |
| `/api/notifications/templates` | 운영 | 모든 메서드 | GET | — |  |
| `/api/notifications/templates/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/notifications/test` | 운영 | 모든 메서드 | GET | — |  |
| `/api/openapi.json` | 운영 | 모든 메서드 | GET | — |  |
| `/api/orchestration/guide` | 운영 | 모든 메서드 | GET | — |  |
| `/api/packages/install` | 조회 | GET | GET | — |  |
| `/api/packages/installed` | 조회 | GET | GET | — |  |
| `/api/packages/remove` | 조회 | GET | GET | — |  |
| `/api/packages/search` | 운영 | 모든 메서드 | GET | — |  |
| `/api/preview/office` | 운영 | 모든 메서드 | GET | — |  |
| `/api/processes` | 운영 | 모든 메서드 | GET | — |  |
| `/api/processes/inspect/{pid}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/processes/kill` | 운영 | 모든 메서드 | GET | — |  |
| `/api/processes/options` | 운영 | 모든 메서드 | GET | — |  |
| `/api/remote-servers` | 조회 | GET | GET | GET, POST |  |
| `/api/remote-servers/check-all` | 조회 | GET | GET | — |  |
| `/api/remote-servers/facts` | 조회 | GET | GET | — |  |
| `/api/remote-servers/ssh-ca` | 조회 | GET | GET | — |  |
| `/api/remote-servers/ssh-key` | 조회 | GET | GET | — |  |
| `/api/remote-servers/ssh-key/generate` | 조회 | GET | GET | — |  |
| `/api/remote-servers/{id}` | 조회 | GET | GET | 서버 규칙 (본인·배정 서버) |  |
| `/api/remote-servers/{id}/attacks` | 조회 | GET | GET | — |  |
| `/api/remote-servers/{id}/check` | 조회 | GET | GET | 서버 규칙 (본인·배정 서버) |  |
| `/api/remote-servers/{id}/deploy-key` | 조회 | GET | GET | — |  |
| `/api/remote-servers/{id}/detect` | 조회 | GET | GET | 서버 규칙 (본인·배정 서버) |  |
| `/api/remote-servers/{id}/exec` | 조회 | GET | GET | — |  |
| `/api/remote-servers/{id}/host-key` | 조회 | GET | GET | 서버 규칙 (본인·배정 서버) |  |
| `/api/remote-servers/{id}/host-trust` | 조회 | GET | GET | 서버 규칙 (본인·배정 서버) |  |
| `/api/remote-servers/{id}/test-key` | 조회 | GET | GET | — |  |
| `/api/remote/console` | 운영 | 모든 메서드 | GET | GET |  |
| `/api/remote/console-creds` | 운영 | 모든 메서드 | GET | — |  |
| `/api/remote/consoles` | 운영 | 모든 메서드 | GET | GET, POST |  |
| `/api/remote/consoles/{id}` | 운영 | 모든 메서드 | GET | 본인 것만 (처리기에서 확인) |  |
| `/api/remote/consoles/{key}/access` | 운영 | 모든 메서드 | GET | 본인 것만 (처리기에서 확인) |  |
| `/api/remote/consoles/{key}/shares` | 운영 | 모든 메서드 | GET | 본인 것만 (처리기에서 확인) |  |
| `/api/remote/consoles/{key}/shares/{token}` | 운영 | 모든 메서드 | GET | 본인 것만 (처리기에서 확인) |  |
| `/api/remote/consoles/{target}/drive` | 운영 | 모든 메서드 | GET | 본인 것만 (처리기에서 확인) |  |
| `/api/remote/consoles/{target}/drive/{name}` | 운영 | 모든 메서드 | GET | 본인 것만 (처리기에서 확인) |  |
| `/api/remote/rdp` | 운영 | 모든 메서드 | GET | — |  |
| `/api/remote/serial` | 운영 | 모든 메서드 | GET | — |  |
| `/api/scope-settings/{kind}/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/scope-settings/{kind}/{id}/history` | 운영 | 모든 메서드 | GET | — |  |
| `/api/server/history` | 운영 | 모든 메서드 | GET | — |  |
| `/api/server/status` | 운영 | 모든 메서드 | GET | — |  |
| `/api/services` | 운영 | 모든 메서드 | GET | — |  |
| `/api/services/action` | 운영 | 모든 메서드 | GET | — |  |
| `/api/services/operations/{id}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/sessions` | 운영 | 모든 메서드 | GET | — |  |
| `/api/sessions/kill` | 운영 | 모든 메서드 | GET | — |  |
| `/api/sessions/last` | 운영 | 모든 메서드 | GET | — |  |
| `/api/share/{token}` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/auth` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/dictation` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/attrs` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/checksum` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/delete` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/download` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/download-folder` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/link` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/list` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/mkdir` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/rename` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/stream` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/upload` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/upload-abort` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/upload-finish` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/upload-part` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/files/usage` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/share/{token}/ws` | 공개 | (로그인 없이) | (로그인 없이) | — |  |
| `/api/ssl/certificates` | 운영 | 모든 메서드 | GET | — |  |
| `/api/ssl/issue` | 운영 | 모든 메서드 | GET | — |  |
| `/api/ssl/renew` | 운영 | 모든 메서드 | GET | — |  |
| `/api/ssl/renew/{domain}` | 운영 | 모든 메서드 | GET | — |  |
| `/api/swap/status` | 운영 | 모든 메서드 | GET | — |  |
| `/api/terminal` | 관리자 전용 | — | — | GET |  |
| `/api/terminal/claude-events` | 관리자 전용 | — | — | — |  |
| `/api/terminal/claude-lead` | 관리자 전용 | — | — | — |  |
| `/api/terminal/claude-remote` | 관리자 전용 | — | — | — |  |
| `/api/terminal/claude/chats` | 관리자 전용 | — | — | — |  |
| `/api/terminal/claude/chats/{id}` | 관리자 전용 | — | — | — |  |
| `/api/terminal/claude/chats/{id}/ask` | 관리자 전용 | — | — | — |  |
| `/api/terminal/claude/chats/{id}/stop` | 관리자 전용 | — | — | — |  |
| `/api/terminal/claude/settings` | 관리자 전용 | — | — | — |  |
| `/api/terminal/claude/speak` | 관리자 전용 | — | — | — |  |
| `/api/terminal/claude/test` | 관리자 전용 | — | — | — |  |
| `/api/terminal/copies` | 관리자 전용 | — | — | GET, POST |  |
| `/api/terminal/copies/{job}/cancel` | 관리자 전용 | — | — | POST |  |
| `/api/terminal/dictation` | 관리자 전용 | — | — | GET |  |
| `/api/terminal/dictation/settings` | 관리자 전용 | — | — | GET |  |
| `/api/terminal/dictation/speak` | 관리자 전용 | — | — | POST |  |
| `/api/terminal/dictation/test` | 관리자 전용 | — | — | — |  |
| `/api/terminal/dictation/voiceprint` | 관리자 전용 | — | — | GET |  |
| `/api/terminal/files/{id}/share-link` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/paste-file` | 관리자 전용 | — | — | — |  |
| `/api/terminal/recording/settings` | 관리자 전용 | — | — | — |  |
| `/api/terminal/recordings` | 관리자 전용 | — | — | — |  |
| `/api/terminal/recordings/{sid}` | 관리자 전용 | — | — | — |  |
| `/api/terminal/serial` | 관리자 전용 | — | — | — |  |
| `/api/terminal/session-order` | 관리자 전용 | — | — | GET, PUT |  |
| `/api/terminal/sessions` | 관리자 전용 | — | — | GET |  |
| `/api/terminal/sessions/{id}` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/sessions/{id}/claude` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/sessions/{id}/close` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/sessions/{id}/keepalive` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/sessions/{id}/name` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/sessions/{id}/scrollback` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/sessions/{id}/serial` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/sessions/{id}/settings` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/sessions/{id}/share` | 관리자 전용 | — | — | — |  |
| `/api/terminal/sessions/{id}/share-link` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/sessions/{id}/status` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/sessions/{id}/timeout` | 관리자 전용 | — | — | 본인 것만 (처리기에서 확인) |  |
| `/api/terminal/settings` | 관리자 전용 | — | — | GET, PUT |  |
| `/api/terminal/sftp/columns` | 관리자 전용 | — | — | GET, PUT |  |
| `/api/terminal/sftp/{id}/attrs` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/checksum` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/delete` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/download` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/download-folder` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/link` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/list` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/mkdir` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/rename` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/stream` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/upload` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/upload-abort` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/upload-finish` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/upload-part` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/sftp/{id}/usage` | 관리자 전용 | — | — | 서버 규칙 (본인·배정 서버) |  |
| `/api/terminal/share-links` | 관리자 전용 | — | — | GET |  |
| `/api/terminal/share-links/{token}` | 관리자 전용 | — | — | DELETE |  |
| `/api/terminal/start-options` | 관리자 전용 | — | — | — |  |
| `/api/terminal/tickets` | 관리자 전용 | — | — | POST |  |
| `/api/terminal/topology` | 관리자 전용 | — | — | — |  |
| `/api/traffic/current` | 운영 | 모든 메서드 | GET | — |  |
| `/api/traffic/summary` | 운영 | 모든 메서드 | GET | — |  |
| `/api/updates/check` | 운영 | 모든 메서드 | GET | — |  |
| `/api/updates/upgrade` | 운영 | 모든 메서드 | GET | — |  |
| `/api/updates/upgrade-package` | 운영 | 모든 메서드 | GET | — |  |
| `/api/users` | 조회 | GET | GET | — |  |
| `/api/users/me` | 조회 | GET | GET | GET |  |
| `/api/users/memberships` | 조회 | GET | GET | — |  |
| `/api/users/my-address` | 조회 | GET | GET | — |  |
| `/api/users/{id}` | 조회 | GET | GET | — |  |
| `/api/users/{id}/external-identities/rancher` | 조회 | GET | GET | — |  |
| `/api/users/{id}/overview` | 조회 | GET | GET | — |  |
| `/api/users/{id}/review` | 조회 | GET | GET | — |  |
| `/api/users/{id}/share-links/revoke` | 조회 | GET | GET | — |  |
| `/api/users/{id}/sign-out` | 조회 | GET | GET | — |  |
| `/api/vhost` | 조회 | GET | GET | — |  |
| `/api/vhost/config` | 관리자 전용 | — | — | — |  |
| `/api/vhost/config/{domain}` | 관리자 전용 | — | — | — |  |
| `/api/vhost/delete` | 조회 | GET | GET | — |  |
