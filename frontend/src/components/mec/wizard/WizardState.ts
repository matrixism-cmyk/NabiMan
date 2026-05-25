export interface WizardState {
  // Step 1
  tenantId: string;
  displayName: string;
  taskName: string;
  contactEmail: string;
  // Step 2
  allocationType: 'dedicated' | 'shared';
  node: string;
  gpuLabel: string;
  // Step 3
  quotaTemplate: 'standard' | 'dedicated_node' | 'large' | 'custom';
  cpuReq: string;
  cpuLim: string;
  memReq: string;
  memLim: string;
  gpu: number;
  storage: string;
  pods: number;
  lbServices: number;
  // Step 4
  deployStarterKit: boolean;
  createHarborProject: boolean;
  generateGuide: boolean;
  includeEgressPolicy: boolean;
}

export function initialState(): WizardState {
  return {
    tenantId: '',
    displayName: '',
    taskName: '',
    contactEmail: '',
    allocationType: 'shared',
    node: '',
    gpuLabel: '',
    quotaTemplate: 'standard',
    cpuReq: '32',
    cpuLim: '64',
    memReq: '64Gi',
    memLim: '128Gi',
    gpu: 2,
    storage: '500Gi',
    pods: 100,
    lbServices: 5,
    deployStarterKit: false,
    createHarborProject: false,
    generateGuide: false,
    includeEgressPolicy: true,
  };
}

export interface QuotaTemplate {
  id: WizardState['quotaTemplate'];
  label: string;
  cpuReq: string;
  cpuLim: string;
  memReq: string;
  memLim: string;
  gpu: number;
  storage: string;
  pods: number;
  lbServices: number;
}

export const QUOTA_TEMPLATES: QuotaTemplate[] = [
  {
    id: 'standard',
    label: '표준 (CPU 32/64, Mem 64Gi/128Gi, GPU 2)',
    cpuReq: '32',
    cpuLim: '64',
    memReq: '64Gi',
    memLim: '128Gi',
    gpu: 2,
    storage: '500Gi',
    pods: 100,
    lbServices: 5,
  },
  {
    id: 'dedicated_node',
    label: '노드 전체 단독 (CPU 64/70, Mem 400Gi/500Gi, GPU 7)',
    cpuReq: '64',
    cpuLim: '70',
    memReq: '400Gi',
    memLim: '500Gi',
    gpu: 7,
    storage: '1Ti',
    pods: 150,
    lbServices: 5,
  },
  {
    id: 'large',
    label: '대용량 (CPU 64/128, Mem 128Gi/256Gi, GPU 4)',
    cpuReq: '64',
    cpuLim: '128',
    memReq: '128Gi',
    memLim: '256Gi',
    gpu: 4,
    storage: '1Ti',
    pods: 150,
    lbServices: 10,
  },
];

export function applyQuotaTemplate(
  state: WizardState,
  templateId: WizardState['quotaTemplate'],
): WizardState {
  if (templateId === 'custom') {
    return { ...state, quotaTemplate: 'custom' };
  }
  const tpl = QUOTA_TEMPLATES.find((t) => t.id === templateId);
  if (!tpl) return state;
  return {
    ...state,
    quotaTemplate: templateId,
    cpuReq: tpl.cpuReq,
    cpuLim: tpl.cpuLim,
    memReq: tpl.memReq,
    memLim: tpl.memLim,
    gpu: tpl.gpu,
    storage: tpl.storage,
    pods: tpl.pods,
    lbServices: tpl.lbServices,
  };
}

export function toRequestBody(state: WizardState): object {
  return {
    tenant_id: state.tenantId,
    display_name: state.displayName,
    task_name: state.taskName || null,
    contact_email: state.contactEmail || null,
    allocation: {
      type: state.allocationType,
      node: state.allocationType === 'dedicated' ? state.node : null,
      gpu_label: state.gpuLabel || null,
      tier: null,
    },
    quota: {
      cpu_requests: state.cpuReq,
      cpu_limits: state.cpuLim,
      memory_requests: state.memReq,
      memory_limits: state.memLim,
      gpu: state.gpu,
      storage: state.storage,
      pods: state.pods,
      pvcs: 20,
      lb_services: state.lbServices,
    },
    options: {
      deploy_starter_kit: state.deployStarterKit,
      create_harbor_project: state.createHarborProject,
      generate_guide: state.generateGuide,
      include_egress_policy: state.includeEgressPolicy,
      ingress_domain: null,
    },
  };
}

export function validateStep1(s: WizardState): string | null {
  if (!s.tenantId.match(/^[a-z][a-z0-9-]{0,62}[a-z0-9]$/)) {
    return '테넌트 ID는 소문자/숫자/하이픈만, 영문자로 시작 - 영문자/숫자로 끝나야 합니다.';
  }
  if (!s.displayName) return '기업명을 입력하세요.';
  return null;
}

export function validateStep2(s: WizardState): string | null {
  if (s.allocationType === 'dedicated' && !s.node) {
    return '단독 할당 선택 시 노드를 지정하세요.';
  }
  return null;
}

export function validateStep3(s: WizardState): string | null {
  if (s.gpu < 0 || s.pods < 1) return '잘못된 자원 값입니다.';
  return null;
}
