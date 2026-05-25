import React from 'react';
import { WizardState } from './WizardState';

interface Props {
  state: WizardState;
  setState: (s: WizardState) => void;
}

export default function Step4Options({ state, setState }: Props) {
  return (
    <div>
      <h3>Step 4 / 4: 추가 옵션</h3>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        <input
          type="checkbox"
          checked={state.deployStarterKit}
          onChange={(e) =>
            setState({ ...state, deployStarterKit: e.target.checked })
          }
        />{' '}
        스타터킷 배포 (Ubuntu SSH + VS Code) — 생성 후 별도 단계에서 배포됩니다
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        <input
          type="checkbox"
          checked={state.createHarborProject}
          onChange={(e) =>
            setState({ ...state, createHarborProject: e.target.checked })
          }
        />{' '}
        Harbor 프로젝트 자동 생성
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        <input
          type="checkbox"
          checked={state.generateGuide}
          onChange={(e) =>
            setState({ ...state, generateGuide: e.target.checked })
          }
        />{' '}
        접속 가이드 docx 자동 생성
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        <input
          type="checkbox"
          checked={state.includeEgressPolicy}
          onChange={(e) =>
            setState({ ...state, includeEgressPolicy: e.target.checked })
          }
        />{' '}
        Egress 허용 NetworkPolicy 포함 (기본 ON)
      </label>

      <div
        className="panel-card"
        style={{ marginTop: '16px', background: '#f3f4f6' }}
      >
        <h4 style={{ marginTop: 0 }}>확인</h4>
        <dl style={{ margin: 0, fontSize: '13px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <dt>테넌트 ID</dt>
            <dd style={{ margin: 0 }}>
              <code>{state.tenantId}</code>
            </dd>
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <dt>할당</dt>
            <dd style={{ margin: 0 }}>
              {state.allocationType === 'dedicated'
                ? `단독 (${state.node})`
                : '공유'}
              {state.gpuLabel ? ` · GPU ${state.gpuLabel}` : ''}
            </dd>
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <dt>CPU</dt>
            <dd style={{ margin: 0 }}>
              {state.cpuReq} / {state.cpuLim}
            </dd>
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <dt>Memory</dt>
            <dd style={{ margin: 0 }}>
              {state.memReq} / {state.memLim}
            </dd>
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <dt>GPU / Pods</dt>
            <dd style={{ margin: 0 }}>
              {state.gpu} / {state.pods}
            </dd>
          </div>
        </dl>
      </div>
    </div>
  );
}
