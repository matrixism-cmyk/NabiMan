import { Lang } from '../context';

// MEC 통합관제(NOC) wall strings. New i18n standard: ko/en/ja from the start.
const noc: Record<string, Record<Lang, string>> = {
  'tab.mecNoc': { ko: '통합관제', en: 'Control Room', ja: '統合管制' },

  'noc.title': { ko: 'MEC 통합관제', en: 'MEC Control Room', ja: 'MEC 統合管制' },
  'noc.subtitle': {
    ko: '실시간 클러스터·자원·접속 상황 관제',
    en: 'Real-time cluster, resource & connection control',
    ja: 'リアルタイム クラスター・リソース・接続管制',
  },
  'noc.loading': { ko: '관제 데이터 로딩 중…', en: 'Loading control data…', ja: '管制データ読込中…' },
  'noc.refresh': { ko: '새로고침', en: 'Refresh', ja: '更新' },
  'noc.fullscreen': { ko: '전체화면 관제', en: 'Fullscreen wall', ja: '全画面管制' },

  // severity
  'noc.normal': { ko: '정상', en: 'Normal', ja: '正常' },
  'noc.caution': { ko: '주의', en: 'Caution', ja: '注意' },
  'noc.critical': { ko: '장애', en: 'Critical', ja: '障害' },

  // freshness
  'noc.live': { ko: '실시간', en: 'LIVE', ja: 'リアルタイム' },
  'noc.stale': { ko: '지연', en: 'STALE', ja: '遅延' },

  // KPI
  'noc.kpi.cpu': { ko: '클러스터 CPU', en: 'Cluster CPU', ja: 'クラスターCPU' },
  'noc.kpi.memory': { ko: '클러스터 메모리', en: 'Cluster Memory', ja: 'クラスターメモリ' },
  'noc.kpi.pods': { ko: '실행 파드', en: 'Running Pods', ja: '実行Pod' },
  'noc.kpi.nodes': { ko: '노드 Ready', en: 'Nodes Ready', ja: 'ノードReady' },
  'noc.kpi.gpu': { ko: 'GPU 슬롯', en: 'GPU Slots', ja: 'GPUスロット' },
  'noc.kpi.sessions': { ko: '활성 세션', en: 'Active Sessions', ja: 'アクティブセッション' },
  'noc.kpi.podsHelper': { ko: '대기 {pending} · 실패 {failed}', en: 'Pending {pending} · Failed {failed}', ja: '待機 {pending}・失敗 {failed}' },
  'noc.kpi.gpuHelper': { ko: '유휴 {idle} 슬롯', en: '{idle} idle slots', ja: '空き {idle} スロット' },
  'noc.kpi.coresOf': { ko: '{used} / {total} 코어', en: '{used} / {total} cores', ja: '{used} / {total} コア' },
  'noc.kpi.gibOf': { ko: '{used} / {total} GiB', en: '{used} / {total} GiB', ja: '{used} / {total} GiB' },

  // panels
  'noc.nodeWall': { ko: '노드 압력 맵', en: 'Node Pressure Map', ja: 'ノード負荷マップ' },
  'noc.attention': { ko: '주의 대상', en: 'Attention Queue', ja: '注意対象' },
  'noc.attentionEmpty': { ko: '주의가 필요한 항목 없음', en: 'Nothing needs attention', ja: '注意項目なし' },
  'noc.podPhases': { ko: '파드 상태 분포', en: 'Pod Phases', ja: 'Pod状態分布' },
  'noc.tenants': { ko: '네임스페이스 자원 소비 (상위)', en: 'Namespace Consumption (top)', ja: 'Namespace消費 (上位)' },
  'noc.events': { ko: '라이브 클러스터 이벤트', en: 'Live Cluster Events', ja: 'ライブクラスターイベント' },
  'noc.tenantHelper': { ko: '{mem} GiB · {pods} 파드', en: '{mem} GiB · {pods} pods', ja: '{mem} GiB・{pods} Pod' },

  // placeholder bands (Phase 4)
  'noc.notConnected': { ko: '데이터 소스 미연결', en: 'Data source not connected', ja: 'データソース未接続' },
  'noc.gpuBand': { ko: 'GPU 가동률', en: 'GPU Utilization', ja: 'GPU稼働率' },
  'noc.gpuHint': { ko: 'dcgm-exporter 연동 필요', en: 'requires dcgm-exporter', ja: 'dcgm-exporter 連携が必要' },
  'noc.sessionBand': { ko: 'VPN / 사용자 세션', en: 'VPN / User Sessions', ja: 'VPN / ユーザーセッション' },
  'noc.sessionHint': { ko: 'AxgateService 연동 필요', en: 'requires AxgateService', ja: 'AxgateService 連携が必要' },

  // marquee
  'noc.allHealthy': { ko: '모든 시스템 정상', en: 'All systems normal', ja: '全システム正常' },
  'noc.owner': { ko: '담당', en: 'On-call', ja: '担当' },

  // attention signal reasons (composed with a target name in code)
  'noc.sig.nodeNotReady': { ko: '노드 응답 없음', en: 'Node not ready', ja: 'ノード応答なし' },
  'noc.sig.nodeCpu': { ko: 'CPU 임계 초과', en: 'CPU over threshold', ja: 'CPU閾値超過' },
  'noc.sig.nodeMem': { ko: '메모리 임계 초과', en: 'Memory over threshold', ja: 'メモリ閾値超過' },
  'noc.sig.podFailed': { ko: '실패한 파드 {n}개', en: '{n} failed pods', ja: '失敗Pod {n}件' },
  'noc.sig.podPending': { ko: '대기 중 파드 {n}개', en: '{n} pending pods', ja: '待機Pod {n}件' },
  'noc.sig.tenantMem': { ko: '테넌트 메모리 과다', en: 'Tenant memory high', ja: 'テナントメモリ過多' },
  'noc.sig.gpuIdle': { ko: 'GPU 유휴 {n} 슬롯', en: '{n} idle GPU slots', ja: 'GPU空き {n} スロット' },
  'noc.sig.eventWarn': { ko: '경고 이벤트 다발', en: 'Warning events', ja: '警告イベント多発' },
  'noc.sig.kubeExpiry': {
    ko: 'kubeconfig 토큰 만료 D-{n} (교체 필요)',
    en: 'kubeconfig token expires in {n}d (rotate it)',
    ja: 'kubeconfigトークン期限 残り{n}日（要更新）',
  },
};

export default noc;
