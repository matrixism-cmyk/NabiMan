import React, { useState } from 'react';
import { useT } from '../../i18n';

export interface AdhocTarget { host: string; port: number; user: string }

/** A one-off SSH target that is not worth saving as a server. */
export default function AdhocConnect({ onConnect }: { onConnect: (target: AdhocTarget) => void }) {
  const { t } = useT();
  const [form, setForm] = useState({ host: '', port: '22', user: 'root' });

  return (
    <div className="rw-view rw-view-scroll">
      <div className="rw-section">
        <div className="rw-section-head"><h3>{t('remote.adhoc')}</h3></div>
        <p className="text-secondary">{t('remote.adhocHint')}</p>
        <form className="filter-row" onSubmit={(e) => {
          e.preventDefault();
          if (!form.host.trim()) return;
          onConnect({
            host: form.host.trim(),
            port: parseInt(form.port, 10) || 22,
            user: form.user.trim() || 'root',
          });
        }}>
          <input className="filter-input" placeholder={t('terminal.hostPlaceholder')} value={form.host}
            onChange={e => setForm({ ...form, host: e.target.value })} required />
          <input className="filter-input" style={{ maxWidth: 90 }} placeholder={t('terminal.portPlaceholder')}
            value={form.port} onChange={e => setForm({ ...form, port: e.target.value })} />
          <input className="filter-input" style={{ maxWidth: 140 }} placeholder={t('terminal.userPlaceholder')}
            value={form.user} onChange={e => setForm({ ...form, user: e.target.value })} />
          <button type="submit" className="btn btn-primary btn-sm">{t('common.connect')}</button>
        </form>
      </div>
    </div>
  );
}
