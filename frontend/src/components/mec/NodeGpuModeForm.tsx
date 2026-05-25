import React, { useState } from 'react';

interface Props {
  currentMode: string;
  onSwitch: (mode: string, replicas?: number) => Promise<void>;
}

const MODES = [
  { value: 'container', label: 'Container (단독 할당)' },
  { value: 'time-slicing', label: 'Time-Slicing (공유)' },
  { value: 'mig', label: 'MIG (H100/A100)' },
  { value: 'vm-passthrough', label: 'VFIO Passthrough (VM)' },
];

export default function NodeGpuModeForm({ currentMode, onSwitch }: Props) {
  const [mode, setMode] = useState(currentMode);
  const [replicas, setReplicas] = useState(2);
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (mode === currentMode) {
      return;
    }
    if (
      !window.confirm(
        `GPU 모드를 ${currentMode} → ${mode} 로 전환합니다. 이 노드의 파드가 재스케줄될 수 있습니다. 계속하시겠습니까?`,
      )
    ) {
      return;
    }
    setBusy(true);
    try {
      await onSwitch(mode, mode === 'time-slicing' ? replicas : undefined);
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      onSubmit={submit}
      style={{ display: 'flex', gap: '8px', alignItems: 'flex-end', flexWrap: 'wrap' }}
    >
      <label style={{ flex: '1 1 180px' }}>
        <div style={{ fontSize: '11px', color: '#6b7280' }}>새 모드</div>
        <select
          value={mode}
          onChange={(e) => setMode(e.target.value)}
          style={{ width: '100%' }}
        >
          {MODES.map((m) => (
            <option key={m.value} value={m.value}>
              {m.label}
            </option>
          ))}
        </select>
      </label>
      {mode === 'time-slicing' && (
        <label style={{ flex: '0 0 120px' }}>
          <div style={{ fontSize: '11px', color: '#6b7280' }}>Replicas</div>
          <input
            type="number"
            min={1}
            max={16}
            value={replicas}
            onChange={(e) => setReplicas(parseInt(e.target.value, 10) || 1)}
            style={{ width: '100%' }}
          />
        </label>
      )}
      <button
        type="submit"
        className="btn btn-primary"
        disabled={busy || mode === currentMode}
      >
        {busy ? '전환 중...' : '전환'}
      </button>
    </form>
  );
}
