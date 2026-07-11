import React, { useState } from 'react';
import { useT } from '../../i18n';

interface Props {
  labels: Record<string, string>;
  onPatch: (add: Record<string, string>, remove: string[]) => Promise<void>;
}

export default function NodeLabelsEditor({ labels, onPatch }: Props) {
  const { t } = useT();
  const [key, setKey] = useState('');
  const [value, setValue] = useState('');
  const [busy, setBusy] = useState(false);

  const add = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!key) return;
    setBusy(true);
    try {
      await onPatch({ [key]: value }, []);
      setKey('');
      setValue('');
    } finally {
      setBusy(false);
    }
  };

  const remove = async (k: string) => {
    if (!window.confirm(t('mec.node.labels.confirmRemove', { key: k }))) return;
    setBusy(true);
    try {
      await onPatch({}, [k]);
    } finally {
      setBusy(false);
    }
  };

  const entries = Object.entries(labels).sort((a, b) => a[0].localeCompare(b[0]));

  return (
    <div>
      <table className="panel-table" style={{ marginBottom: '12px' }}>
        <thead>
          <tr>
            <th>Key</th>
            <th>Value</th>
            <th>{t('mec.node.labels.actions')}</th>
          </tr>
        </thead>
        <tbody>
          {entries.length === 0 ? (
            <tr>
              <td colSpan={3} style={{ color: '#6b7280', fontStyle: 'italic' }}>
                {t('mec.node.labels.empty')}
              </td>
            </tr>
          ) : (
            entries.map(([k, v]) => (
              <tr key={k}>
                <td><code>{k}</code></td>
                <td>{v}</td>
                <td>
                  <button
                    className="btn btn-danger btn-small"
                    disabled={busy || isSystemLabel(k)}
                    onClick={() => remove(k)}
                    title={isSystemLabel(k) ? t('mec.node.labels.systemLabelTip') : ''}
                  >
                    {t('mec.node.labels.remove')}
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
        <label style={{ flex: '1 1 200px' }}>
          <div style={{ fontSize: '11px', color: '#6b7280' }}>Key</div>
          <input
            required
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder="e.g. dedicated"
            style={{ width: '100%' }}
          />
        </label>
        <label style={{ flex: '1 1 200px' }}>
          <div style={{ fontSize: '11px', color: '#6b7280' }}>Value</div>
          <input
            value={value}
            onChange={(e) => setValue(e.target.value)}
            placeholder="e.g. ygram-poc"
            style={{ width: '100%' }}
          />
        </label>
        <button type="submit" className="btn btn-primary" disabled={busy}>
          {busy ? t('mec.node.labels.applying') : t('mec.node.labels.addBtn')}
        </button>
      </form>
    </div>
  );
}

function isSystemLabel(k: string): boolean {
  return (
    k.startsWith('kubernetes.io/') ||
    k.startsWith('node.kubernetes.io/') ||
    k.startsWith('node-role.kubernetes.io/') ||
    k.startsWith('beta.kubernetes.io/') ||
    k.startsWith('topology.kubernetes.io/')
  );
}
