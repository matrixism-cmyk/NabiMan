import { Lang } from './index';

// MEC Tenant management panels (list, detail, and its Overview / Members /
// Quota / Resources / Audit / Starter Kit tabs). Reuses the shared
// mec.action.* / mec.state.* / mec.col.* keys from mec-live.ts.
const mecTenants: Record<string, Record<Lang, string>> = {
  "mec.scopedManaged.allowGrant": { ko: "새 외부 권한 부여 허용", en: "Allow new external access grants", ja: "新規外部権限付与を許可" },
  "mec.scopedManaged.allowRevoke": { ko: "관리 중인 직접 연결 회수 허용 (전체 접근 차단과 구분)", en: "Allow revoking managed direct bindings (distinct from all-access revocation)", ja: "管理中の直接接続の回収を許可（全アクセス遮断とは別）" },
  "mec.scopedManaged.grantUnavailable": { ko: "현재 유효한 권한 부여 위임을 확인할 수 없습니다. 연결 목록·회수 권한은 아래에서 별도로 확인하세요.", en: "A current grant delegation could not be verified. Review the binding list and revocation permissions below.", ja: "有効な権限付与の委任を確認できません。接続一覧・回収権限は下で別途確認してください。" },
  "mec.scopedManaged.title": { ko: "회원별 관리 중인 외부 연결", en: "Managed external bindings by member", ja: "メンバー別の管理中の外部接続" },
  "mec.scopedManaged.explanation": { ko: "승인된 관리 연결의 기록과 소속 정책 상태입니다. 기록만으로 현재 유효 접근을 증명하지 않습니다. 직접 연결 부재는 group·cluster·다른 project 권한의 전체 회수를 보증하지 않습니다.", en: "Durable managed binding records and current source policy. Records alone do not prove effective access. Direct binding absence does not prove revocation of group, cluster or other-project access.", ja: "承認済み管理接続の記録と所属ポリシーです。記録のみでは有効アクセスを証明しません。直接接続の不在はgroup・cluster・他projectの全権限回収を保証しません。" },
  "mec.scopedManaged.recordedActive": { ko: "기록상 관리 중", en: "Managed in the ledger", ja: "記録上、管理中" },
  "mec.scopedManaged.sourcePolicy": { ko: "현재 소속·환경 정책", en: "Current source and environment policy", ja: "現在の所属・環境ポリシー" },
  "mec.scopedManaged.allowed": { ko: "허용 (실제 접근은 별도 확인)", en: "Allowed (verify effective access separately)", ja: "許可（有効アクセスは別途確認）" },
  "mec.scopedManaged.denied": { ko: "불허 또는 환경 불일치", en: "Denied or environment mismatch", ja: "不許可または環境不一致" },
  "mec.scopedManaged.contextChanged": { ko: "현재 realm/project가 기록과 다릅니다. 최고관리자 검토가 필요합니다.", en: "Current realm/project differs from the record. Highest-administrator review is required.", ja: "現在のrealm/projectが記録と異なります。最高管理者の確認が必要です。" },
  "mec.scopedManaged.latest": { ko: "최근 기록된 변경", en: "Latest recorded change", ja: "直近の変更記録" },
  "mec.scopedManaged.pending": { ko: "해결되지 않은 회원 권한 변경이 있습니다. 회수를 반복하지 말고 최고관리자에게 결과 확인을 요청하세요.", en: "A member access change is unresolved. Do not repeat revocation; request highest-administrator confirmation.", ja: "メンバー権限の変更結果が未確定です。回収を繰り返さず最高管理者に結果確認を依頼してください。" },
  "mec.scopedManaged.empty": { ko: "현재 관리할 수 있는 기록된 연결이 없습니다.", en: "No recorded bindings currently within your management scope.", ja: "現在、管理範囲内の接続記録はありません。" },
  "mec.scopedManaged.revokeExplanation": { ko: "이 연결만 회수합니다. 회원 계정·소속 정책과 다른 연결은 유지됩니다. 외부 목록에서 정확한 ID·계정·역할을 확인하고, 부재와 감사를 저장한 뒤에만 회수 결과를 확정합니다.", en: "Revoke this binding only. The member account, source policy and other bindings remain. Exact ID, account and role are verified; revocation is confirmed only after absence and its audit are saved.", ja: "この接続のみを回収します。メンバーのアカウント・所属ポリシー・他接続は維持します。ID・アカウント・ロールを照合し、不在と監査の保存後に結果を確定します。" },
  "mec.scopedManaged.reason": { ko: "연결 회수 사유", en: "Reason for binding revocation", ja: "接続回収の理由" },
  "mec.scopedManaged.confirm": { ko: "위의 관리 연결 생성 요청 ID를 입력해 확인", en: "Enter the managed grant operation ID above to confirm", ja: "上記の管理接続作成リクエストIDを入力して確認" },
  "mec.scopedManaged.accept": { ko: "이 직접 연결의 부재 확인과 전체 외부 접근 차단의 차이를 검토했습니다.", en: "I reviewed the difference between absence of this direct binding and revocation of all external access.", ja: "この直接接続の不在確認と全外部アクセス遮断の違いを確認しました。" },
  "mec.scopedManaged.revoke": { ko: "이 관리 연결 회수", en: "Revoke this managed binding", ja: "この管理接続を回収" },
  "mec.scopedManaged.unconfirmed": { ko: "회수 결과를 확정하지 못했습니다. 삭제를 반복하지 말고 새 목록과 최고관리자의 변경 기록을 확인하세요.", en: "Revocation was not confirmed. Do not repeat deletion; check the refreshed list and highest-administrator operation history.", ja: "回収結果を確定できませんでした。削除を繰り返さず、更新された一覧と最高管理者の変更記録を確認してください。" },
  "mec.delegation.open": { ko: "외부 권한 관리", en: "Manage external access", ja: "外部権限を管理" },
  "mec.delegation.title": { ko: "최고관리자: 외부 권한 부여 위임", en: "Highest administrator: delegate access grants", ja: "最高管理者：外部権限付与の委任" },
  "mec.delegation.explanation": { ko: "지정한 tenant 관리자에게 승인된 하위 회원의 외부 권한 부여만 위임합니다. 자기 자신·동급 관리자·다른 tenant는 제외합니다. 위임 해제·만료는 새 권한 변경 요청을 차단하며 기존 연결을 자동 회수하지 않습니다. 기존 권한은 회원 소속 정책으로 회수합니다.", en: "Delegate external access grants for approved lower members to selected tenant administrators. Self, peer administrators and other tenants are excluded. Disabling or expiry blocks new access changes; existing grants remain governed by member source policy.", ja: "指定したテナント管理者に承認済み下位メンバーへの外部権限付与を委任します。自分・同格管理者・他テナントは対象外です。無効化・期限切れは新規の権限変更を禁止し、既存接続は所属ポリシーにより回収します。" },
  "mec.delegation.enabled": { ko: "위임 정책 활성 설정됨", en: "Delegation configured as enabled", ja: "委任ポリシーは有効に設定されています" },
  "mec.delegation.disabled": { ko: "위임 정책 비활성 또는 미설정", en: "Delegation disabled or not configured", ja: "委任は無効または未設定です" },
  "mec.delegation.current": { ko: "저장된 관리자·역할·만료일", en: "Saved managers, roles and expiry", ja: "保存済み管理者・ロール・期限" },
  "mec.delegation.enable": { ko: "위임 활성화", en: "Enable delegation", ja: "委任を有効化" },
  "mec.delegation.managers": { ko: "위임할 tenant 관리자", en: "Tenant administrators to delegate to", ja: "委任するテナント管理者" },
  "mec.delegation.memberRole": { ko: "project-member 역할도 허용 (공식 namespace 권한 범위 검토 필요)", en: "Also allow project-member (review documented namespace permissions)", ja: "project-memberも許可（namespace権限範囲の確認が必要）" },
  "mec.delegation.expiry": { ko: "위임 만료일 (지금부터 최대 30일)", en: "Delegation expiry (at most 30 days from now)", ja: "委任期限（現在から最大30日）" },
  "mec.delegation.reason": { ko: "위임 변경 사유", en: "Reason for delegation change", ja: "委任変更の理由" },
  "mec.delegation.confirm": { ko: "현재 tenant ID를 입력해 확인", en: "Enter the current tenant ID to confirm", ja: "現在のテナントIDを入力して確認" },
  "mec.delegation.review": { ko: "위임 대상·역할 범위·만료일과 기존 권한 유지 조건을 검토했습니다.", en: "I reviewed delegates, role scope, expiry and retention of existing grants.", ja: "委任先・ロール範囲・期限・既存権限の維持条件を確認しました。" },
  "mec.delegation.save": { ko: "위임 정책 저장", en: "Save delegation policy", ja: "委任ポリシーを保存" },
  "mec.delegation.saved": { ko: "위임 정책과 변경 감사를 함께 저장했습니다.", en: "Delegation and its audit were saved together.", ja: "委任ポリシーと変更監査を同時に保存しました。" },
  "mec.delegation.failed": { ko: "위임 정책을 확인하지 못했습니다. 새로고침 후 설정을 다시 검토하세요.", en: "Delegation was not confirmed. Refresh and review the settings again.", ja: "委任を確認できませんでした。更新して設定を再確認してください。" },
  "mec.delegation.delegatedTitle": { ko: "tenant 관리자: 승인된 외부 권한 부여", en: "Tenant administrator: approved external access grants", ja: "テナント管理者：承認済み外部権限付与" },
  "mec.delegation.delegatedExplanation": { ko: "최고관리자가 지정한 역할·만료 범위에서 자기 소속 하위 회원에게만 부여합니다. 외부 계정 연결은 최고관리자가 먼저 승인해야 합니다. 생성·회수 결과 불명은 최고관리자 검토가 필요하며 재생성하지 않습니다.", en: "Grant only to your lower members within the roles and expiry approved by the highest administrator. Provider identity links require prior highest-administrator approval. Uncertain outcomes require highest-administrator review; do not recreate access.", ja: "最高管理者が承認したロール・期限の範囲で所属下位メンバーにのみ付与します。外部アカウントの接続は最高管理者の事前承認が必要です。結果不明は最高管理者の確認が必要で、再作成しません。" },
  "mec.pendingManaged.title": { ko: "결과 불명 관리 요청", en: "Uncertain managed requests", ja: "結果不明の管理リクエスト" },
  "mec.pendingManaged.explanation": { ko: "아직 생성·회수 결과가 확정되지 않은 승인된 관리 요청입니다. 생성 요청을 반복하지 않습니다. 이 목록은 현재 유효 접근 권한을 증명하지 않습니다.", en: "Approved managed requests whose creation or cleanup is not confirmed. Creation is never replayed. This list does not prove effective access.", ja: "作成・回収結果が未確定の承認済み管理リクエストです。作成を再実行しません。この一覧は有効なアクセス権を証明しません。" },
  "mec.pendingManaged.project": { ko: "외부 프로젝트 ID", en: "External project ID", ja: "外部プロジェクトID" },
  "mec.pendingManaged.account": { ko: "외부 계정 ID", en: "External account ID", ja: "外部アカウントID" },
  "mec.pendingManaged.candidate": { ko: "소속 정책이 해제되면 저장된 연결 ID와 프로젝트·계정·역할의 일치를 다시 확인한 뒤 승인된 회수를 진행합니다. 불일치·조회 실패에는 관리자 검토가 필요합니다. 회수가 완료된 상태는 아닙니다.", en: "After source policy loss, cleanup proceeds only after the stored binding ID, project, account and role match. Mismatch or lookup failure requires review. Cleanup is not yet complete.", ja: "所属ポリシー解除後、保存済み接続ID・プロジェクト・アカウント・ロールの一致を確認して回収します。不一致・取得失敗には管理者の確認が必要です。回収は未完了です。" },
  "mec.pendingManaged.noCandidate": { ko: "저장된 연결 ID가 없어 자동 확인·회수가 불가능합니다. 이름이나 역할로 연결을 추정하지 않습니다. 외부 관리자 확인이 필요합니다.", en: "No stored binding ID: automatic confirmation and cleanup are unavailable. Names and roles are not used to infer ownership. External administrator review is required.", ja: "保存済み接続IDがないため自動確認・回収はできません。名前やロールから接続を推測しません。外部管理者の確認が必要です。" },
  "mec.pendingManaged.empty": { ko: "현재 결과 불명 관리 요청이 없습니다.", en: "No uncertain managed requests at present.", ja: "現在、結果不明の管理リクエストはありません。" },
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
