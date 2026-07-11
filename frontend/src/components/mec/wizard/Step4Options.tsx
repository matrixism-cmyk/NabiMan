import React from 'react';
import { useT } from '../../../i18n';
import { WizardState } from './WizardState';

interface Props {
  state: WizardState;
  setState: (s: WizardState) => void;
}

export default function Step4Options({ state, setState }: Props) {
  const { t } = useT();
  return (
    <div>
      <h3>{t('mec.wizard.s4.title')}</h3>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        <input
          type="checkbox"
          checked={state.deployStarterKit}
          onChange={(e) =>
            setState({ ...state, deployStarterKit: e.target.checked })
          }
        />{' '}
        {t('mec.wizard.s4.starterKit')}
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        <input
          type="checkbox"
          checked={state.createHarborProject}
          onChange={(e) =>
            setState({ ...state, createHarborProject: e.target.checked })
          }
        />{' '}
        {t('mec.wizard.s4.harbor')}
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        <input
          type="checkbox"
          checked={state.generateGuide}
          onChange={(e) =>
            setState({ ...state, generateGuide: e.target.checked })
          }
        />{' '}
        {t('mec.wizard.s4.guide')}
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        <input
          type="checkbox"
          checked={state.includeEgressPolicy}
          onChange={(e) =>
            setState({ ...state, includeEgressPolicy: e.target.checked })
          }
        />{' '}
        {t('mec.wizard.s4.egress')}
      </label>

      <div
        className="panel-card"
        style={{ marginTop: '16px', background: '#f3f4f6' }}
      >
        <h4 style={{ marginTop: 0 }}>{t('mec.wizard.s4.confirm')}</h4>
        <dl style={{ margin: 0, fontSize: '13px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <dt>{t('mec.wizard.s4.tenantId')}</dt>
            <dd style={{ margin: 0 }}>
              <code>{state.tenantId}</code>
            </dd>
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between' }}>
            <dt>{t('mec.wizard.s4.allocation')}</dt>
            <dd style={{ margin: 0 }}>
              {state.allocationType === 'dedicated'
                ? t('mec.wizard.s4.dedicatedSummary', { node: state.node })
                : t('mec.wizard.s4.sharedSummary')}
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
