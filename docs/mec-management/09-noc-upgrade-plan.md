# MEC 관제(NOC) & 제품 UI/UX 업그레이드 계획 (확정)

> 상태: **계획 확정 / 구현 보류**. 작성 2026-06-04. 다중 에이전트 벤치마킹·코드베이스 실측·설계·적대적 검증을 거쳐 도출.
> 결정: ① 계획만 확정 ② GPU%/VPN 인프라 불가·불확실 → Phase 4 보류 ③ i18n ko/en/ja 처음부터 완전 지원(새 표준).

## 1. 비전
기존 `frontend/src/components/mec/common/*` 프리미티브를 **디자인 시스템**으로 격상해 모든 화면이 동일한 컴포넌트·**의미론적 심각도 체계**·드릴다운·신선도(Live/Stale) 처리를 공유한다. 플래그십은 **풀스크린 MEC 관제(NOC) 월**. 이미 실데이터인 `/dashboard/live` 위에 기존 프리미티브만 조립해 1단계를 무중단 출시하고, 이후 단일 SSE → 신규 백엔드 데이터 순으로 확장한다. 수치는 절대 지어내지 않으며 미연결 소스는 "데이터 소스 미연결"로 정직하게 표기한다.

심각도 분류(전 제품 공통): **정상=녹 / 주의=황 / 장애=적 / 오프라인=회**. 색 단독 금지 — 색+글리프(모양/아이콘)+한글 라벨의 3중 채널(WCAG·색맹 안전).

