import { Lang } from './index';

// Copy for the MEC Dashboard, Health, and Discovery panels. Reuses the shared
// mec.action.* / mec.state.* / mec.col.* keys defined in mec-live.ts.
const mecDashboard: Record<string, Record<Lang, string>> = {
  // MecDashboardPanel (mec.dash.*)
  'mec.dash.title': { ko: 'MEC 통합 대시보드', en: 'MEC Unified Dashboard', ja: 'MEC統合ダッシュボード' },
  'mec.dash.subtitle': {
    ko: '실시간 클러스터 자원 현황과 최근 운영 활동 요약',
    en: 'Real-time cluster resource status and a summary of recent operations',
    ja: 'リアルタイムのクラスターリソース状況と最近の運用アクティビティの概要',
  },
  'mec.dash.tenants': { ko: '테넌트', en: 'Tenants', ja: 'テナント' },
  'mec.dash.tenantsHelper': {
    ko: '활성 {active}곳 · 전체 {total}곳',
    en: 'Active {active} · Total {total}',
    ja: 'アクティブ {active} · 全体 {total}',
  },
  'mec.dash.nodesReady': { ko: '노드 Ready', en: 'Nodes Ready', ja: 'ノード Ready' },
  'mec.dash.nodesReadyHelper': {
    ko: '{pct}% 정상',
    en: '{pct}% healthy',
    ja: '{pct}% 正常',
  },
  'mec.dash.gpuSlot': { ko: 'GPU Slot', en: 'GPU Slot', ja: 'GPU Slot' },
  'mec.dash.gpuSlotHelper': {
    ko: '여유 {available} slots',
    en: '{available} slots free',
    ja: '空き {available} slots',
  },
  'mec.dash.lbServices': { ko: 'LB Services', en: 'LB Services', ja: 'LB Services' },
  'mec.dash.lbServicesHelper': {
    ko: '공인 IP 할당 {count}개',
    en: '{count} public IPs assigned',
    ja: 'パブリックIP {count} 件割当',
  },
  'mec.dash.natRules': { ko: 'NAT 규칙', en: 'NAT Rules', ja: 'NATルール' },
  'mec.dash.natRulesHelper': {
    ko: '보안 정책 {count}개',
    en: '{count} security policies',
    ja: 'セキュリティポリシー {count} 件',
  },
  'mec.dash.gpuAllocation': { ko: 'GPU Slot 할당', en: 'GPU Slot Allocation', ja: 'GPU Slot 割当' },
  'mec.dash.gpuAllocated': { ko: '할당', en: 'Allocated', ja: '割当' },
  'mec.dash.gpuAvailable': { ko: '여유', en: 'Available', ja: '空き' },
  'mec.dash.nodeStatus': { ko: '노드 상태', en: 'Node Status', ja: 'ノード状態' },
  'mec.dash.nodeTotal': { ko: '총 {total} 노드', en: '{total} nodes total', ja: '合計 {total} ノード' },
  'mec.dash.resourceUsage': { ko: '자원 사용률', en: 'Resource Usage', ja: 'リソース使用率' },
  'mec.dash.activeTenants': { ko: '활성 테넌트', en: 'Active Tenants', ja: 'アクティブテナント' },
  'mec.dash.healthyNodes': { ko: '정상 노드', en: 'Healthy Nodes', ja: '正常ノード' },
  'mec.dash.recentActivity': { ko: '최근 활동', en: 'Recent Activity', ja: '最近のアクティビティ' },
  'mec.dash.noActivity': {
    ko: '기록된 활동이 없습니다.',
    en: 'No recorded activity.',
    ja: '記録されたアクティビティはありません。',
  },
  'mec.dash.colUser': { ko: '사용자', en: 'User', ja: 'ユーザー' },
  'mec.dash.colAction': { ko: '작업', en: 'Action', ja: '操作' },
  'mec.dash.colDuration': { ko: '소요', en: 'Duration', ja: '所要' },
  'mec.dash.colResult': { ko: '결과', en: 'Result', ja: '結果' },

  // MecHealthPanel (mec.health.*)
  'mec.health.title': { ko: '외부 시스템 연결 상태', en: 'External System Connectivity', ja: '外部システム接続状態' },
  'mec.health.subtitle': {
    ko: '모드: {mode} · 실제 연결 {connected} / 4',
    en: 'Mode: {mode} · Real connections {connected} / 4',
    ja: 'モード: {mode} · 実接続 {connected} / 4',
  },
  'mec.health.checking': { ko: '검사 중...', en: 'Checking…', ja: '確認中…' },
  'mec.health.recheck': { ko: '재검사', en: 'Re-check', ja: '再検査' },

  // MecDiscoveryPanel (mec.disc.*)
  'mec.disc.title': { ko: '테넌트 Discovery', en: 'Tenant Discovery', ja: 'テナント Discovery' },
  'mec.disc.subtitle': {
    ko: '발견 {found} · 미등록 {unmanaged} · Import 로 기존 리소스를 재생성 없이 등록',
    en: 'Found {found} · Unregistered {unmanaged} · Import registers existing resources without recreating them',
    ja: '発見 {found} · 未登録 {unmanaged} · Import で既存リソースを再作成せずに登録',
  },
  'mec.disc.rescan': { ko: '다시 스캔', en: 'Rescan', ja: '再スキャン' },
  'mec.disc.infoIsProbe': {
    ko: '는 클러스터에서',
    en: ' scans the cluster for namespaces with a',
    ja: ' はクラスターから',
  },
  'mec.disc.infoSuffix': {
    ko: '접미사 또는',
    en: ' suffix or a',
    ja: ' 接尾辞または',
  },
  'mec.disc.infoLabel': {
    ko: '라벨이 있는 네임스페이스를 자동 탐색합니다. Import 시 기존 K8s / Rancher 리소스를 건드리지 않고 NabiMan DB 에만 등록합니다.',
    en: ' label automatically. On import it registers only into the NabiMan DB without touching existing K8s / Rancher resources.',
    ja: ' ラベルの付いたネームスペースを自動検出します。Import 時は既存の K8s / Rancher リソースに触れず NabiMan DB にのみ登録します。',
  },
  'mec.disc.filterPlaceholder': {
    ko: '필터 (네임스페이스)',
    en: 'Filter (namespace)',
    ja: 'フィルター（ネームスペース）',
  },
  'mec.disc.scanning': { ko: '스캔 중...', en: 'Scanning…', ja: 'スキャン中…' },
  'mec.disc.noResults': {
    ko: '발견된 미관리 테넌트가 없습니다.',
    en: 'No unmanaged tenants found.',
    ja: '未管理のテナントは見つかりませんでした。',
  },
  'mec.disc.colNamespace': { ko: '네임스페이스', en: 'Namespace', ja: 'ネームスペース' },
  'mec.disc.colLabels': { ko: 'Labels (일부)', en: 'Labels (partial)', ja: 'Labels（一部）' },
  'mec.disc.colStatus': { ko: '상태', en: 'Status', ja: '状態' },
  'mec.disc.registered': { ko: '등록됨', en: 'Registered', ja: '登録済み' },
  'mec.disc.importing': { ko: 'Import 중...', en: 'Importing…', ja: 'Import 中…' },
};

export default mecDashboard;
