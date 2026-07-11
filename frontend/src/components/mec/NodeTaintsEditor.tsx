import React, { useState } from 'react';
import { useT } from '../../i18n';
import { Taint } from '../../types/mec';

interface Props {
  taints: Taint[];
  onPatch: (
    add: Taint[],
    remove: { key: string; effect?: string }[],
  ) => Promise<void>;
}

export default function NodeTaintsEditor({ taints, onPatch }: Props) {
  const { t } = useT();
  const [key, setKey] = useState('tenant');
  const [value, setValue] = useState('');
  const [effect, setEffect] = useState<Taint['effect']>('NoSchedule');
  const [busy, setBusy] = useState(false);

  const add = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!key) return;
    setBusy(true);
    try {
      await onPatch(
        [{ key, value: value || null, effect }],
        [],
      );
      setValue('');
    } finally {
      setBusy(false);
    }
  };

  const remove = async (taintToRemove: Taint) => {
    if (
      !window.confirm(
        t('mec.node.taints.confirmRemove', {
          taint: `${taintToRemove.key}${taintToRemove.value ? '=' + taintToRemove.value : ''}:${taintToRemove.effect}`,
        }),
      )
    )
      return;
    setBusy(true);
    try {
      await onPatch([], [{ key: taintToRemove.key, effect: taintToRemove.effect }]);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <table className="panel-table" style={{ marginBottom: '12px' }}>
        <thead>
          <tr>
            <th>Key</th>
            <th>Value</th>
            <th>Effect</th>
            <th>{t('mec.node.taints.actions')}</th>
          </tr>
        </thead>
        <tbody>
          {taints.length === 0 ? (
            <tr>
              <td colSpan={4} style={{ color: '#6b7280', fontStyle: 'italic' }}>
                {t('mec.node.taints.empty')}
              </td>
            </tr>
          ) : (
            taints.map((taint, i) => (
              <tr key={`${taint.key}-${i}`}>
                <td><code>{taint.key}</code></td>
                <td>{taint.value || '-'}</td>
                <td>{taint.effect}</td>
                <td>
                  <button
                    className="btn btn-danger btn-small"
                    disabled={busy}
                    onClick={() => remove(taint)}
                  >
                    {t('mec.node.taints.remove')}
                  </button>
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>

      <form
        onSubmit={add}
        style={{ display: 'flex', gap: '8px', alignItems: 'flex-end', flexWrap: 'wrap' }}
      >
        <label style={{ flex: '1 1 140px' }}>
          <div style={{ fontSize: '11px', color: '#6b7280' }}>Key</div>
          <input
            required
            value={key}
            onChange={(e) => setKey(e.target.value)}
            style={{ width: '100%' }}
          />
        </label>
        <label style={{ flex: '1 1 140px' }}>
          <div style={{ fontSize: '11px', color: '#6b7280' }}>{t('mec.node.taints.valueOptional')}</div>
          <input
            value={value}
            onChange={(e) => setValue(e.target.value)}
            style={{ width: '100%' }}
          />
        </label>
        <label style={{ flex: '0 0 160px' }}>
          <div style={{ fontSize: '11px', color: '#6b7280' }}>Effect</div>
          <select
            value={effect}
            onChange={(e) => setEffect(e.target.value as Taint['effect'])}
            style={{ width: '100%' }}
          >
            <option value="NoSchedule">NoSchedule</option>
            <option value="PreferNoSchedule">PreferNoSchedule</option>
            <option value="NoExecute">NoExecute</option>
          </select>
        </label>
        <button type="submit" className="btn btn-primary" disabled={busy}>
          {busy ? t('mec.node.taints.applying') : t('mec.node.taints.addBtn')}
        </button>
      </form>
    </div>
  );
}
