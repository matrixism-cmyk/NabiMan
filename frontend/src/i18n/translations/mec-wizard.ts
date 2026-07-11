import { Lang } from './index';

// MEC tenant-creation wizard copy. Reuses shared mec.action.* / mec.state.* /
// mec.col.* keys (defined in mec-live.ts); everything wizard-specific lives
// under the mec.wizard.* prefix.
const mecWizard: Record<string, Record<Lang, string>> = {
  // Step tabs / nav
  'mec.wizard.step.basic': { ko: '기본', en: 'Basic', ja: '基本' },
  'mec.wizard.step.node': { ko: '노드', en: 'Node', ja: 'ノード' },
  'mec.wizard.step.quota': { ko: '쿼터', en: 'Quota', ja: 'クォータ' },
  'mec.wizard.step.options': { ko: '옵션', en: 'Options', ja: 'オプション' },

  'mec.wizard.cancel': { ko: '취소', en: 'Cancel', ja: 'キャンセル' },
  'mec.wizard.prev': { ko: '← 이전', en: '← Back', ja: '← 戻る' },
  'mec.wizard.next': { ko: '다음 →', en: 'Next →', ja: '次へ →' },
  'mec.wizard.creating': { ko: '생성 중...', en: 'Creating…', ja: '作成中…' },
  'mec.wizard.create': { ko: '✓ 생성', en: '✓ Create', ja: '✓ 作成' },

  // Step 1: Basic
  'mec.wizard.s1.title': { ko: 'Step 1 / 4: 기본 정보', en: 'Step 1 / 4: Basic Info', ja: 'ステップ 1 / 4: 基本情報' },
  'mec.wizard.s1.displayName': { ko: '기업명 (한글)', en: 'Company Name (Korean)', ja: '企業名（韓国語）' },
  'mec.wizard.s1.displayNamePh': { ko: '㈜와이그램', en: 'Y-Gram Inc.', ja: 'Y-Gram Inc.' },
  'mec.wizard.s1.tenantId': {
    ko: '테넌트 ID (영문 소문자/숫자/하이픈, NS/프로젝트명으로 사용)',
    en: 'Tenant ID (lowercase letters/digits/hyphens, used as namespace/project name)',
    ja: 'テナントID（英小文字/数字/ハイフン、Namespace/プロジェクト名として使用）',
  },
  'mec.wizard.s1.taskName': { ko: '과제명', en: 'Project Name', ja: '課題名' },
  'mec.wizard.s1.taskNamePh': {
    ko: "페르소나 AI 토이 'NOVA'의 5G MEC 기반 한국형 서비스 실증",
    en: "PoC of Korean service for persona AI toy 'NOVA' on 5G MEC",
    ja: "ペルソナAIトイ「NOVA」の5G MECベース韓国型サービス実証",
  },
  'mec.wizard.s1.contactEmail': { ko: '담당자 이메일', en: 'Contact Email', ja: '担当者メール' },

  // Step 2: Node
  'mec.wizard.s2.title': { ko: 'Step 2 / 4: 노드 할당', en: 'Step 2 / 4: Node Allocation', ja: 'ステップ 2 / 4: ノード割り当て' },
  'mec.wizard.s2.dedicated': {
    ko: '단독 할당 (Taint + NoSchedule)',
    en: 'Dedicated (Taint + NoSchedule)',
    ja: '専有割り当て（Taint + NoSchedule）',
  },
  'mec.wizard.s2.shared': {
    ko: '공유 할당 (nodeSelector만)',
    en: 'Shared (nodeSelector only)',
    ja: '共有割り当て（nodeSelectorのみ）',
  },
  'mec.wizard.s2.selectNode': {
    ko: '노드 선택 (단독 할당 시 필수)',
    en: 'Select Node (required for dedicated allocation)',
    ja: 'ノード選択（専有割り当て時は必須）',
  },
  'mec.wizard.s2.loading': { ko: '노드 로딩 중...', en: 'Loading nodes…', ja: 'ノード読み込み中…' },
  'mec.wizard.s2.col.select': { ko: '선택', en: 'Select', ja: '選択' },
  'mec.wizard.s2.col.node': { ko: '노드', en: 'Node', ja: 'ノード' },
  'mec.wizard.s2.col.currentTenant': { ko: '현재 Tenant', en: 'Current Tenant', ja: '現在のTenant' },
  'mec.wizard.s2.available': { ko: '가용', en: 'Available', ja: '空き' },
  'mec.wizard.s2.gpuLabel': { ko: 'GPU 라벨', en: 'GPU Label', ja: 'GPUラベル' },

  // Step 3: Quota
  'mec.wizard.s3.title': { ko: 'Step 3 / 4: 리소스 쿼터', en: 'Step 3 / 4: Resource Quota', ja: 'ステップ 3 / 4: リソースクォータ' },
  'mec.wizard.s3.template': { ko: '템플릿', en: 'Template', ja: 'テンプレート' },
  'mec.wizard.s3.custom': { ko: '커스텀 (직접 편집)', en: 'Custom (edit manually)', ja: 'カスタム（手動編集）' },

  // Quota template labels (rendered in Step3Quota via QUOTA_TEMPLATES[].labelKey)
  'mec.wizard.tpl.standard': {
    ko: '표준 (CPU 32/64, Mem 64Gi/128Gi, GPU 2)',
    en: 'Standard (CPU 32/64, Mem 64Gi/128Gi, GPU 2)',
    ja: '標準 (CPU 32/64, Mem 64Gi/128Gi, GPU 2)',
  },
  'mec.wizard.tpl.dedicatedNode': {
    ko: '노드 전체 단독 (CPU 64/70, Mem 400Gi/500Gi, GPU 7)',
    en: 'Whole-node dedicated (CPU 64/70, Mem 400Gi/500Gi, GPU 7)',
    ja: 'ノード全体専有 (CPU 64/70, Mem 400Gi/500Gi, GPU 7)',
  },
  'mec.wizard.tpl.large': {
    ko: '대용량 (CPU 64/128, Mem 128Gi/256Gi, GPU 4)',
    en: 'Large (CPU 64/128, Mem 128Gi/256Gi, GPU 4)',
    ja: '大容量 (CPU 64/128, Mem 128Gi/256Gi, GPU 4)',
  },

  // Step 4: Options
  'mec.wizard.s4.title': { ko: 'Step 4 / 4: 추가 옵션', en: 'Step 4 / 4: Additional Options', ja: 'ステップ 4 / 4: 追加オプション' },
  'mec.wizard.s4.starterKit': {
    ko: '스타터킷 배포 (Ubuntu SSH + VS Code) — 생성 후 별도 단계에서 배포됩니다',
    en: 'Deploy Starter Kit (Ubuntu SSH + VS Code) — deployed in a separate step after creation',
    ja: 'スターターキット配備（Ubuntu SSH + VS Code）— 作成後に別ステップで配備されます',
  },
  'mec.wizard.s4.harbor': {
    ko: 'Harbor 프로젝트 자동 생성',
    en: 'Auto-create Harbor project',
    ja: 'Harborプロジェクトを自動作成',
  },
  'mec.wizard.s4.guide': {
    ko: '접속 가이드 docx 자동 생성',
    en: 'Auto-generate access guide (docx)',
    ja: 'アクセスガイドdocxを自動生成',
  },
  'mec.wizard.s4.egress': {
    ko: 'Egress 허용 NetworkPolicy 포함 (기본 ON)',
    en: 'Include egress-allow NetworkPolicy (default ON)',
    ja: 'Egress許可NetworkPolicyを含む（デフォルトON）',
  },
  'mec.wizard.s4.confirm': { ko: '확인', en: 'Confirm', ja: '確認' },
  'mec.wizard.s4.tenantId': { ko: '테넌트 ID', en: 'Tenant ID', ja: 'テナントID' },
  'mec.wizard.s4.allocation': { ko: '할당', en: 'Allocation', ja: '割り当て' },
  'mec.wizard.s4.dedicatedSummary': { ko: '단독 ({node})', en: 'Dedicated ({node})', ja: '専有 ({node})' },
  'mec.wizard.s4.sharedSummary': { ko: '공유', en: 'Shared', ja: '共有' },

  // Validation error messages (returned as keys by validateStepX)
  'mec.wizard.err.tenantId': {
    ko: '테넌트 ID는 소문자/숫자/하이픈만, 영문자로 시작 - 영문자/숫자로 끝나야 합니다.',
    en: 'Tenant ID must use only lowercase letters/digits/hyphens, start with a letter, and end with a letter or digit.',
    ja: 'テナントIDは英小文字/数字/ハイフンのみ、英字で始まり、英字または数字で終わる必要があります。',
  },
  'mec.wizard.err.displayName': {
    ko: '기업명을 입력하세요.',
    en: 'Please enter a company name.',
    ja: '企業名を入力してください。',
  },
  'mec.wizard.err.dedicatedNode': {
    ko: '단독 할당 선택 시 노드를 지정하세요.',
    en: 'Please select a node when using dedicated allocation.',
    ja: '専有割り当てを選択した場合はノードを指定してください。',
  },
  'mec.wizard.err.badResources': {
    ko: '잘못된 자원 값입니다.',
    en: 'Invalid resource values.',
    ja: 'リソース値が不正です。',
  },
};

export default mecWizard;
