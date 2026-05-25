# 07. 운영자 런북 (Operator Runbook)

> **대상**: JCIA 5G MEC 테스트베드 운영자. 이 문서는 NabiMan MEC Management 모듈을 운영 클러스터와 처음 연결할 때와 장애 상황에서 사용합니다.

## 1. 사전 점검

### 1.1 네트워크 도달성

NabiMan 서버(`115.68.193.237`)에서 MEC 내부망으로의 경로가 확보되어야 합니다. 다음 명령으로 도달성을 확인합니다.

```bash
# NabiMan 서버에서 실행
curl -k --connect-timeout 5 https://172.20.26.239:6443/version  # K8s API
curl -k --connect-timeout 5 https://rancher.jcia.mec.local/v3   # Rancher
nc -zv 121.147.13.228 2222                                      # AXGATE SSH
curl -k --connect-timeout 5 https://172.20.26.234/api/v2.0/health # Harbor
```

4개 중 하나라도 실패하면 **VPN 또는 AXGATE NAT 설정**이 선행되어야 합니다.

### 1.2 자격증명 준비

| 시스템 | 필요한 정보 | 비고 |
|-------|-----------|------|
| Kubernetes | kubeconfig 파일 (cluster-admin) | `/etc/nabiman/kubeconfig` 에 복사, `root:root 0600` |
| Rancher | API Token (Rancher UI → API & Keys 생성) | `token-xxxxx:xxxxxxxxxx` 형식 |
| AXGATE | username / password | CLI 접근 권한 필요 (show, configure) |
| Harbor | username / password | admin 계정 또는 프로젝트 관리자 |

## 2. 첫 연결 (Read-only 모드 권장)

### 2.1 systemd drop-in 작성

**MEC 설정** 탭에서 "복사" 버튼으로 템플릿을 복사한 뒤, 서버에서 실행:

```bash
sudo mkdir -p /etc/systemd/system/nabiman.service.d
sudo tee /etc/systemd/system/nabiman.service.d/mec.conf > /dev/null <<'EOF'
[Service]
Environment=NABIMAN_MEC_READ_ONLY=true
Environment=NABIMAN_MEC_MODE=real
Environment=NABIMAN_MEC_KUBECONFIG=/etc/nabiman/kubeconfig
Environment=NABIMAN_MEC_RANCHER_URL=https://rancher.jcia.mec.local
Environment=NABIMAN_MEC_RANCHER_TOKEN=token-xxxxx:xxxxxxxxxx
Environment=NABIMAN_MEC_RANCHER_CLUSTER=local
Environment=NABIMAN_MEC_RANCHER_INSECURE=true
Environment=NABIMAN_MEC_AXGATE_HOST=121.147.13.228
Environment=NABIMAN_MEC_AXGATE_PORT=2222
Environment=NABIMAN_MEC_AXGATE_USER=admin
Environment=NABIMAN_MEC_AXGATE_PASSWORD=***REPLACE***
Environment=NABIMAN_MEC_HARBOR_URL=https://harbor.jcia.mec.local
Environment=NABIMAN_MEC_HARBOR_USER=admin
Environment=NABIMAN_MEC_HARBOR_PASSWORD=***REPLACE***
Environment=NABIMAN_MEC_HARBOR_INSECURE=true
EOF
sudo chmod 600 /etc/systemd/system/nabiman.service.d/mec.conf
sudo systemctl daemon-reload
sudo systemctl restart nabiman
```

**핵심**: `NABIMAN_MEC_READ_ONLY=true` 는 모든 변경 작업을 차단합니다. 반드시 이 상태로 첫 연결을 진행하세요.

### 2.2 Health 검증

브라우저에서 https://nabiman.xos.kr → **MEC → Health** 탭:

- 4개 시스템이 모두 녹색 `connected` 이어야 정상
- 회색/빨강이면 **MEC 설정** 탭의 `✓/✗` 배지로 누락된 자격증명을 확인
- `latency_ms` 가 3000ms 이상이면 네트워크 경로를 점검

### 2.3 기존 테넌트 Discovery

