# 07. 연동 사전 준비 (Integration Prerequisites)

> NabiMan 서버에서 MEC 클러스터를 실제 연동(Real Mode)하기 위해 필요한 네트워크 경로와 자격증명을 정리합니다.

---

## 0. 환경 개요

```
┌────────────────────────────┐                ┌──────────────────────────────┐
│  NabiMan 서버              │                │  JCIA 5G MEC 클러스터        │
│  (외부)                    │ ──── ? ───►   │  (전남 나주 내부망)          │
│                            │                │                              │
│  Public IP: 115.68.193.237 │                │  내부망: 172.20.26.0/24      │
└────────────────────────────┘                └──────────────────────────────┘
```

연동하려면 **(1) 네트워크 경로** + **(2) 자격증명 4종 세트** 가 필요합니다.

---

## 1. 네트워크 연결

| 대상 | 접근 프로토콜 | 현재 상태 확인 필요 |
|------|------------|---------------------|
| **Kubernetes API** | HTTPS 6443 → 172.20.26.239 (VIP) | VPN 연결 또는 AXGATE NAT로 외부 노출? |
| **Rancher** | HTTPS 443 → rancher.jcia.mec.local | DNS 해석 가능한지? (121.147.13.233 NAT 규칙 존재 확인) |
| **AXGATE 방화벽** | SSH 2222 → 121.147.13.228 | 외부 접근 가능. 관리자 IP 제한 있는지? |
| **Harbor** | HTTPS 443 → 172.20.26.234 | NAT 규칙 필요 |

### 가장 먼저 확인할 것

NabiMan 서버(115.68.193.237)에서 MEC 내부망에 도달 가능한지 확인:

```bash
# Kubernetes API
curl -k https://172.20.26.239:6443

# Rancher (DNS 해석 확인)
nslookup rancher.jcia.mec.local
curl -k https://rancher.jcia.mec.local

# AXGATE SSH
nc -zv 121.147.13.228 2222

# Harbor
curl -k https://172.20.26.234
```

→ 안 되면 **VPN 연결** 또는 **AXGATE NAT 설정**부터 필요합니다.

---

## 2. 자격증명 (환경변수로 주입)

### 2.1 Kubernetes

| 항목 | 값 |
|------|-----|
| **kubeconfig 파일** | `~/.kube/config` (operator-kit 운영 서버에서 복사) 또는 Rancher에서 다운로드 |
| **대안** | ServiceAccount Token + CA cert (권장, 장기 사용 시) |

**필요 권한**: `cluster-admin` 또는 아래 ClusterRole

```yaml
# nabiman-mec-admin ClusterRole
rules:
- apiGroups: ["", "apps", "networking.k8s.io", "rbac.authorization.k8s.io"]
  resources: ["*"]
  verbs: ["*"]
- apiGroups: ["management.cattle.io"]   # Rancher CRD
  resources: ["projects", "users", "projectroletemplatebindings"]
  verbs: ["*"]
```

테넌트 생성, 노드 Taint, GPU 모드 전환 작업에 필요합니다.

### 2.2 Rancher

| 항목 | 값 |
|------|-----|
| **API Base URL** | `https://rancher.jcia.mec.local` |
| **API Token** | `token-xxxxx:xxxxxxxxxxxxx` 형식 (Rancher UI → API & Keys 에서 생성) |
| **Cluster ID** | 보통 `local` 또는 `c-xxxxx` |
| **인증서** | 자가서명 인증서면 `INSECURE=true` 플래그 |

### 2.3 AXGATE 방화벽

| 항목 | 값 |
|------|-----|
| **Host** | `121.147.13.228` |
| **Port** | `2222` |
| **Username / Password** | CLI 접근 권한 (`show running-config` / `configure terminal` 가능해야 함) |

### 2.4 Harbor

| 항목 | 값 |
|------|-----|
| **Base URL** | `https://harbor.jcia.mec.local` |
| **Username / Password** | 기존 운영 가이드 기준 `admin / Jcia12345!@#` |

---

## 3. NabiMan 서버 반영

받은 정보로 systemd 서비스에 환경변수 추가 + 재시작:

