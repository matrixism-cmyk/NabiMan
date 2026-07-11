import { Lang } from './index';

// Shared MEC panel copy (actions, states, common table columns) + the Live
// Monitoring panel. New MEC panels reuse the mec.action.* / mec.state.* /
// mec.col.* keys so wording stays consistent across the module.
const mecLive: Record<string, Record<Lang, string>> = {
  'mec.action.refresh': { ko: '새로고침', en: 'Refresh', ja: '更新' },
  'mec.state.loading': { ko: '로딩 중...', en: 'Loading…', ja: '読み込み中…' },

  'mec.col.time': { ko: '시간', en: 'Time', ja: '時刻' },
  'mec.col.type': { ko: '유형', en: 'Type', ja: '種別' },
  'mec.col.reason': { ko: '사유', en: 'Reason', ja: '理由' },
  'mec.col.object': { ko: '대상', en: 'Object', ja: '対象' },
  'mec.col.message': { ko: '메시지', en: 'Message', ja: 'メッセージ' },

  'mec.live.title': { ko: '실시간 모니터링', en: 'Live Monitoring', ja: 'リアルタイム監視' },
  'mec.live.subtitle': {
    ko: 'metrics-server 기반 실제 자원 사용률과 라이브 클러스터 이벤트',
    en: 'Real resource usage from metrics-server plus live cluster events',
    ja: 'metrics-serverによる実際のリソース使用率とライブクラスターイベント',
  },
  'mec.live.cpuCard': { ko: '클러스터 CPU', en: 'Cluster CPU', ja: 'クラスターCPU' },
  'mec.live.memCard': { ko: '클러스터 메모리', en: 'Cluster Memory', ja: 'クラスターメモリ' },
  'mec.live.podsRunning': { ko: '실행 중 파드', en: 'Running Pods', ja: '実行中Pod' },
  'mec.live.podsHelper': {
    ko: '대기 {pending} · 실패 {failed} · 완료 {succeeded}',
    en: 'Pending {pending} · Failed {failed} · Succeeded {succeeded}',
    ja: '待機 {pending} · 失敗 {failed} · 完了 {succeeded}',
  },
  'mec.live.nodes': { ko: '노드', en: 'Nodes', ja: 'ノード' },
  'mec.live.nodesHelper': {
    ko: 'metrics-server 수집 노드',
    en: 'Nodes reported by metrics-server',
    ja: 'metrics-serverが収集したノード',
  },
  'mec.live.nodeCpu': { ko: '노드별 CPU 사용률', en: 'CPU Usage by Node', ja: 'ノード別CPU使用率' },
  'mec.live.nodeMem': { ko: '노드별 메모리 사용률', en: 'Memory Usage by Node', ja: 'ノード別メモリ使用率' },
  'mec.live.podDist': { ko: '파드 상태 분포', en: 'Pod Phase Distribution', ja: 'Pod状態の分布' },
  'mec.live.podTotal': { ko: '총 {total} 파드', en: '{total} pods total', ja: '合計 {total} Pod' },
  'mec.live.nsConsumption': {
    ko: '네임스페이스별 자원 소비 (CPU 상위)',
    en: 'Resource Consumption by Namespace (top CPU)',
    ja: 'ネームスペース別リソース消費（CPU上位）',
  },
  'mec.live.events': { ko: '라이브 클러스터 이벤트', en: 'Live Cluster Events', ja: 'ライブクラスターイベント' },
  'mec.live.noEvents': { ko: '이벤트가 없습니다.', en: 'No events.', ja: 'イベントはありません。' },
};

export default mecLive;
