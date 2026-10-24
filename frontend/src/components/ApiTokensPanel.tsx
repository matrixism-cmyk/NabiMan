import React, { useState } from 'react';
import { apiPost, apiRequest, useApi } from '../shared/api/useApi';
import { useT } from '../i18n';
import { confirmDialog } from '../shared/ui/ConfirmDialog';
interface Token { id: string; name: string; prefix: string; owner_name: string | null; scopes: string[]; status: string; expires_at: string | null; last_used: string | null }
interface Issued { id: string; key: string; expires_at: string }
const scopes = ['status:read', 'servers:read', 'sessions:read', 'tenants:read'];

/** The public read API a key unlocks: which scope reaches which GET endpoint. */
const ENDPOINTS: { scope: string; method: string; path: string }[] = [
  { scope: 'status:read', method: 'GET', path: '/api/server/status' },
  { scope: 'servers:read', method: 'GET', path: '/api/remote-servers' },
  { scope: 'sessions:read', method: 'GET', path: '/api/terminal/sessions' },
  { scope: 'tenants:read', method: 'GET', path: '/api/mec/v1/tenants' },
];

/** A short developer reference so a key is actually usable: base URL, how to
 *  authenticate, and the exact scope→endpoint map with a copy-paste example. */
function ApiDocs() {
  const { t } = useT();
  const base = typeof window !== 'undefined' ? window.location.origin : 'https://your-nabiman';
  const curl = `curl -H "Authorization: Bearer nbm_…" \\\n     ${base}/api/server/status`;
  return (
    <details className="api-docs" style={{ margin: '12px 0', border: '1px solid var(--border)', borderRadius: 8, padding: '8px 12px' }}>
      <summary style={{ cursor: 'pointer', fontWeight: 600 }}>{t('tokens.docs.title')}</summary>
      <p style={{ fontSize: 13 }}>{t('tokens.docs.auth')}</p>
      <pre style={{ fontSize: 12, whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', background: 'var(--bg-secondary, rgba(127,127,127,.1))', padding: 8, borderRadius: 6 }}>{`Authorization: Bearer nbm_…
X-API-Key: nbm_…   ${t('tokens.docs.orHeader')}`}</pre>
      <table className="data-table" style={{ fontSize: 13 }}>
        <thead><tr><th>{t('tokens.scopes')}</th><th>{t('tokens.docs.endpoint')}</th></tr></thead>
        <tbody>{ENDPOINTS.map(e => (
          <tr key={e.scope}><td><code>{e.scope}</code></td><td><code>{e.method} {e.path}</code></td></tr>
        ))}</tbody>
      </table>
      <p style={{ fontSize: 13 }}>{t('tokens.docs.example')}</p>
      <pre style={{ fontSize: 12, whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', background: 'var(--bg-secondary, rgba(127,127,127,.1))', padding: 8, borderRadius: 6 }}>{curl}</pre>
      <p style={{ fontSize: 12 }}>{t('tokens.docs.notes')} <a href="/api/openapi.json" target="_blank" rel="noreferrer">/api/openapi.json</a></p>
    </details>
  );
}
export default function ApiTokensPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<Token[]>('/api/apikeys');
  const [name, setName] = useState(''); const [days, setDays] = useState(7);
  const [selected, setSelected] = useState<string[]>(['status:read']);
  const [issued, setIssued] = useState<Issued | null>(null); const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const create = async (event: React.FormEvent) => {
    event.preventDefault(); setBusy(true); setIssued(null); setMessage('');
    try {
      const result = await apiPost<Issued>('/api/apikeys', { name: name.trim(), scopes: selected, expires_in_days: days });
      if (!result.success || !result.data) throw new Error(result.message);
      setIssued(result.data); setName(''); refetch();
    } catch (cause) { setMessage(cause instanceof Error ? cause.message : t('common.error')); }
    finally { setBusy(false); }
  };
  const revoke = async (token: Token) => {
    if (!(await confirmDialog({ message: `${t('tokens.confirm')}\n${token.name} · ${token.prefix}`, danger: true }))) return;
    setBusy(true); setMessage('');
    try {
      const result = await apiRequest(`/api/apikeys/${encodeURIComponent(token.id)}`, { method: 'DELETE' });
      if (!result.success) throw new Error(result.message);
      if (issued?.id === token.id) setIssued(null);
      setMessage(t('tokens.revoked')); refetch();
    } catch (cause) { setMessage(cause instanceof Error ? cause.message : t('common.error')); }
    finally { setBusy(false); }
  };
  return <section style={{ textAlign: 'left' }} aria-label={t('tokens.title')}>
    <h3>{t('tokens.title')}</h3><p>{t('tokens.description')}</p><p>{t('tokens.usage')}</p><p><a href="/automation-auth.openapi.json" target="_blank" rel="noreferrer">{t('tokens.contract')}</a></p>
    <ApiDocs />
    {(error || message) && <p role="alert">{error || message}</p>}
    {issued && <div role="status"><strong>{t('apikeys.newKeyWarning')}</strong><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }} data-testid="issued-token">{issued.key}</pre><p>{t('apikeys.expires')}: {issued.expires_at}</p><button className="btn btn-secondary" onClick={() => setIssued(null)}>{t('apikeys.dismiss')}</button></div>}
    {loading ? <p>{t('common.loading')}</p> : <div style={{ overflowX: 'auto' }}><table className="data-table"><thead><tr>
      <th>{t('apikeys.name')}</th><th>{t('tokens.owner')}</th><th>{t('tokens.scopes')}</th><th>{t('tokens.state')}</th><th>{t('apikeys.expires')}</th><th>{t('apikeys.lastUsed')}</th><th>{t('common.actions')}</th>
    </tr></thead><tbody>{data?.map(token => <tr key={token.id}>
      <td>{token.name}<br /><code>{token.prefix}…</code></td><td>{token.owner_name || '—'}</td><td>{token.scopes.join(', ') || '—'}</td><td>{t(`tokens.${token.status}`)}</td><td>{token.expires_at || '—'}</td><td>{token.last_used || '—'}</td>
      <td><button className="btn btn-danger" disabled={busy} onClick={() => revoke(token)}>{t('apikeys.revoke')}</button></td>
    </tr>)}</tbody></table></div>}
    <form onSubmit={create} style={{ display: 'grid', gap: 12, maxWidth: 680, marginTop: 20 }}>
      <label>{t('apikeys.name')}<input className="form-input" aria-label={t('apikeys.name')} required maxLength={100} value={name} onChange={e => setName(e.target.value)} /></label>
      <label>{t('tokens.days')}<input className="form-input" aria-label={t('tokens.days')} type="number" min={1} max={90} required value={days} onChange={e => setDays(Number(e.target.value))} /></label>
      <fieldset><legend>{t('tokens.scopes')}</legend>{scopes.map(scope => <label key={scope} style={{ display: 'block' }}><input type="checkbox" checked={selected.includes(scope)} onChange={e => setSelected(e.target.checked ? [...selected, scope] : selected.filter(s => s !== scope))} /> {t(`tokens.${scope}`)} <code>{scope}</code></label>)}</fieldset>
      <button className="btn btn-primary" disabled={busy || !data || Boolean(error) || !name.trim() || !selected.length || days < 1 || days > 90}>{t('apikeys.generateBtn')}</button>
    </form>
  </section>;
}
