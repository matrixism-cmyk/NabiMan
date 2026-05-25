import React from 'react';
import { useMecList } from '../../../hooks/mec/useMecApi';
import { MecNode } from '../../../types/mec';
import { WizardState } from './WizardState';

interface Props {
  state: WizardState;
  setState: (s: WizardState) => void;
}

export default function Step2Node({ state, setState }: Props) {
  const { data, loading } = useMecList<MecNode>('/api/mec/v1/nodes');

  return (
    <div>
      <h3>Step 2 / 4: 노드 할당</h3>
      <div style={{ marginBottom: '12px' }}>
        <label style={{ marginRight: '16px' }}>
          <input
            type="radio"
            name="alloc"
            checked={state.allocationType === 'dedicated'}
            onChange={() => setState({ ...state, allocationType: 'dedicated' })}
          />{' '}
          단독 할당 (Taint + NoSchedule)
        </label>
        <label>
          <input
            type="radio"
            name="alloc"
            checked={state.allocationType === 'shared'}
            onChange={() => setState({ ...state, allocationType: 'shared' })}
          />{' '}
          공유 할당 (nodeSelector만)
        </label>
      </div>

      <h4>노드 선택 (단독 할당 시 필수)</h4>
      {loading && !data && <div className="panel-loading">노드 로딩 중...</div>}
      {data && (
        <table className="panel-table">
          <thead>
            <tr>
              <th>선택</th>
              <th>노드</th>
              <th>Role</th>
              <th>CPU</th>
              <th>Memory</th>
              <th>GPU</th>
              <th>현재 Tenant</th>
            </tr>
          </thead>
          <tbody>
            {data.map((n) => {
              const disabled =
                state.allocationType === 'dedicated' && !!n.current_tenant;
              return (
                <tr
                  key={n.name}
                  style={{ opacity: disabled ? 0.5 : 1 }}
                >
                  <td>
                    <input
                      type="radio"
                      name="node"
                      disabled={disabled}
                      checked={state.node === n.name}
                      onChange={() =>
                        setState({
                          ...state,
                          node: n.name,
                          gpuLabel: n.gpu_info?.model?.toLowerCase() || state.gpuLabel,
                        })
                      }
                    />
                  </td>
                  <td><code>{n.name}</code></td>
                  <td>{n.roles.join(', ')}</td>
                  <td>{n.capacity.cpu}</td>
                  <td>{n.capacity.memory}</td>
                  <td>
                    {n.gpu_info
                      ? `${n.gpu_info.model} × ${n.gpu_info.count}`
                      : '-'}
                  </td>
                  <td>
                    {n.current_tenant ? (
                      <span style={{ color: '#ef4444' }}>{n.current_tenant}</span>
                    ) : (
                      <span style={{ color: '#10b981' }}>가용</span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}

      <label style={{ display: 'block', marginTop: '16px' }}>
        GPU 라벨
        <input
          value={state.gpuLabel}
          onChange={(e) => setState({ ...state, gpuLabel: e.target.value })}
          placeholder="gh200 / a40 / t4"
          style={{ width: '200px' }}
        />
      </label>
    </div>
  );
}
