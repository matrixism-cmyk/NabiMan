import { Lang } from './index';

// MEC GPU 및 스토리지/LB Pool/Harbor 패널 문구. 공통 mec.action.* / mec.state.* /
// mec.col.* 키는 mec-live.ts 것을 재사용한다.
const mecGpuStorage: Record<string, Record<Lang, string>> = {
  // --- GPU panel (mec.gpu.*) ---
  'mec.gpu.title': { ko: 'GPU 자원', en: 'GPU Resources', ja: 'GPUリソース' },
  'mec.gpu.subtitle': {
    ko: '총 {total} slots · {pct}% 할당',
    en: '{total} slots total · {pct}% allocated',
    ja: '合計 {total} slots · {pct}% 割り当て',
  },
  'mec.gpu.totalSlots': { ko: '전체 Slots', en: 'Total Slots', ja: '全Slots' },
  'mec.gpu.totalSlotsHelper': {
    ko: 'GPU 공유 전략 반영',
    en: 'GPU sharing strategy applied',
    ja: 'GPU共有戦略を反映',
  },
  'mec.gpu.allocated': { ko: '할당', en: 'Allocated', ja: '割り当て' },
  'mec.gpu.allocatedHelper': { ko: '{pct}% 사용', en: '{pct}% used', ja: '{pct}% 使用' },
  'mec.gpu.available': { ko: '가용', en: 'Available', ja: '空き' },
  'mec.gpu.availableHelper': {
    ko: '즉시 할당 가능',
    en: 'Ready to allocate',
    ja: '即時割り当て可能',
  },
  'mec.gpu.modelKinds': { ko: 'GPU 모델 종류', en: 'GPU Model Types', ja: 'GPUモデル種類' },
  'mec.gpu.overallUsage': { ko: '전체 GPU 사용률', en: 'Overall GPU Usage', ja: '全体GPU使用率' },
  'mec.gpu.sliceAllocated': { ko: '할당', en: 'Allocated', ja: '割り当て' },
  'mec.gpu.sliceAvailable': { ko: '가용', en: 'Available', ja: '空き' },
  'mec.gpu.modelDistribution': {
    ko: '모델별 slots 분포',
    en: 'Slots by Model',
    ja: 'モデル別slots分布',
  },
  'mec.gpu.modelCenter': { ko: '모델', en: 'Models', ja: 'モデル' },
  'mec.gpu.modelDetail': { ko: '모델별 상세', en: 'Model Details', ja: 'モデル別詳細' },
  'mec.gpu.nodesHelper': { ko: '노드: {nodes}', en: 'Nodes: {nodes}', ja: 'ノード: {nodes}' },

  // --- Storage / LB Pool / Harbor panel (mec.storage.*) ---
  'mec.storage.title': {
    ko: '스토리지 / LB Pool / Harbor',
    en: 'Storage / LB Pool / Harbor',
    ja: 'ストレージ / LB Pool / Harbor',
  },
  'mec.storage.subtitle': {
    ko: 'LB {used}/{total} · PVC {pvcs}개 (Bound {bound}) · Harbor {projects}개 프로젝트',
    en: 'LB {used}/{total} · PVC {pvcs} (Bound {bound}) · Harbor {projects} projects',
    ja: 'LB {used}/{total} · PVC {pvcs}件 (Bound {bound}) · Harbor {projects}件のプロジェクト',
  },
  'mec.storage.lbIpUsage': { ko: 'LB IP 사용', en: 'LB IP Usage', ja: 'LB IP使用' },
  'mec.storage.lbPoolsHelper': { ko: '{count}개 Pool', en: '{count} Pools', ja: '{count}個のPool' },
  'mec.storage.pvc': { ko: 'PVC', en: 'PVC', ja: 'PVC' },
  'mec.storage.pvcBoundHelper': { ko: 'Bound {bound}', en: 'Bound {bound}', ja: 'Bound {bound}' },
  'mec.storage.harborProjects': {
    ko: 'Harbor 프로젝트',
    en: 'Harbor Projects',
    ja: 'Harborプロジェクト',
  },
  'mec.storage.metallbPool': { ko: 'MetalLB IP Pool', en: 'MetalLB IP Pool', ja: 'MetalLB IP Pool' },
  'mec.storage.noPool': { ko: 'Pool 이 없습니다.', en: 'No pools.', ja: 'Poolがありません。' },
  'mec.storage.pvcSection': {
    ko: 'PersistentVolumeClaim',
    en: 'PersistentVolumeClaim',
    ja: 'PersistentVolumeClaim',
  },
  'mec.storage.pvcFilterPlaceholder': {
    ko: '필터 (ns, name, storage-class)',
    en: 'Filter (ns, name, storage-class)',
    ja: 'フィルター (ns, name, storage-class)',
  },
  'mec.storage.noPvc': { ko: 'PVC 가 없습니다.', en: 'No PVCs.', ja: 'PVCがありません。' },
  'mec.storage.colName': { ko: '이름', en: 'Name', ja: '名前' },
  'mec.storage.colPhase': { ko: '상태', en: 'Phase', ja: '状態' },
  'mec.storage.colRequest': { ko: '요청', en: 'Request', ja: '要求' },
  'mec.storage.harborSection': {
    ko: 'Harbor 프로젝트',
    en: 'Harbor Projects',
    ja: 'Harborプロジェクト',
  },
  'mec.storage.newProjectPlaceholder': {
    ko: '프로젝트 이름 (예: ygram-poc)',
    en: 'Project name (e.g. ygram-poc)',
    ja: 'プロジェクト名 (例: ygram-poc)',
  },
  'mec.storage.creating': { ko: '생성 중...', en: 'Creating…', ja: '作成中…' },
  'mec.storage.addProject': { ko: '+ 프로젝트', en: '+ Project', ja: '+ プロジェクト' },
  'mec.storage.noHarbor': {
    ko: 'Harbor 프로젝트가 없습니다.',
    en: 'No Harbor projects.',
    ja: 'Harborプロジェクトがありません。',
  },
  'mec.storage.colHarborName': { ko: '이름', en: 'Name', ja: '名前' },
  'mec.storage.colPublic': { ko: '공개', en: 'Public', ja: '公開' },
  'mec.storage.colRepos': { ko: '저장소', en: 'Repositories', ja: 'リポジトリ' },
  'mec.storage.colCreated': { ko: '생성', en: 'Created', ja: '作成' },
};

export default mecGpuStorage;
