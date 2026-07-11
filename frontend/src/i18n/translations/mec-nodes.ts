import { Lang } from './index';

// MEC node management panels (node list, detail, taints/labels/GPU-mode editors).
// Reuses the shared mec.action.* / mec.state.* / mec.col.* keys defined in
// mec-live.ts so wording stays consistent across the MEC module.
const mecNodes: Record<string, Record<Lang, string>> = {
  // MecNodesPanel
  'mec.node.title': { ko: 'MEC 노드', en: 'MEC Nodes', ja: 'MECノード' },
  'mec.node.subtitle': {
    ko: '{count}개 노드 · Ready {ready} · GPU {allocated}/{total} slots',
    en: '{count} nodes · Ready {ready} · GPU {allocated}/{total} slots',
    ja: '{count}ノード · Ready {ready} · GPU {allocated}/{total} slots',
  },
  'mec.node.gpuSlotSection': {
    ko: '노드별 GPU slot (할당 vs 전체)',
    en: 'GPU slots by node (allocated vs total)',
    ja: 'ノード別GPU slot（割当 vs 全体）',
  },
  'mec.node.unallocated': { ko: '미할당', en: 'Unallocated', ja: '未割当' },
  'mec.node.filterPlaceholder': {
    ko: '필터 (이름/GPU/tenant)',
    en: 'Filter (name/GPU/tenant)',
    ja: 'フィルター（名前/GPU/tenant）',
  },
  'mec.node.empty': { ko: '노드가 없습니다.', en: 'No nodes.', ja: 'ノードがありません。' },
  'mec.node.col.node': { ko: '노드', en: 'Node', ja: 'ノード' },
  'mec.node.col.status': { ko: '상태', en: 'Status', ja: '状態' },
  'mec.node.col.roles': { ko: '역할', en: 'Roles', ja: '役割' },
  'mec.node.detail': { ko: '상세', en: 'Detail', ja: '詳細' },

  // MecNodeDetail
  'mec.node.detailTitle': { ko: '노드 상세 — {name}', en: 'Node Detail — {name}', ja: 'ノード詳細 — {name}' },
  'mec.node.backToList': { ko: '목록으로', en: 'Back to list', ja: '一覧へ' },
  'mec.node.noInfo': { ko: '노드 정보가 없습니다.', en: 'No node information.', ja: 'ノード情報がありません。' },
  'mec.node.info.status': { ko: '상태', en: 'Status', ja: '状態' },
  'mec.node.info.roles': { ko: '역할', en: 'Roles', ja: '役割' },
  'mec.node.info.arch': { ko: '아키텍처', en: 'Architecture', ja: 'アーキテクチャ' },
  'mec.node.info.cudaDriver': { ko: 'CUDA 드라이버', en: 'CUDA Driver', ja: 'CUDAドライバー' },
  'mec.node.info.currentTenant': { ko: '현재 Tenant', en: 'Current Tenant', ja: '現在のTenant' },
  'mec.node.info.noTenant': { ko: '(할당 없음)', en: '(none assigned)', ja: '（割当なし）' },
  'mec.node.gpu.model': { ko: '모델', en: 'Model', ja: 'モデル' },
  'mec.node.gpu.physicalCards': { ko: '물리 카드', en: 'Physical Cards', ja: '物理カード' },
  'mec.node.gpu.totalSlots': { ko: '전체 Slots', en: 'Total Slots', ja: '全体Slots' },
  'mec.node.gpu.currentMode': { ko: '현재 모드', en: 'Current Mode', ja: '現在のモード' },
  'mec.node.gpu.sharingStrategy': { ko: 'Sharing 전략', en: 'Sharing Strategy', ja: 'Sharing戦略' },

  // NodeLabelsEditor
  'mec.node.labels.actions': { ko: '작업', en: 'Actions', ja: '操作' },
  'mec.node.labels.empty': { ko: 'Label 없음', en: 'No labels', ja: 'Labelなし' },
  'mec.node.labels.systemLabelTip': {
    ko: '시스템 label은 제거 불가',
    en: 'System labels cannot be removed',
    ja: 'システムlabelは削除できません',
  },
  'mec.node.labels.remove': { ko: '제거', en: 'Remove', ja: '削除' },
  'mec.node.labels.confirmRemove': { ko: 'Label {key} 제거?', en: 'Remove label {key}?', ja: 'Label {key} を削除しますか？' },
  'mec.node.labels.applying': { ko: '적용 중...', en: 'Applying…', ja: '適用中…' },
  'mec.node.labels.addBtn': { ko: '+ Label', en: '+ Label', ja: '+ Label' },

  // NodeTaintsEditor
  'mec.node.taints.actions': { ko: '작업', en: 'Actions', ja: '操作' },
  'mec.node.taints.empty': { ko: 'Taint 없음', en: 'No taints', ja: 'Taintなし' },
  'mec.node.taints.remove': { ko: '제거', en: 'Remove', ja: '削除' },
  'mec.node.taints.confirmRemove': {
    ko: 'Taint {taint} 제거?',
    en: 'Remove taint {taint}?',
    ja: 'Taint {taint} を削除しますか？',
  },
  'mec.node.taints.valueOptional': { ko: 'Value (선택)', en: 'Value (optional)', ja: 'Value（任意）' },
  'mec.node.taints.applying': { ko: '적용 중...', en: 'Applying…', ja: '適用中…' },
  'mec.node.taints.addBtn': { ko: '+ Taint', en: '+ Taint', ja: '+ Taint' },

  // NodeGpuModeForm
  'mec.node.gpuMode.container': { ko: 'Container (단독 할당)', en: 'Container (dedicated)', ja: 'Container（専有割当）' },
  'mec.node.gpuMode.timeSlicing': { ko: 'Time-Slicing (공유)', en: 'Time-Slicing (shared)', ja: 'Time-Slicing（共有）' },
  'mec.node.gpuMode.mig': { ko: 'MIG (H100/A100)', en: 'MIG (H100/A100)', ja: 'MIG (H100/A100)' },
  'mec.node.gpuMode.vmPassthrough': { ko: 'VFIO Passthrough (VM)', en: 'VFIO Passthrough (VM)', ja: 'VFIO Passthrough (VM)' },
  'mec.node.gpuMode.confirm': {
    ko: 'GPU 모드를 {from} → {to} 로 전환합니다. 이 노드의 파드가 재스케줄될 수 있습니다. 계속하시겠습니까?',
    en: 'Switch GPU mode from {from} → {to}. Pods on this node may be rescheduled. Continue?',
    ja: 'GPUモードを {from} → {to} に切り替えます。このノードのPodが再スケジュールされる可能性があります。続行しますか？',
  },
  'mec.node.gpuMode.newMode': { ko: '새 모드', en: 'New Mode', ja: '新しいモード' },
  'mec.node.gpuMode.switching': { ko: '전환 중...', en: 'Switching…', ja: '切替中…' },
  'mec.node.gpuMode.switch': { ko: '전환', en: 'Switch', ja: '切替' },
};

export default mecNodes;