```ini
# /etc/systemd/system/nabiman.service.d/mec.conf
[Service]
Environment=NABIMAN_MEC_MODE=real
Environment=NABIMAN_MEC_KUBECONFIG=/etc/nabiman/kubeconfig

Environment=NABIMAN_MEC_RANCHER_URL=https://rancher.jcia.mec.local
Environment=NABIMAN_MEC_RANCHER_TOKEN=token-xxxxx:xxxxxxxxxxxxx
Environment=NABIMAN_MEC_RANCHER_CLUSTER=local
Environment=NABIMAN_MEC_RANCHER_INSECURE=true

Environment=NABIMAN_MEC_AXGATE_HOST=121.147.13.228
Environment=NABIMAN_MEC_AXGATE_PORT=2222
Environment=NABIMAN_MEC_AXGATE_USER=admin
Environment=NABIMAN_MEC_AXGATE_PASSWORD=***

Environment=NABIMAN_MEC_HARBOR_URL=https://harbor.jcia.mec.local
Environment=NABIMAN_MEC_HARBOR_USER=admin
Environment=NABIMAN_MEC_HARBOR_PASSWORD=***
Environment=NABIMAN_MEC_HARBOR_INSECURE=true
```

적용:

```bash
sudo systemctl daemon-reload
sudo systemctl restart nabiman
```

### 비밀값 저장 권장사항

- 파일 권한: `root:root` 0600
- 가능하면 **age/sops 암호화**로 config manager 탭에 넣어둘 것

---

## 4. 정보 전달 시 안전한 방법

비밀번호/토큰은 채팅창/이메일 대신 다음 방식으로 전달:

| 방법 | 절차 |
|------|------|
| **SSH 직접 입력** | `ssh nabiman-server` → `sudo nano /etc/systemd/system/nabiman.service.d/mec.conf` 에 직접 붙여넣기 |
| **scp 파일 전송** | `scp ~/.kube/config nabiman-server:/etc/nabiman/kubeconfig` (root:root, 0600 권한으로 복사) |

---

## 5. 부분 연동도 가능 (Auto Mode)

4종 모두 한 번에 필요한 건 아닙니다. `NABIMAN_MEC_MODE=auto` 이면:

- 환경변수 있는 것만 **Real** 사용
- 없는 것은 **Mock** 동작

### 권장 진행 순서

```
1. AXGATE만 먼저 연동 → NAT 규칙 실제 동작 시범
2. Harbor 추가
3. Rancher 추가
4. Kubernetes 마지막 (가장 강력한 권한 필요)
```

> AXGATE가 외부 접근(2222 포트) 가능해서 가장 쉽습니다.

---

## 6. 가장 먼저 알려줄 정보

연동 시작을 위해 다음 두 가지 답변이 필요합니다:

### Q1. 네트워크 경로

NabiMan 서버(115.68.193.237)에서 MEC 내부망에 접근하는 경로는?

- [ ] VPN 연결 (예: AXGATE SSL VPN 클라이언트가 서버에 설치됨)
- [ ] AXGATE NAT (특정 공인 IP를 NabiMan 전용으로 매핑)
- [ ] 사내 Bastion 경유 (SSH 터널)
- [ ] 기타: ___________

### Q2. 시작할 연동 대상

위 4개 중 어떤 것부터 연동을 시작할지?

- [ ] **AXGATE 방화벽** (권장 - 외부 접근 가능, 가장 쉬움)
- [ ] Harbor
- [ ] Rancher
- [ ] Kubernetes

---

## 7. 첫 연동 단계 (예시: AXGATE)

위 두 답변을 받으면 다음과 같은 흐름으로 진행됩니다:

```
1. NabiMan 서버에서 121.147.13.228:2222 접근 테스트
2. 자격증명 환경변수 등록 (mec.conf)
3. NabiMan 재시작
4. /api/mec/v1/firewall/nat-rules 호출 테스트
5. UI에서 NAT 규칙 조회/추가 시범
```

성공하면 다음 대상(Harbor → Rancher → Kubernetes) 순으로 확장.

---

## 8. 보안 체크리스트

- [ ] `mec.conf` 파일 권한 0600 (root 전용 읽기)
- [ ] kubeconfig 파일 권한 0600
- [ ] 환경변수 출력 시 마스킹 (`Environment=*PASSWORD=...` 검열)
- [ ] Audit Log에 비밀번호/토큰 저장 금지
- [ ] AXGATE 비밀번호 정기 교체 (현재 `The5GMEC2604!@`, `260420` 변경됨)
- [ ] Rancher API Token에 만료일 설정 (예: 90일)
- [ ] NabiMan에 접근하는 관리자 계정도 별도 보호 (MFA 권장)
