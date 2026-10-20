import React, { useState } from 'react';
import { apiRequest } from '../../hooks/useApi';
import { RemoteServer } from '../../types';
import { useT } from '../../i18n';

/** `onSaved` says whether a key is now approved (false after a revoke). */
export default function HostTrustForm({ server, onSaved }: { server: RemoteServer; onSaved: (approved: boolean) => void }) {
  const { t } = useT();
  const [key, setKey] = useState(server.host_trust?.public_key || '');
  const [fingerprint, setFingerprint] = useState(server.host_trust?.fingerprint || '');
  const [reason, setReason] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const prefix = `host-trust-${server.id}`;
  const save = async (revoke: boolean) => {
    if (!reason.trim() || (!revoke && (!key.trim() || !fingerprint.trim()))) {
      setMessage(t('remote.trust.required')); return;
    }
    setBusy(true);
    try {
      const res = await apiRequest<RemoteServer>(`/api/remote-servers/${server.id}/host-trust`, {
        method: 'PUT', body: JSON.stringify({
          host: server.host, port: server.port,
          public_key: revoke ? '' : key.trim(), fingerprint: revoke ? '' : fingerprint.trim(),
          previous_fingerprint: server.host_trust?.fingerprint || '',
          previous_version: server.host_trust_version || '', reason: reason.trim(),
        }),
      });
      if (!res.success) { setMessage(res.message); return; }
      setMessage(t('remote.trust.success')); onSaved(!revoke);
    } catch (error) { setMessage(error instanceof Error ? error.message : t('common.error')); }
    finally { setBusy(false); }
  };
  return <section className="rw-section" aria-labelledby={`${prefix}-heading`}>
    <h3 id={`${prefix}-heading`}>{t('remote.trust.title')}</h3>
    <p>{t('remote.trust.hint')}</p>
    <p>{server.host_trust ? `${t('remote.trust.verified')}: ${server.host_trust.verified_by} · ${server.host_trust.verified_at}` : t('remote.trust.unverified')}</p>
    {message && <p role="status">{message}</p>}
    <div className="form-group"><label htmlFor={`${prefix}-key`}>{t('remote.trust.key')}</label>
      <textarea id={`${prefix}-key`} value={key} onChange={e => setKey(e.target.value)} maxLength={8192} className="filter-input" placeholder="ssh-ed25519 AAAA…" /></div>
    <div className="form-group"><label htmlFor={`${prefix}-fingerprint`}>{t('remote.trust.fingerprint')}</label>
      <input id={`${prefix}-fingerprint`} value={fingerprint} onChange={e => setFingerprint(e.target.value)} className="filter-input" placeholder="SHA256:…" /></div>
    <div className="form-group"><label htmlFor={`${prefix}-reason`}>{t('remote.trust.reason')}</label>
      <input id={`${prefix}-reason`} value={reason} onChange={e => setReason(e.target.value)} maxLength={2000} className="filter-input" /></div>
    <div className="btn-group">
      <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => save(false)}>{t('remote.trust.save')}</button>
      {server.host_trust && <button type="button" className="btn btn-danger btn-sm" disabled={busy} onClick={() => save(true)}>{t('remote.trust.revoke')}</button>}
    </div>
  </section>;
}