**MEC → Discovery** 탭:

- 클러스터의 `*-poc` 네임스페이스가 목록에 표시 (5개 실증기업)
- 각 항목의 **Import** 버튼으로 NabiMan DB에 관리 대상으로 등록
- Import 는 기존 K8s/Rancher 리소스를 **재생성하지 않고 참조만** 합니다

### 2.4 조회 시험

Read-only 모드에서 다음이 모두 정상 동작해야 합니다:

- **테넌트** 탭: Import된 5개가 표시, "상세" 클릭 시 Pods/Services/Quota 조회
- **노드** 탭: mec-cp01..mec-wn04 (7개), GPU 모델/Taint/Tenant 매핑
- **GPU 자원** 탭: T4/A40/GH200 Slots 사용률
- **방화벽** 탭: AXGATE NAT 규칙 목록 (Rancher HTTPS, Bastion SSH, RDP, 3306 등)
- **스토리지** 탭: PVC 목록, Harbor 프로젝트

위 조회가 모두 정상이고 MEC 운영 활동과 일치하면 실제 연결이 성공한 것입니다.

## 3. Read-only → 쓰기 모드 전환

충분히 검증된 후에만 전환하세요.

```bash
sudo sed -i 's/NABIMAN_MEC_READ_ONLY=true/NABIMAN_MEC_READ_ONLY=false/' \
  /etc/systemd/system/nabiman.service.d/mec.conf
sudo systemctl daemon-reload
sudo systemctl restart nabiman
```

전환 후 **MEC 설정** 탭에서 🔒 배지가 사라진 것을 확인합니다.

## 4. 롤백 (Rollback)

NabiMan에서 의심스러운 변경이 감지되면 다음 순서로 복구:

### 4.1 즉시 차단

```bash
sudo sed -i 's/NABIMAN_MEC_READ_ONLY=false/NABIMAN_MEC_READ_ONLY=true/' \
  /etc/systemd/system/nabiman.service.d/mec.conf
sudo systemctl restart nabiman
```

### 4.2 변경 이력 추적

**MEC → 감사로그** 탭에서 최근 작업 확인:
- `tenant_create / tenant_delete`: 테넌트 관련
- `firewall_nat_add / firewall_nat_delete`: AXGATE NAT
- `node_patch_taints / gpu_mode_switch`: 노드 상태 변경
- `starter_kit_deploy`: Pod 배포

각 로그의 `user`, `status`, `operations` 필드로 누가·언제·무엇을 했는지 파악.

### 4.3 Kubernetes 롤백

```bash
# 삭제된 리소스가 있다면 Velero(또는 etcd backup)에서 복구
velero restore create --from-backup <backup-name> --include-namespaces ygram-poc

# NAT 규칙은 AXGATE CLI로 직접 복구 (기존 operator-kit 스크립트 참고)
ssh -p 2222 admin@121.147.13.228
# AXGATE# configure terminal
# AXGATE# ip nat policy from untrust to trust 12
# ...
```

## 5. 장애 시나리오

### 5.1 Health 탭에서 특정 시스템만 `disconnected`

| 시스템 | 주요 원인 | 대응 |
|-------|---------|------|
| Kubernetes | kubeconfig 만료/경로 오류 | `/etc/nabiman/kubeconfig` 재발급 |
| Rancher | Token 만료 | Rancher UI에서 새 token 생성 |
| AXGATE | SSH 세션 타임아웃 / 계정 10분 잠금 | §5.1.1 참조 |
| Harbor | 자체 인증서 + insecure=false | `NABIMAN_MEC_HARBOR_INSECURE=true` |

### 5.1.1 AXGATE 특수 이슈 (2026-04-21 실측 반영)

**CLI 인증 플로우** (handoff 문서의 "password 두 번 입력" 정확한 의미):

1. SSH 인증 — 첫 번째 password (username/password)
2. SSH 접속 직후 CLI 레벨 `Password: ` 프롬프트 표시 — **같은 password 한 번 더**
3. `enable` 등 escalation 명령 **불필요** (2번째 인증 후 이미 privileged 모드)

