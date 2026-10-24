import React, { useState } from 'react';
import { useT } from '../../shared/i18n';

export interface AdhocTarget { host: string; port: number; user: string; proto: 'ssh' | 'telnet' }

/** A one-off target that is not worth saving as a server. SSH still needs the
 *  address registered and its host key verified; Telnet (no host key) connects
 *  to any address. */
export default function AdhocConnect({ onConnect }: { onConnect: (target: AdhocTarget) => void }) {
  const { t } = useT();
  const [form, setForm] = useState({ host: '', port: '22', user: 'root', proto: 'ssh' as 'ssh' | 'telnet' });

  const changeProto = (proto: 'ssh' | 'telnet') => {
    // Carry the default port over only while it is still the other protocol's default.
    const port = form.port === '' || form.port === '22' || form.port === '23' ? (proto === 'telnet' ? '23' : '22') : form.port;
    setForm({ ...form, proto, port });
  };

  return (
    <div className="rw-view rw-view-scroll">
      <div className="rw-section">
        <div className="rw-section-head"><h3>{t('remote.adhoc')}</h3></div>
        <p className="text-secondary">{t('remote.adhocHint')}</p>
        {form.proto === 'ssh'
          ? <p className="text-secondary" style={{ fontSize: 12 }}>{t('remote.adhoc.sshNote')}</p>
          : <p className="proto-warning" role="note">⚠ {t('remote.adhoc.telnetNote')}</p>}
        <form className="filter-row" onSubmit={(e) => {
          e.preventDefault();
          if (!form.host.trim()) return;
          onConnect({
            host: form.host.trim(),
            port: parseInt(form.port, 10) || (form.proto === 'telnet' ? 23 : 22),
            user: form.user.trim() || (form.proto === 'telnet' ? '' : 'root'),
            proto: form.proto,
          });
        }}>
          <select className="select-input" style={{ maxWidth: 110 }} value={form.proto}
            onChange={e => changeProto(e.target.value as 'ssh' | 'telnet')} aria-label={t('remote.protocol')}>
            <option value="ssh">SSH</option>
            <option value="telnet">Telnet</option>
          </select>
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
