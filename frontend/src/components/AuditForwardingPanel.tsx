import React, { useState } from 'react';
import { apiPost, useApi } from '../hooks/useApi';
import { useT } from '../i18n';
interface Destination { id: string; revision: string; name: string; url: string; enabled: boolean; has_auth: boolean; pending: number; blocked: number; last_error: string | null; acknowledged: number; last_ack_at: string | null }
interface Status { destinations: Destination[]; counters: Record<string, number>; legacy_requires_review: boolean; delivery_paused_by_operator: boolean }
export default function AuditForwardingPanel() {
  const { t } = useT(); const { data, error: loadError, refetch } = useApi<Status>('/api/audit/forwarding', 10000);
  const [name, setName] = useState(''); const [url, setUrl] = useState(''); const [auth, setAuth] = useState('');
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [info, setInfo] = useState('');
  const create = async (event: React.FormEvent) => {
    event.preventDefault(); const secret = auth; setAuth(''); setBusy(true); setError(''); setInfo('');
    try { const result = await apiPost('/api/audit/forwarding', { name, url, auth_header: secret }); if (!result.success) throw new Error(result.message); setName(''); setUrl(''); setInfo(t('forward.saved')); refetch(); }
    catch (cause) { setError(cause instanceof Error ? cause.message : t('common.error')); } finally { setBusy(false); }
  };
  const action = async (destination: Destination, kind: string) => {
    if (['enable', 'disable', 'delete'].includes(kind) && !window.confirm(`${t('forward.confirm')}\n${destination.name}\n${destination.url}`)) return;
    setBusy(true); setError(''); setInfo('');
    try { const result = await apiPost(`/api/audit/forwarding/${encodeURIComponent(destination.id)}/${kind}`, { revision: destination.revision }); if (!result.success) throw new Error(result.message); setInfo(t(kind === 'test' ? 'forward.queued' : 'forward.changed')); refetch(); }
    catch (cause) { setError(cause instanceof Error ? cause.message : t('common.error')); } finally { setBusy(false); }
  };
  return <section aria-label={t('forward.title')} style={{ textAlign: 'left', marginTop: 24 }}>
    <h3>{t('forward.title')}</h3><p>{t('forward.description')}</p><p>{t('forward.limits')}</p>
    {(loadError || error) && <p role="alert">{loadError || error}</p>}{info && <p role="status">{info}</p>}
    {data?.legacy_requires_review && <p role="alert">{t('forward.legacy')}</p>}
    {data?.delivery_paused_by_operator && <p role="alert">{t('forward.paused')}</p>}
    {Boolean(data?.counters.capture_failed) && <p role="alert">{t('forward.gaps')} ({data?.counters.capture_failed})</p>}
    {data && <p>{t('forward.ack')}: {data.counters.acknowledged} · {t('forward.cancelled')}: {data.counters.cancelled}</p>}
    <form onSubmit={create} style={{ display: 'grid', gap: 8, maxWidth: 650 }}>
      <label>{t('forward.name')}<input className="form-input" aria-label={t('forward.name')} required maxLength={80} value={name} onChange={e => setName(e.target.value)} /></label>
      <label>{t('forward.url')}<input className="form-input" aria-label={t('forward.url')} type="url" required placeholder="https://collector.example/events" value={url} onChange={e => setUrl(e.target.value)} /></label>
      <label>{t('forward.secret')}<input className="form-input" aria-label={t('forward.secret')} type="password" autoComplete="new-password" maxLength={4096} placeholder="Bearer …" value={auth} onChange={e => setAuth(e.target.value)} /></label>
      <button className="btn btn-secondary" disabled={busy || !data || Boolean(loadError)}>{t('forward.add')}</button>
    </form>
    {data && <div style={{ overflowX: 'auto' }}><table className="data-table"><thead><tr><th>{t('forward.name')}</th><th>{t('forward.state')}</th><th>{t('forward.pending')}</th><th>{t('forward.ack')}</th><th>{t('forward.actions')}</th></tr></thead><tbody>
      {data.destinations.map(d => <tr key={d.id}><td><strong>{d.name}</strong><br /><code>{d.url}</code></td><td>{t(d.enabled ? 'common.enabled' : 'common.disabled')}<br />{d.last_error}</td><td>{d.pending} / {d.blocked}</td><td>{d.acknowledged}<br />{d.last_ack_at}</td><td>
        <button className="btn btn-secondary btn-sm" disabled={busy || data.delivery_paused_by_operator} onClick={() => action(d, 'test')}>{t('forward.test')}</button>
        <button className="btn btn-secondary btn-sm" disabled={busy} onClick={() => action(d, d.enabled ? 'disable' : 'enable')}>{t(d.enabled ? 'forward.disable' : 'forward.enable')}</button>
        <button className="btn btn-secondary btn-sm" disabled={busy || !d.blocked} onClick={() => action(d, 'retry')}>{t('forward.retry')}</button>
        <button className="btn btn-danger btn-sm" disabled={busy} onClick={() => action(d, 'delete')}>{t('forward.delete')}</button>
      </td></tr>)}
    </tbody></table></div>}
  </section>;
}
