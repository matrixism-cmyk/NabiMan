import React from 'react';
import { WizardState } from './WizardState';

interface Props {
  state: WizardState;
  setState: (s: WizardState) => void;
}

export default function Step1Basic({ state, setState }: Props) {
  return (
    <div>
      <h3>Step 1 / 4: 기본 정보</h3>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        기업명 (한글)
        <input
          required
          value={state.displayName}
          onChange={(e) => setState({ ...state, displayName: e.target.value })}
          placeholder="㈜와이그램"
          style={{ width: '100%' }}
        />
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        테넌트 ID (영문 소문자/숫자/하이픈, NS/프로젝트명으로 사용)
        <input
          required
          value={state.tenantId}
          onChange={(e) =>
            setState({ ...state, tenantId: e.target.value.toLowerCase() })
          }
          placeholder="ygram-poc"
          pattern="[a-z][a-z0-9-]*[a-z0-9]"
          style={{ width: '100%' }}
        />
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        과제명
        <input
          value={state.taskName}
          onChange={(e) => setState({ ...state, taskName: e.target.value })}
          placeholder="페르소나 AI 토이 'NOVA'의 5G MEC 기반 한국형 서비스 실증"
          style={{ width: '100%' }}
        />
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        담당자 이메일
        <input
          type="email"
          value={state.contactEmail}
          onChange={(e) => setState({ ...state, contactEmail: e.target.value })}
          placeholder="contact@example.com"
          style={{ width: '100%' }}
        />
      </label>
    </div>
  );
}
