import React, { useState } from 'react';
import { apiPost, apiRequest, useApi } from '../hooks/useApi';
import { useT } from '../i18n';
interface Token { id: string; name: string; prefix: string; owner_name: string | null; scopes: string[]; status: string; expires_at: string | null; last_used: string | null }
interface Issued { id: string; key: string; expires_at: string }
const scopes = ['status:read', 'servers:read', 'tenants:read'];
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
    if (!window.confirm(`${t('tokens.confirm')}\n${token.name} · ${token.prefix}`)) return;
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
