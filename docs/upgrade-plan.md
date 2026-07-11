# nabiman 전체 솔루션 업그레이드 계획 (상세)

> 작성 2026-06-29. 전체 솔루션 점검 결과를 토대로 수립한 제품 전반의 상세 업그레이드 로드맵.
> MEC 관제(NOC) 단독 계획은 [`mec-management/09-noc-upgrade-plan.md`](mec-management/09-noc-upgrade-plan.md) 참조 — 본 문서는 이를 포함한 **제품 전체** 관점이며 중복은 참조로 갈음한다.

---

## 0. 점검 요약 (2026-06-29 실측)

| 항목 | 결과 | 판정 |
|------|------|------|
| 백엔드 `cargo check` | exit 0, **dead-code 경고 48건** | 🟡 빌드 정상, 잔여물 |
| 백엔드 `cargo test` | 전 바이너리 통과 (142 test fn) | 🟢 |
| 프론트 `tsc --noEmit` | 클린 | 🟢 |
| 프론트 `eslint` | error 0 / **warning 1** (`TerminalPanel.tsx:230` 미사용 `timeoutLabel`) | 🟢 |
| 배포 정적빌드 | `main.82e4ce8a.js` — 소스 빌드와 해시 일치 | 🟢 최신 |
| 라인 규율 (하드 400) | **초과 0건** (주석·공백 제외 코드 라인 기준) | 🟢 |
| 라인 규율 (소프트 250) | 초과 약 25건 (최대 `MecFirewallPanel.tsx` 379) | 🟡 |
| i18n 적용 | 레거시 서버패널 **39/39** 적용 / MEC 패널 **4/27** (NOC만) | 🟡 MEC 미적용 |
| 프론트 테스트 | `App.test.tsx` 1개뿐 | 🔴 커버리지 공백 |
| 규모 | 백엔드 Rust 22.1k LOC / 프론트 TS 15.6k LOC / 패널 85개 | — |

**총평**: 기능·안정성은 양호(빌드·타입·테스트 그린, 배포 최신). 부채는 ① 백엔드 하드리미트 2파일 ② dead-code 48건 ③ MEC 패널 i18n 공백 ④ 프론트 테스트 공백 ⑤ MEC 패널의 디자인시스템 부분 적용. 신규 가치는 ⑥ 실시간 스트리밍(SSE) ⑦ 관측성 심화(GPU/VPN) ⑧ 제품 UX 고도화(명령 팔레트·딥링크·임베디드 콘솔).

## 0.1 구현 현황 (2026-07-11)

