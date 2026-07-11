import { Lang } from './index';

// MEC Tenant management panels (list, detail, and its Overview / Members /
// Quota / Resources / Audit / Starter Kit tabs). Reuses the shared
// mec.action.* / mec.state.* / mec.col.* keys from mec-live.ts.
const mecTenants: Record<string, Record<Lang, string>> = {
  // ── TenantsPanel ─────────────────────────────────────────────────────────
  'mec.tenant.title': { ko: '테넌트', en: 'Tenants', ja: 'テナント' },
  'mec.tenant.subtitle': {
    ko: '{count}개 등록 · 활성 {active} · Discovery 탭에서 미등록 테넌트 Import 가능',
    en: '{count} registered · {active} active · unregistered tenants can be imported from the Discovery tab',
    ja: '{count}件登録 · アクティブ {active} · Discoveryタブで未登録テナントをImport可能',
  },
  'mec.tenant.cancel': { ko: '취소', en: 'Cancel', ja: 'キャンセル' },
  'mec.tenant.new': { ko: '+ 신규 테넌트', en: '+ New Tenant', ja: '+ 新規テナント' },
  'mec.tenant.filter': {
    ko: '필터 (ID, 기업명, 노드, GPU)',
    en: 'Filter (ID, company, node, GPU)',
    ja: 'フィルタ (ID, 企業名, ノード, GPU)',
  },
  'mec.tenant.empty': {
    ko: '등록된 테넌트가 없습니다. "+ 신규 테넌트" 또는 Discovery 탭에서 Import.',
    en: 'No tenants registered. Use "+ New Tenant" or import from the Discovery tab.',
    ja: '登録されたテナントがありません。「+ 新規テナント」またはDiscoveryタブからImportしてください。',
  },
  'mec.tenant.col.id': { ko: '테넌트 ID', en: 'Tenant ID', ja: 'テナントID' },
  'mec.tenant.col.company': { ko: '기업명', en: 'Company', ja: '企業名' },
  'mec.tenant.col.namespace': { ko: '네임스페이스', en: 'Namespace', ja: 'ネームスペース' },
  'mec.tenant.col.node': { ko: '노드', en: 'Node', ja: 'ノード' },
  'mec.tenant.shared': { ko: '공유', en: 'Shared', ja: '共有' },
  'mec.tenant.col.status': { ko: '상태', en: 'Status', ja: '状態' },
  'mec.tenant.action.detail': { ko: '상세', en: 'Detail', ja: '詳細' },
  'mec.tenant.title.docx': { ko: 'Word 문서', en: 'Word document', ja: 'Word文書' },
  'mec.tenant.title.txt': { ko: '텍스트', en: 'Text', ja: 'テキスト' },
  'mec.tenant.action.delete': { ko: '삭제', en: 'Delete', ja: '削除' },

  // Delete preflight / confirmation
  'mec.tenant.delete.emptyResponse': {
    ko: '삭제 사전 검사 응답이 비어 있습니다.',
    en: 'The delete preflight response is empty.',
    ja: '削除の事前検査レスポンスが空です。',
  },
  'mec.tenant.delete.heading': {
    ko: "테넌트 '{id}' 완전 삭제",
    en: "Permanently delete tenant '{id}'",
    ja: "テナント '{id}' を完全に削除",
  },
  'mec.tenant.delete.runningPods': {
    ko: '• 실행 중 Pod: {count}개',
    en: '• Running pods: {count}',
    ja: '• 実行中Pod: {count}件',
  },
  'mec.tenant.delete.lbServices': {
    ko: '• LoadBalancer 서비스: {count}개',
    en: '• LoadBalancer services: {count}',
    ja: '• LoadBalancerサービス: {count}件',
  },
  'mec.tenant.delete.starterKit': {
    ko: '• Starter Kit: {value}',
    en: '• Starter Kit: {value}',
    ja: '• Starter Kit: {value}',
  },
  'mec.tenant.present': { ko: '있음', en: 'Present', ja: 'あり' },
  'mec.tenant.absent': { ko: '없음', en: 'None', ja: 'なし' },
  'mec.tenant.delete.rancherProject': {
    ko: '• Rancher Project: {value}',
    en: '• Rancher Project: {value}',
    ja: '• Rancher Project: {value}',
  },
  'mec.tenant.presentDeleted': { ko: '있음 (삭제됨)', en: 'Present (deleted)', ja: 'あり (削除済み)' },
  'mec.tenant.delete.dedicatedNode': {
    ko: '• 단독 노드: {node}',
    en: '• Dedicated node: {node}',
    ja: '• 専用ノード: {node}',
  },
  'mec.tenant.delete.confirmPrompt': {
    ko: '확인을 위해 테넌트 ID를 그대로 입력하세요:',
    en: 'To confirm, type the tenant ID exactly:',
    ja: '確認のため、テナントIDをそのまま入力してください:',
  },
  'mec.tenant.delete.mismatch': {
    ko: '입력한 이름이 테넌트 ID와 일치하지 않아 삭제를 취소했습니다.',
    en: 'The name you entered does not match the tenant ID, so the deletion was cancelled.',
    ja: '入力した名前がテナントIDと一致しないため、削除をキャンセルしました。',
  },

  // ── TenantDetail ─────────────────────────────────────────────────────────
  'mec.tenant.notFound': {
    ko: '테넌트를 찾을 수 없습니다.',
    en: 'Tenant not found.',
    ja: 'テナントが見つかりません。',
  },
  'mec.tenant.backToList': { ko: '목록으로', en: 'Back to list', ja: '一覧へ' },
  'mec.tenant.detailHeading': {
    ko: '테넌트 — {name}',
    en: 'Tenant — {name}',
    ja: 'テナント — {name}',
  },

  // ── TenantOverviewTab ────────────────────────────────────────────────────
  'mec.tenant.overview.basicInfo': { ko: '기본 정보', en: 'Basic Info', ja: '基本情報' },
  'mec.tenant.overview.company': { ko: '기업명', en: 'Company', ja: '企業名' },
  'mec.tenant.overview.taskName': { ko: '과제명', en: 'Project Name', ja: '課題名' },
  'mec.tenant.overview.contactEmail': { ko: '담당자 이메일', en: 'Contact Email', ja: '担当者メール' },
  'mec.tenant.overview.status': { ko: '상태', en: 'Status', ja: '状態' },
  'mec.tenant.overview.allocatedResources': { ko: '할당 리소스', en: 'Allocated Resources', ja: '割り当てリソース' },
  'mec.tenant.overview.namespace': { ko: '네임스페이스', en: 'Namespace', ja: 'ネームスペース' },
  'mec.tenant.overview.allocationType': { ko: '할당 유형', en: 'Allocation Type', ja: '割り当てタイプ' },
  'mec.tenant.overview.node': { ko: '노드', en: 'Node', ja: 'ノード' },
  'mec.tenant.overview.gpuLabel': { ko: 'GPU 라벨', en: 'GPU Label', ja: 'GPUラベル' },
  'mec.tenant.overview.history': { ko: '이력', en: 'History', ja: '履歴' },
  'mec.tenant.overview.created': { ko: '생성', en: 'Created', ja: '作成' },
  'mec.tenant.overview.updated': { ko: '최종 갱신', en: 'Last Updated', ja: '最終更新' },

  // ── TenantMembersTab ─────────────────────────────────────────────────────
  'mec.tenant.members.roleOwner': {
    ko: 'Owner (전체 관리)',
    en: 'Owner (full management)',
    ja: 'Owner (全体管理)',
  },
  'mec.tenant.members.roleMember': {
    ko: 'Member (읽기/쓰기)',
    en: 'Member (read/write)',
    ja: 'Member (読み書き)',
  },
  'mec.tenant.members.roleReadOnly': {
    ko: 'Read-only (조회만)',
    en: 'Read-only (view only)',
    ja: 'Read-only (閲覧のみ)',
  },
  'mec.tenant.members.removeConfirm': {
    ko: '멤버 {user} 제거?',
    en: 'Remove member {user}?',
    ja: 'メンバー {user} を削除しますか?',
  },
  'mec.tenant.members.add': { ko: '멤버 추가', en: 'Add Member', ja: 'メンバー追加' },
  'mec.tenant.members.username': { ko: '사용자명', en: 'Username', ja: 'ユーザー名' },
  'mec.tenant.members.initialPassword': {
    ko: '초기 비밀번호 (신규 생성 시)',
    en: 'Initial password (when creating a new user)',
    ja: '初期パスワード (新規作成時)',
  },
  'mec.tenant.members.role': { ko: '권한', en: 'Role', ja: '権限' },
  'mec.tenant.members.createIfMissing': {
    ko: '사용자 없으면 생성',
    en: 'Create user if missing',
    ja: 'ユーザーがいなければ作成',
  },
  'mec.tenant.members.adding': { ko: '추가 중...', en: 'Adding…', ja: '追加中…' },
  'mec.tenant.members.addBtn': { ko: '+ 추가', en: '+ Add', ja: '+ 追加' },
  'mec.tenant.members.list': { ko: '멤버 목록', en: 'Member List', ja: 'メンバー一覧' },
  'mec.tenant.members.filter': {
    ko: '필터 (username, role)',
    en: 'Filter (username, role)',
    ja: 'フィルタ (username, role)',
  },
  'mec.tenant.members.empty': {
    ko: '이 테넌트에 멤버가 없습니다.',
    en: 'This tenant has no members.',
    ja: 'このテナントにはメンバーがいません。',
  },
  'mec.tenant.members.colUsername': { ko: '사용자명', en: 'Username', ja: 'ユーザー名' },
  'mec.tenant.members.colDisplayName': { ko: '표시명', en: 'Display Name', ja: '表示名' },
  'mec.tenant.members.colRole': { ko: '권한', en: 'Role', ja: '権限' },
  'mec.tenant.members.colBindingId': { ko: '바인딩 ID', en: 'Binding ID', ja: 'バインディングID' },
  'mec.tenant.members.remove': { ko: '제거', en: 'Remove', ja: '削除' },

  // ── TenantQuotaTab ───────────────────────────────────────────────────────
  'mec.tenant.quota.usage': { ko: 'ResourceQuota 사용률', en: 'ResourceQuota Usage', ja: 'ResourceQuota使用率' },

  // ── TenantResourcesTab ───────────────────────────────────────────────────
  'mec.tenant.resources.podsEmpty': {
    ko: '실행 중인 Pod가 없습니다.',
    en: 'No running pods.',
    ja: '実行中のPodがありません。',
  },
  'mec.tenant.resources.colName': { ko: '이름', en: 'Name', ja: '名前' },
  'mec.tenant.resources.colNode': { ko: '노드', en: 'Node', ja: 'ノード' },
  'mec.tenant.resources.colContainers': { ko: '컨테이너', en: 'Containers', ja: 'コンテナ' },
  'mec.tenant.resources.servicesEmpty': {
    ko: 'Service 가 없습니다.',
    en: 'No services.',
    ja: 'Serviceがありません。',
  },

  // ── TenantAuditTab ───────────────────────────────────────────────────────
  'mec.tenant.audit.empty': {
    ko: '이 테넌트의 감사 기록이 없습니다.',
    en: 'No audit records for this tenant.',
    ja: 'このテナントの監査記録がありません。',
  },
  'mec.tenant.audit.colUser': { ko: '사용자', en: 'User', ja: 'ユーザー' },
  'mec.tenant.audit.colAction': { ko: '작업', en: 'Action', ja: '操作' },
  'mec.tenant.audit.colResult': { ko: '결과', en: 'Result', ja: '結果' },
  'mec.tenant.audit.colDuration': { ko: '소요', en: 'Duration', ja: '所要時間' },

  // ── TenantStarterKitTab ──────────────────────────────────────────────────
  'mec.tenant.starter.description': {
    ko: 'Ubuntu SSH (port 22) + VS Code (code-server, port 8080) 파드를 이 테넌트에 배포합니다. MetalLB 가 설정되어 있다면 LoadBalancer 서비스로 자동 외부 IP가 할당됩니다.',
    en: 'Deploys an Ubuntu SSH (port 22) + VS Code (code-server, port 8080) pod to this tenant. If MetalLB is configured, an external IP is automatically assigned via a LoadBalancer service.',
    ja: 'Ubuntu SSH (port 22) + VS Code (code-server, port 8080) のPodをこのテナントにデプロイします。MetalLBが設定されていれば、LoadBalancerサービスで外部IPが自動的に割り当てられます。',
  },
  'mec.tenant.starter.deployedHeading': {
    ko: '배포 완료 — 비밀번호는 한 번만 표시됩니다',
    en: 'Deployment complete — passwords are shown only once',
    ja: 'デプロイ完了 — パスワードは一度だけ表示されます',
  },
  'mec.tenant.starter.passwordWarning': {
    ko: '이 비밀번호는 저장되지 않습니다. 안전한 곳에 복사해두세요.',
    en: 'These passwords are not stored. Copy them somewhere safe.',
    ja: 'このパスワードは保存されません。安全な場所にコピーしてください。',
  },
  'mec.tenant.starter.processing': { ko: '처리 중...', en: 'Processing…', ja: '処理中…' },
  'mec.tenant.starter.deploy': { ko: '배포 / 재배포', en: 'Deploy / Redeploy', ja: 'デプロイ / 再デプロイ' },
  'mec.tenant.starter.delete': { ko: '삭제', en: 'Delete', ja: '削除' },
  'mec.tenant.starter.deleteConfirm': {
    ko: "⚠️ Starter Kit (Ubuntu SSH + VS Code) 을(를) 삭제합니다.\n확인을 위해 테넌트 ID '{id}' 를 다시 입력하세요:",
    en: "⚠️ This will delete the Starter Kit (Ubuntu SSH + VS Code).\nTo confirm, re-enter the tenant ID '{id}':",
    ja: "⚠️ Starter Kit (Ubuntu SSH + VS Code) を削除します。\n確認のため、テナントID '{id}' を再入力してください:",
  },
  'mec.tenant.starter.deleteMismatch': {
    ko: '입력한 ID가 일치하지 않아 삭제를 취소했습니다.',
    en: 'The ID you entered does not match, so the deletion was cancelled.',
    ja: '入力したIDが一致しないため、削除をキャンセルしました。',
  },
};

export default mecTenants;