**절대 하지 말 것**:
- 2번째 `Password:` 프롬프트에 `enable` 등 password 이외의 텍스트 전송
  → **계정 10분 잠금** (`% You are blocked for 600 seconds!`)
- 잠금 상태에서 반복 시도 → 잠금 연장 가능

NabiMan은 `greet` 응답에서 `blocked for` 감지 시 CLI 인증 시도 없이 즉시 실패
반환하여 잠금 연장을 방지합니다 (`mec/services/axgate_real/session.rs`).

**레거시 SSH 알고리즘**:
- AXGATE: `diffie-hellman-group14-sha1`, `ssh-rsa` (SHA1) 만 지원
- NabiMan: `Preferred::DEFAULT` + 레거시 algorithms 확장 (`legacy_preferred()`)
- OpenSSH client 로 수동 접속 시: `-oKexAlgorithms=+diffie-hellman-group14-sha1
  -oHostKeyAlgorithms=+ssh-rsa -oPubkeyAcceptedAlgorithms=+ssh-rsa`

### 5.2 Rate Limit 발생

**응답**: `429 Too Many Requests, MEC_RATE_LIMIT`

- 분당 30건 변경 / 10건 삭제 초과
- 60초 대기 후 재시도
- 대량 작업이 필요하면 별도 운영자 계정으로 분산

### 5.3 SSE 스트림 끊김

**Jobs** 탭에서 실시간 진행상황이 멈추는 경우:
- 15초마다 heartbeat 전송되므로 정상 연결은 끊기지 않음
- Apache/Nginx 프록시의 `ProxyTimeout` 이 300초 이상인지 확인
- 최종 상태는 Job 상세 조회로 재확인

## 6. 비밀값 관리

**주의**:
- `/etc/systemd/system/nabiman.service.d/mec.conf` 는 `root:root 0600` 이어야 함
- SSH 접근 가능한 관리자 그룹만 `/etc/systemd/system/` 수정 권한 보유
- 토큰/비밀번호 회전은 분기별 실시:
  ```bash
  # Rancher token 회전 시
  sudo sed -i 's|TOKEN=token-.*|TOKEN=token-NEW-XXX|' /etc/systemd/system/nabiman.service.d/mec.conf
  sudo systemctl restart nabiman
  ```
- 감사로그(`audit_logs` 테이블)에는 비밀값이 **저장되지 않음** (SSH password는 request body에서 마스킹 필요 — 향후 개선)

## 7. 확인 헤더 우회 (자동화 전용)

배치 스크립트/CI에서 삭제 작업이 필요하면:

```bash
# 특정 작업만 우회
curl -X DELETE \
  -H "Authorization: Bearer $TOKEN" \
  -H "X-Confirm-Name: ygram-poc" \
  https://nabiman.xos.kr/api/mec/v1/tenants/ygram-poc

# 장기 우회 (비권장)
Environment=NABIMAN_MEC_SKIP_CONFIRM=1
```

**절대로 `NABIMAN_MEC_SKIP_CONFIRM=1` 를 프로덕션 환경변수에 상시 설정하지 마세요.**

## 8. 정기 점검

| 주기 | 점검 항목 |
|-----|----------|
| 매일 | Health 탭 4/4 connected, 감사로그 이상 여부 |
| 주 1회 | Jobs 탭 실패 건 확인, Audit 로그 백업 |
| 월 1회 | Rancher/Harbor 사용자 목록 정리, Rate Limit 이력 확인 |
| 분기 | 크리덴셜 회전 (Rancher token, AXGATE/Harbor password) |

## 9. 에스컬레이션 연락처

장애 발생 시 참조:
- 운영 관리자: (용역사 엔지니어 연락처)
- 자문위원: 김전일 / 이광선
- NabiMan 개발: XOS 개발팀
- 감독 기관: (재)전남정보문화산업진흥원 (월간 보고)

---

**문서 이력**
- 2026-04-21: 초기 작성 (NabiMan MEC Management v1.2.0)
