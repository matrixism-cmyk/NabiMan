import React, { useCallback, useEffect, useState } from 'react';
import { apiPost, apiRequest } from '../../shared/api/useApi';
import { RemoteServer } from './remoteTypes';
import { useT } from '../../shared/i18n';

/** `onSaved` says whether a key is now approved (false after a revoke). */
export default function HostTrustForm({ server, onSaved }: { server: RemoteServer; onSaved: (approved: boolean) => void }) {
  const { t } = useT();
  const [key, setKey] = useState(server.host_trust?.public_key || '');
  const [fingerprint, setFingerprint] = useState(server.host_trust?.fingerprint || '');
  const [reason, setReason] = useState('');
  const [legacy, setLegacy] = useState(Boolean(server.legacy_ssh));
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const prefix = `host-trust-${server.id}`;
  // The key the server presents fills the form; registering it stays a choice
  // made after comparing the fingerprint. A server not yet trusted is asked at once.
  const fetchKey = useCallback(async () => {
    setBusy(true);
    try {
      const res = await apiPost<{ public_key: string; fingerprint: string; legacy?: boolean }>(`/api/remote-servers/${server.id}/host-key`, {});
      if (!res.success || !res.data) { setMessage(res.message || t('common.error')); return; }
      setKey(res.data.public_key);
      setFingerprint(res.data.fingerprint);
      setLegacy(Boolean(res.data.legacy));
      setReason(r => r || t('remote.trust.fetchedReason'));
      setMessage(t('remote.trust.fetched', { fingerprint: res.data.fingerprint }));
    } catch (error) { setMessage(error instanceof Error ? error.message : t('common.error')); }
    finally { setBusy(false); }
  }, [server.id, t]);
  const untrusted = !server.host_trust;
  useEffect(() => { if (untrusted) fetchKey(); }, [untrusted, fetchKey]);
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
          previous_version: server.host_trust_version || '', reason: reason.trim(), legacy: revoke ? false : legacy,
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
    {legacy && <p className="text-warning">{t('remote.trust.legacy')}</p>}
    <div className="form-group"><label htmlFor={`${prefix}-key`}>{t('remote.trust.key')}</label>
      <textarea id={`${prefix}-key`} value={key} onChange={e => setKey(e.target.value)} maxLength={8192} className="filter-input" placeholder="ssh-ed25519 AAAA…" /></div>
    <div className="form-group"><label htmlFor={`${prefix}-fingerprint`}>{t('remote.trust.fingerprint')}</label>
      <input id={`${prefix}-fingerprint`} value={fingerprint} onChange={e => setFingerprint(e.target.value)} className="filter-input" placeholder="SHA256:…" /></div>
    <div className="form-group"><label htmlFor={`${prefix}-reason`}>{t('remote.trust.reason')}</label>
      <input id={`${prefix}-reason`} value={reason} onChange={e => setReason(e.target.value)} maxLength={2000} className="filter-input" /></div>
    <div className="btn-group">
      <button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={fetchKey}>{t('remote.trust.fetch')}</button>
      <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => save(false)}>{t('remote.trust.save')}</button>
      {server.host_trust && <button type="button" className="btn btn-danger btn-sm" disabled={busy} onClick={() => save(true)}>{t('remote.trust.revoke')}</button>}
    </div>
  </section>;
}