## 2. 벤치마킹 핵심
- **Datadog(DRUIDS)**: 전 화면 동일 패턴 재사용 → `common/*` 디자인시스템화, 신규 패널 독자 타일/색 금지.
- **Netdata**: 2초 미만 체감 → 단일 SSE + rAF 합치기, 고정길이 스파크라인.
- **Run:ai / NVIDIA**: Ready/Allocated/**Idle GPU**가 1순위 효율 신호 → KPI·유휴 슬롯 강조.
- **NOC 영상벽(Barco/삼성)**: 중앙=상황도, 상단 KPI띠+하단 경보 티커, 멀리서 읽히는 RAG, bento 그리드, 다크.
- **국내 클라우드(NHN·네이버·KT·삼성SDS)**: 이용현황 먼저 + **장애 시 담당자 즉시 전달**, 한글 RAG.
- **Zabbix/PRTG·WCAG**: 색맹 안전 2차 채널, 다크 대비 재튜닝.
- **Proxmox/Lens/Cockpit/Coolify**: 상세창 임베디드 콘솔, 리스트 행 인라인 스파크라인.
- **Grafana Scenes**: URL 딥링크 가능한 뷰 상태.

## 3. MEC 관제 화면 설계
기존 **focusMode/Fullscreen**(Esc/F11 연동 완료)으로 엣지투엣지 실행되는 다크 bento 그리드.

- **헤더**: 제목·클러스터명·LiveIndicator·**SeverityRollup**(정상 N│주의 N│장애 N, 클릭 시 큐 필터)·⛶
- **상단 KPI 스트립**(count-up+스파크라인+RAG 테두리): 클러스터 CPU% / MEM% / 파드(running/total) / 노드 Ready / GPU 슬롯(used/total·유휴) / 세션(미연동 플레이스홀더)
- **중앙 HERO — 노드 압력 맵**: 노드별 RAG 타일 + CPU/MEM 미니바 + 상태점, **최악 우선 FLIP 정렬**, DOWN 노드 적색 해칭, 클릭→노드 탭
- **우측 — 주의 대상 큐**: 심각도 정렬 액션 행(NotReady 노드·CrashLoop/Failed 파드·CPU/MEM 임계·테넌트 초과·**kubeconfig 만료 D-n**·유휴 GPU), 글리프+색, →[패널] 드릴
- **중단**: 파드 상태 도넛 + 네임스페이스 소비 상위 바(클릭→테넌트)
- **라이브 이벤트 피드**: `SortableTable<ClusterEvent>` 신규 행 슬라이드인, Warning 적색+글리프
- **하단 경보 띠(Marquee)**: 장애/주의 요약 스크롤, 노드/파드/네임스페이스+사유+**담당자** 명시, 정상 시 "모든 시스템 정상"으로 축소, reduced-motion 시 정적 목록
- **GPU 가동률 / VPN·세션 밴드**: "데이터 소스 미연결(dcgm-exporter 필요 / AxgateService.list_vpn_sessions 필요)" 플레이스홀더 — Phase 4에서 점등

**실데이터(지금 가능)**: 클러스터/노드 CPU·MEM(metrics-server), 파드 상태, 네임스페이스 소비, 라이브 이벤트, GPU 슬롯 회계, kubeconfig 만료 조기경보.
**플레이스홀더**: GPU 가동률%, VPN/사용자 세션 (Phase 4, 인프라 확보 시).

모션: comprehension 목적만, prefers-reduced-motion 킬스위치. 재사용 키프레임 `mec-pop/mec-fade-in-up/mec-row-in/mec-live-pulse` + `useCountUp`. KPI count-up(250–300ms, 중간 도착 시 snap), 임계 교차 시 디바운스 펄스(≤1/s), 노드월 FLIP(<300ms·노드 수 상한), 마퀴는 compositor transform.

## 4. 단계별 구현 계획 (검증 수정 반영)

### Phase 1 — 관제 월 v1 (재사용 only · 무중단 · 인프라 선행조건 없음)
| 모듈 | 파일 | ~라인 | 재사용 |
|------|------|----|------|
| BE: `build_live`에 노드 **Ready/상태 + GPU 슬롯** 추가 *(현재 LiveSnapshot에 없음)* | `models/mec/live.rs`, `handlers/dashboard.rs` | 15 | `build_summary` 로직 |
| BE: kube list `.limit()` 페이지네이션 안전 | `mec/services/kube_real/live.rs` (74/99/131/148) | 15 | — |
| FE: `App.tsx` 렌더 스위치 → `<PanelOutlet>` 추출 *(App.tsx 400 한계, 선행 필수)* | `App.tsx`(분해) | 90 | 동작 보존 |
| FE: `useNocSignals` (순수 훅: 스냅샷→주의목록+롤업, 임계값 전담) | `hooks/mec/useNocSignals.ts` | 130 | tone()/타입 |
| FE: `MecNocPanel` (얇은 오케스트레이터) | `components/mec/MecNocPanel.tsx` | 200 | 모든 프리미티브, useMecApi(10s) |
| FE: `SeverityRollup` | `components/mec/noc/SeverityRollup.tsx` | 70 | useCountUp·tone·Dot |
| FE: `NocNodeWall`+`NocNodeTile` | `components/mec/noc/NocNodeWall.tsx` | 150 | BarChart·Sparkline·tone |
| FE: `NocAttentionQueue` | `components/mec/noc/NocAttentionQueue.tsx` | 110 | StatusBadge·mec-row-in |
| FE: `NocAlertMarquee` | `components/mec/noc/NocAlertMarquee.tsx` | 85 | tone·reduced-motion |
| FE: **i18n ko/en/ja** NOC 문자열(`useT`) + 셸 wiring(Tab/카테고리/렌더) | `i18n/translations/noc.ts`, `App.tsx` | 40 | i18n 인프라 |

데이터: 신규 전송 없음(기존 10s `/dashboard/live` 폴 + 문서 숨김 시 일시정지). GPU%/VPN은 정직한 플레이스홀더.
리스크: 낮음(순수 조립). MecNocPanel 라인 크리프 → noc/* 위임으로 방지. FLIP은 reduced-motion+노드수 가드.

### Phase 2 — 키오스크 하드닝 + 심각도 마감
`useWakeLock`(visibilitychange 재획득, ~45), `useStaleWatchdog`(무응답>~40s STALE, ~40), 벽 밀도 헬퍼(~40), **글리프 채널** `severityGlyph()`(common/StatusBadge.tsx, ~30), 다크 6프리셋 대비 감사(presets.ts, ~30). 데이터 신규 없음.

### Phase 3 — 단일 SSE 스트림 (검증: `watch`로 단순화)
`dashboard_sse.rs`(jobs_sse 모델, **진짜 event:heartbeat**, ~110), MecState에 `watch::Sender<LiveSnapshot>` + 백그라운드 폴 1개(~40), 라우트(~5), `useDashboardStream`(rAF 합치기·백오프 재연결·워치독, ~150). → N개 벽이 kube 호출 1세트 공유(6-연결 한도·N×4 호출 팬아웃 해소).
**검증 수정**: `watch`는 Lagged 없음 → Last-Event-ID/resync **불필요**(약 30–40라인 절감). 단일 벽이면 Phase 3 선출시 불필요하나, 다중 벽이면 Phase 2로 당기는 것 검토.

### Phase 4 — GPU 가동률 + VPN/세션 **[보류: 인프라 불가·불확실]**
업사이드·일정 독립. 확보 시: dcgm-exporter→`gpu{}`(NodeMetrics.gpu_usage_percent 이미 Option), `AxgateService::list_vpn_sessions()`→`vpn{}`(**600초 잠금 가드**, 형식은 라이브 세션 1개 캡처 후 확정 — `[[axgate-vpn-cli]]` 참조). 추가 optional 필드라 클라이언트 재작성 없음. 플레이스홀더 밴드 점등.

### Phase 5 — 서버 보유 시계열 + 제품 전반 UX
링버퍼 시계열+`/dashboard/history`(메모리 상한), Cmd+K 명령 팔레트, URL 딥링크+밀도 토글+App 셸 분해, 노드 상세 임베디드 터미널·행 인라인 스파크라인. (드래그 줌·동기 크로스헤어는 운영 가치 낮아 후순위.)

## 5. 신규 재사용 모듈
- `useNocSignals` — 순수 파생(스냅샷→주의목록+롤업), 임계값 전담, NOC·대시보드·향후 알림 공용.
- `useDashboardStream` — 프로덕션급 SSE 소비(합치기·재연결·워치독); `useJobStream`을 장기 채널로 일반화.
- `useWakeLock` / `useStaleWatchdog` — 키오스크·신선도 공용.
- `severityGlyph()` — tone()에 색맹 안전 2차 채널 추가(제품 전반).
- `noc/*` 프리미티브 — MecNocPanel을 얇게 유지하는 작은 조합 컴포넌트.

## 6. 안정성 (핵심)
- **소스별 오류 격리**: 한 의존성(kube 401·metrics-server 다운·AXGate 오프라인)이 죽어도 해당 밴드만 강등, 전체 월 생존.
- **kubeconfig 만료 D-n 조기경보**로 반복 MEC 401 사고 사전 차단(`[[mec-kube-auth]]`).
- `fetchWithRefresh` 401 재발급 재사용 → 원시 401 미노출.
- 명시적 신선도(Live/Stale/Reconnecting), 부분/빈 데이터 무크래시 렌더(자가 테스트).
- SSE: 연결/재연결 시 snapshot-then-stream, 백오프 1s→30s.
- 정직한 플레이스홀더(수치 미조작).
- 단일 스트림+숨김 시 일시정지로 백엔드 부하 상한.
- *(검증 추가)*: `useMetricHistory` localStorage 쓰기 압력 회피(월은 인메모리/디바운스), FLIP 노드 수 상한, 자가 새로고침은 **무사고 시간대에만**, Wake Lock 실패는 무시(크래시 금지).

## 7. 라인 규율 (소프트 250 / 하드 400)
`MecNocPanel`은 ~200라인 얇은 오케스트레이터, 시각 블록은 `noc/*` 위임, 임계값 로직은 `useNocSignals`에. 손대는 김에 리팩터 대상(하드 400 미만): `App.tsx(400 한계 — Phase 1에서 렌더 스위치 추출 선행)`, `MecFirewallPanel(393)`, `TenantsPanel(316)`, `MecStoragePanel(303)`, `MecIngressPanel(302)`, `MecUsersPanel(286)`, `MecNodesPanel(253)`. `MecLivePanel(226)`이 NOC 템플릿. BE 신규 파일은 `jobs_sse.rs(97)` 규모.

## 8. i18n 표준 (신규)
**모든 신규 컴포넌트는 `useT` 사용, ko/en/ja 키 필수.** NOC가 첫 적용처. 기존 MEC 패널(현재 0/27 useT)은 손대는 김에 점진 이관(WS6). 새 키는 `i18n/translations/noc.ts`로 분리.

## 9. 미해결/확인 필요
- dcgm-exporter(또는 NVML) 배포 가능 시점 — Phase 4 GPU% 블로커.
- AXGate VPN 라이브 세션 1개 캡처 가능 여부 — 형식 확정 전 세션 밴드는 플레이스홀더 유지.
- TLS 리버스 프록시/HTTP2 유무(`proxy_buffering off`) — Phase 3 단일 SSE·Wake Lock(HTTPS) 전제.
- 심각도 임계값 확정(현재 바: ≥70 주의/≥90 장애) — 노드/테넌트 알림 정책.
- 담당자 라우팅: 단일 글로벌(the@aeokorea.com) vs 노드/테넌트별.
- 다중 클러스터 관리 여부(클러스터 셀렉터 필요성).
