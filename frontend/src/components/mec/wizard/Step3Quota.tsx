import React from 'react';
import {
  applyQuotaTemplate,
  QUOTA_TEMPLATES,
  WizardState,
} from './WizardState';

interface Props {
  state: WizardState;
  setState: (s: WizardState) => void;
}

export default function Step3Quota({ state, setState }: Props) {
  const setTemplate = (tpl: WizardState['quotaTemplate']) => {
    setState(applyQuotaTemplate(state, tpl));
  };

  const custom = state.quotaTemplate === 'custom';

  return (
    <div>
      <h3>Step 3 / 4: 리소스 쿼터</h3>

      <label style={{ display: 'block', marginBottom: '8px' }}>
        템플릿
        <select
          value={state.quotaTemplate}
          onChange={(e) =>
            setTemplate(e.target.value as WizardState['quotaTemplate'])
          }
          style={{ width: '100%' }}
        >
          {QUOTA_TEMPLATES.map((t) => (
            <option key={t.id} value={t.id}>
              {t.label}
            </option>
          ))}
          <option value="custom">커스텀 (직접 편집)</option>
        </select>
      </label>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(160px, 1fr))',
          gap: '8px',
        }}
      >
        <label>
          CPU Requests
          <input
            value={state.cpuReq}
            onChange={(e) =>
              setState({
                ...state,
                cpuReq: e.target.value,
                quotaTemplate: custom ? 'custom' : state.quotaTemplate,
              })
            }
          />
        </label>
        <label>
          CPU Limits
          <input
            value={state.cpuLim}
            onChange={(e) =>
              setState({
                ...state,
                cpuLim: e.target.value,
                quotaTemplate: custom ? 'custom' : state.quotaTemplate,
              })
            }
          />
        </label>
        <label>
          Memory Req
          <input
            value={state.memReq}
            onChange={(e) =>
              setState({ ...state, memReq: e.target.value, quotaTemplate: custom ? 'custom' : state.quotaTemplate })
            }
          />
        </label>
        <label>
          Memory Lim
          <input
            value={state.memLim}
            onChange={(e) =>
              setState({ ...state, memLim: e.target.value, quotaTemplate: custom ? 'custom' : state.quotaTemplate })
            }
          />
        </label>
        <label>
          GPU Slots
          <input
            type="number"
            min={0}
            value={state.gpu}
            onChange={(e) =>
              setState({
                ...state,
                gpu: parseInt(e.target.value, 10) || 0,
                quotaTemplate: custom ? 'custom' : state.quotaTemplate,
              })
            }
          />
        </label>
        <label>
          Storage
          <input
            value={state.storage}
            onChange={(e) =>
              setState({ ...state, storage: e.target.value, quotaTemplate: custom ? 'custom' : state.quotaTemplate })
            }
          />
        </label>
        <label>
          Max Pods
          <input
            type="number"
            min={1}
            value={state.pods}
            onChange={(e) =>
              setState({
                ...state,
                pods: parseInt(e.target.value, 10) || 1,
                quotaTemplate: custom ? 'custom' : state.quotaTemplate,
              })
            }
          />
        </label>
        <label>
          LB Services
          <input
            type="number"
            min={0}
            value={state.lbServices}
            onChange={(e) =>
              setState({
                ...state,
                lbServices: parseInt(e.target.value, 10) || 0,
                quotaTemplate: custom ? 'custom' : state.quotaTemplate,
              })
            }
          />
        </label>
      </div>
    </div>
  );
}
