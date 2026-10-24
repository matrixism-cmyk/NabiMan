import React, { useState } from 'react';
import { useT } from '../../shared/i18n';
import { createConsole, updateConsole, VmConsole } from './vncApi';

type EditTarget = { id: string; label: string; host: string; port: number } | null;

/** Add or edit an external VNC console (label + host/domain + port). Internal
 *  KubeVirt consoles are detected automatically and are not edited here. */
export default function VncConsoleForm({ edit, onSaved, onCancel }: {
  edit: EditTarget;
  onSaved: (c: VmConsole) => void;
  onCancel: () => void;
}) {
  const { t } = useT();
  const [label, setLabel] = useState(edit?.label || '');
  const [host, setHost] = useState(edit?.host || '');
  const [port, setPort] = useState(String(edit?.port || 5900));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const p = parseInt(port, 10);
    if (!label.trim() || !host.trim() || !p) { setError(t('remote.console.form.invalid')); return; }
    setBusy(true); setError('');
    const res = edit ? await updateConsole(edit.id, label.trim(), host.trim(), p) : await createConsole(label.trim(), host.trim(), p);
    setBusy(false);
    if (res.success && res.data) onSaved(res.data);
    else setError(res.message || t('common.error'));
  };

  return (
    <div className="rw-view rw-view-scroll">
      <div className="rw-section">
        <div className="rw-section-head"><h3>{edit ? t('remote.console.form.editTitle') : t('remote.console.form.addTitle')}</h3></div>
        <p className="text-secondary">{t('remote.console.form.intro')}</p>
        <form className="rw-form" onSubmit={submit}>
          <div className="form-group"><label>{t('remote.console.form.label')} *</label>
            <input className="filter-input" value={label} onChange={e => setLabel(e.target.value)} placeholder="Windows VM" maxLength={80} required /></div>
          <div className="form-group"><label>{t('remote.console.form.host')} *</label>
            <input className="filter-input" value={host} onChange={e => setHost(e.target.value)} placeholder="10.0.0.5 / vm.example.com" required /></div>
          <div className="form-group"><label>{t('remote.console.form.port')} *</label>
            <input className="filter-input" type="number" min={1} max={65535} value={port} onChange={e => setPort(e.target.value)} placeholder="5900" required /></div>
          <p className="text-secondary" style={{ fontSize: 12 }}>{t('remote.console.form.note')}</p>
          {error && <p role="alert" className="message">{error}</p>}
          <div style={{ display: 'flex', gap: 8 }}>
            <button type="submit" className="btn btn-primary btn-sm" disabled={busy}>{edit ? t('common.save') : t('common.add')}</button>
            <button type="button" className="btn btn-secondary btn-sm" onClick={onCancel}>{t('common.cancel')}</button>
          </div>
        </form>
      </div>
    </div>
  );
}
