import React, { useEffect, useState } from 'react';
import { fetchWithRefresh } from '../shared/api/useApi';
import { MemberUser } from './MembershipEditor';
import { useT } from '../i18n';
import './membership-management.css';
import { formatDateTime } from '../shared/i18n/format';
interface Link { remote_user_id: string; remote_username: string; verified_at: string; linked_by: string; reason: string }
interface Snapshot { realm: string; link: Link | null }
export default function ExternalIdentityEditor({ user, onClose }: { user: MemberUser; onClose: () => void }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [remoteId, setRemoteId] = useState('');
  const [reason, setReason] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const { t } = useT();
  // Holds a translation key, so a language change re-words a shown error.
  const endpoint = `/api/users/${encodeURIComponent(user.id)}/external-identities/rancher`;
  useEffect(() => {
    const controller = new AbortController();
    setSnapshot(null); setError(''); setRemoteId(''); setReason('');
    (async () => {
      try {
        const response = await fetchWithRefresh(endpoint, { signal: controller.signal });
        const body = await response.json();
        if (!response.ok || body.error || !body.data?.realm) throw new Error();
        if (!controller.signal.aborted) setSnapshot(body.data);
      } catch { if (!controller.signal.aborted) setError('identity.loadFailed'); }
    })();
    return () => controller.abort();
  }, [endpoint]);
  return <section className="inline-form external-identity-editor" aria-label={t('identity.title')}>
    <h3>{t('identity.heading', { name: user.username })}</h3>
    <p>{t('identity.lead')}</p>
    {error && <p role="alert">{t(error)}</p>}
    {!snapshot && !error && <p>{t('identity.loading')}</p>}
    {snapshot && <>
      <p>{t('identity.realm')}<code>{snapshot.realm}</code></p>
      {snapshot.link ? <dl className="external-identity-details">
        <dt>{t('identity.linked')}</dt><dd>{snapshot.link.remote_username} · {snapshot.link.remote_user_id}</dd>
        <dt>{t('identity.verified')}</dt><dd>{formatDateTime(snapshot.link.verified_at)} · {snapshot.link.linked_by}</dd>
        <dt>{t('identity.reason')}</dt><dd>{snapshot.link.reason}</dd>
      </dl> : <form onSubmit={async e => {
        e.preventDefault(); setBusy(true); setError('');
        try {
          const response = await fetchWithRefresh(endpoint, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ realm: snapshot.realm, remote_user_id: remoteId, reason }) });
          const body = await response.json();
          if (!response.ok || body.error || !body.data?.remote_user_id) throw new Error();
          setSnapshot({ ...snapshot, link: body.data });
        } catch { setError('identity.linkFailed'); }
        finally { setBusy(false); }
      }}>
        <label>{t('identity.accountId')}<input required maxLength={256} value={remoteId} onChange={e => setRemoteId(e.target.value)} /></label>
        <label>{t('identity.reasonInput')}<input required minLength={8} maxLength={1000} value={reason} onChange={e => setReason(e.target.value)} /></label>
        <button className="btn btn-primary" disabled={busy}>{t('identity.submit')}</button>
      </form>}
    </>}
    <button className="btn btn-secondary" disabled={busy} onClick={onClose}>{t('common.close')}</button>
  </section>;
}
