import React, { useState } from 'react';
import { useT } from '../../i18n';

interface Props {
  currentMode: string;
  onSwitch: (mode: string, replicas?: number) => Promise<void>;
}

const MODES = [
  { value: 'container', labelKey: 'mec.node.gpuMode.container' },
  { value: 'time-slicing', labelKey: 'mec.node.gpuMode.timeSlicing' },
  { value: 'mig', labelKey: 'mec.node.gpuMode.mig' },
  { value: 'vm-passthrough', labelKey: 'mec.node.gpuMode.vmPassthrough' },
];

export default function NodeGpuModeForm({ currentMode, onSwitch }: Props) {
  const { t } = useT();
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
        t('mec.node.gpuMode.confirm', { from: currentMode, to: mode }),
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
        <div style={{ fontSize: '11px', color: '#6b7280' }}>{t('mec.node.gpuMode.newMode')}</div>
        <select
          value={mode}
          onChange={(e) => setMode(e.target.value)}
          style={{ width: '100%' }}
        >
          {MODES.map((m) => (
            <option key={m.value} value={m.value}>
              {t(m.labelKey)}
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
        {busy ? t('mec.node.gpuMode.switching') : t('mec.node.gpuMode.switch')}
      </button>
    </form>
  );
}