| WS | 상태 | 요약 |
|----|------|------|
| WS-C | ✅ 완료 | oauth/ldap 모듈 분해, dead-code 48→0(하드코딩 DB 비번 상수 삭제 포함), eslint 경고 0 |
| WS-D | ✅ 완료(핵심) | 프론트 단위 안전망(useNocSignals·trendOf·useMetricHistory·i18n)+App 스모크; 패널별 스모크는 WS-B와 병행 잔여 |
| WS-A | ✅ 완료 | 단일 SSE 스트림(dashboard_sse + watch 폴러 + useDashboardStream, /live 폴백). Playwright 검증 |
| WS-B | ✅ 완료 | MEC 패널 전체 ko/en/ja(useT); t() 키 391개 전부 정의, 사용자 표시 한글 0. EN 런타임 검증 |
| WS-F | ✅ 완료 | Cmd+K 명령 팔레트 + URL 딥링크(#cat/tab). Playwright 검증 |
| WS-G | ✅ 완료(코어) | kubeconfig 만료 D-n 조기경보(JWT exp 디코드; opaque 토큰은 정직하게 unknown) → NOC 주의 큐. HTTPS 전제는 아래 §HTTPS |
| WS-E | ⏸ 보류(인프라) | GPU%(dcgm-exporter)·VPN(AXGate 라이브 세션) 미확보 → NOC 정직한 플레이스홀더 유지. 코드 슬롯 준비됨 |

**HTTPS/프록시 전제(WS-A/WS-G)**: 단일 SSE는 리버스 프록시에서 `proxy_buffering off`(nginx) 또는 HTTP/2가 필요하다. Wake Lock API는 HTTPS(보안 컨텍스트)에서만 동작한다. 미충족 시 SSE는 자동으로 `/live` 폴링으로 폴백하므로 무중단이다.

---

## 1. 업그레이드 원칙 (전 워크스트림 공통)

1. **라인 규율** — 소프트 250 / 하드 400. 손대는 김에 초과 파일 분해. 신규 파일은 처음부터 준수.
2. **디자인 시스템 우선** — `components/mec/common/*`를 단일 출처로. 신규 화면은 독자 타일/색 금지, 프리미티브 조립.
3. **i18n 표준** — 모든 신규/수정 컴포넌트 `useT`, ko/en/ja 키 필수. 번역은 `i18n/translations/*`로 분리.
4. **정직한 데이터** — 수치 미조작. 미연결 소스는 "데이터 소스 미연결" 명시.
5. **소스별 오류 격리** — 한 의존성 장애가 전체 화면을 죽이지 않는다.
6. **안정성 회귀 금지** — 변경은 빌드·타입·테스트 그린 유지가 머지 조건.

---

## 2. 워크스트림 (WS)

### WS-A · 실시간 스트리밍 (NOC Phase 3 승격)
폴링(N개 벽 × 4 kube 호출/10s)을 단일 SSE로 대체해 부하·체감지연 동시 개선.

| 작업 | 파일 | ~라인 | 비고 |
|------|------|------|------|
| `dashboard_sse.rs` (jobs_sse 모델, 진짜 `event:heartbeat`) | `mec/handlers/dashboard_sse.rs` | 110 | `jobs_sse.rs` 재사용 |
| `MecState`에 `watch::Sender<LiveSnapshot>` + 백그라운드 폴 1개 | `mec/state.rs` | 40 | watch=Lagged 없음, resync 불필요 |
| 라우트 등록 | `mec/routes.rs` | 5 | |
| `useDashboardStream` (rAF 합치기·백오프 재연결·워치독) | `hooks/mec/useDashboardStream.ts` | 150 | `useJobStream` 일반화 |
| `MecNocPanel`/`MecLivePanel` 폴→스트림 전환 | 해당 패널 | 20 | 폴백: 스트림 실패 시 기존 `useMecApi` |

**선행조건**: TLS 리버스 프록시에서 `proxy_buffering off`(또는 HTTP/2). **수용 기준**: 벽 2개 동시 접속 시 kube 호출이 1세트로 합쳐짐(서버 로그 확인), 스트림 끊김 시 자동 재연결, 화면 깜빡임 없음.

### WS-B · MEC 패널 디자인시스템 + i18n 정렬 (최대 부채)
레거시 서버패널은 i18n 완료. **MEC 패널 23개가 `common/*` 부분 적용 + useT 미적용**. NOC가 레퍼런스.

- 대상(우선순위순): `MecDashboardPanel`, `MecLivePanel`, `TenantsPanel`(+탭 6종), `MecNodesPanel`, `MecStoragePanel`, `MecIngressPanel`, `MecFirewallPanel`, `MecUsersPanel`, `MecGpuPanel`, `MecHealthPanel`, `MecAuditPanel`, `MecJobsPanel`, `MecSettingsPanel`, `MecDiscoveryPanel`.
- 각 패널: ① 자체 테이블/카드 → `SortableTable`/`MetricCard`/`PanelLayout`로 치환 ② 하드코딩 한글 → `useT` + `i18n/translations/mec-*.ts` ③ 심각도 색 → `tone()`+`severityGlyph()` 3중 채널.
- **점진 이관**: 패널 1개 = PR 1개. 동작 보존 + 스냅샷 회귀 확인. 라인 초과분 동시 분해.

### WS-C · 기술부채 청산 (저위험·고청결)
- **하드리미트 분해**: `ldap_auth.rs`(489)→인증 흐름/디렉터리 검색/테스트 분리; `oauth.rs`(466)→provider별 또는 flow별 분리.
- **dead-code 48건**: 미사용 함수/필드 — (a) 향후 사용 예정이면 `#[allow(dead_code)]`+TODO, (b) 불필요하면 삭제. `cargo fix` 자동 7건 우선 적용 후 수동 검토.
- **eslint 경고**: `TerminalPanel.tsx:230` `timeoutLabel` 제거.
- **소프트 250 초과 33건**: WS-B/WS-A에서 손대는 파일 우선, 나머지는 기회주의적.

### WS-D · 프론트 테스트 안전망
현재 `App.test.tsx` 1개. 회귀 위험 큼.
- **순수 로직 우선**: `useNocSignals`(임계/롤업), `useMetricHistory`(trendOf), `i18n` 보간 — 단위 테스트.
- **렌더 스모크**: 각 패널 최소 1개 "데이터 빈/부분/정상" 무크래시 렌더 테스트.
- **목표**: 핵심 훅 100% + 패널 스모크. CI에서 `tsc`+`eslint`+`test` 게이트.

### WS-E · 관측성 심화 [보류: 인프라 의존]
- **GPU 가동률%**: dcgm-exporter(또는 NVML) → `NodeMetrics.gpu_usage_percent`(이미 `Option`). 배포 시점이 블로커.
- **VPN/사용자 세션**: `AxgateService::list_vpn_sessions()` — **600초 잠금 가드** 필수, 형식은 라이브 세션 1개 캡처 후 확정([`memory: axgate-vpn-cli`]). NOC 세션 밴드 플레이스홀더 점등.
- 둘 다 optional 필드 추가라 클라이언트 재작성 불필요. 일정 독립.

### WS-F · 제품 UX 고도화
- **Cmd+K 명령 팔레트**: 탭 전환·테넌트/노드 점프·작업 실행. `nav.ts` 메타 재사용.
- **URL 딥링크**: 탭/카테고리/포커스 상태를 URL에 — 새로고침·공유 가능(Grafana Scenes 패턴).
- **상세창 임베디드 콘솔**: 노드/테넌트 상세에서 `TerminalPanel` 임베드(Proxmox/Lens 패턴).
- **리스트 인라인 스파크라인 + 가상 스크롤**: 대형 목록(파드/이벤트/감사) 성능.

### WS-G · 보안·운영 하드닝
- **kubeconfig 만료 D-n 조기경보**: 반복된 MEC 401 사고 사전 차단([`memory: mec-kube-auth`]) — NOC 주의 큐 + 알림.
- 기존 자산 점검: `rbac`·`audit`·`ip_block`·`rate_limit`·`totp`·`jwt_sessions` 정책 일관성 재확인.
- SSE/Wake Lock의 HTTPS 전제 문서화.

---

## 3. 우선순위 & 시퀀싱

| 스프린트 | 포함 WS | 근거 |
|----------|---------|------|
| **S1 (청결·안전망)** | WS-C 전부 + WS-D 핵심 훅 | 저위험·즉시 부채 감소. 이후 작업의 안전망 선확보. |
| **S2 (실시간)** | WS-A | NOC 플래그십 완성. 선행조건(proxy_buffering) 확인 필요. |
| **S3 (일관성)** | WS-B (대시보드·라이브·테넌트 먼저) + WS-D 스모크 동반 | 사용 빈도 높은 MEC 패널부터 디자인시스템·i18n 정렬. |
| **S4 (UX 고도화)** | WS-F | 명령 팔레트·딥링크·임베디드 콘솔. |
| **상시·독립** | WS-E, WS-G | 인프라/보안 — 일정 비동기. |

권장 시작점: **S1** (즉시 착수 가능, 리스크 최저). WS-A는 `proxy_buffering` 확인 직후.

---

## 4. 리스크

- **WS-A SSE**: 리버스 프록시 버퍼링 시 스트림 미작동 → 폴백(`useMecApi`) 유지로 무중단.
- **WS-B 대량 변경**: 패널 1개=PR 1개 + 스냅샷 회귀로 폭발반경 축소.
- **WS-C dead-code 삭제**: 외부 호출 가능성 → 삭제 전 워크스페이스 전체 grep + 테스트.
- **WS-E 인프라**: 미확보 시 정직한 플레이스홀더 유지(허위 수치 금지).

---

## 5. 미해결 결정사항 (착수 전 확인)

1. TLS 프록시 `proxy_buffering off` / HTTP/2 여부 — WS-A 블로커.
2. dcgm-exporter 배포 시점 — WS-E GPU 블로커.
3. AXGate VPN 라이브 세션 캡처 가능 여부 — WS-E VPN 형식 확정.
4. 심각도 임계값 확정(현재 ≥70 주의 / ≥90 장애) — 알림 정책.
5. 담당자 라우팅: 단일 글로벌 vs 노드/테넌트별.
6. 다중 클러스터 관리 필요성(클러스터 셀렉터).
7. dead-code: 삭제 vs `#[allow]` 보존 — 향후 로드맵 의존.
