import DelegatedManagedGrant from './mec/DelegatedManagedGrant';
import HostTrustForm from './remote/HostTrustForm';
import { useT } from '../i18n';
import React, { useState } from 'react';
import { apiPost, useApi } from '../shared/api/useApi';
import { useMecList, mecGet } from '../hooks/mec/useMecApi';
import { RemoteServer, RemoteServerStatus } from './remote/remoteTypes';
import { accessActive } from './MemberAccessFields';
import { MemberUser, membershipRole } from './MembershipEditor';

export default function ScopeResourcesPanel({ kind }: { kind: 'server' | 'tenant' }) {
  const { t } = useT();
  const [grantTenantId, setGrantTenantId] = useState('');
  const [trustId, setTrustId] = useState('');
  const { data: me } = useApi<MemberUser>('/api/users/me');
  const { data: servers, error: serverError, refetch: reloadServers } = useApi<RemoteServer[]>(kind === 'server' ? '/api/remote-servers' : '');
  const { data: tenants, error: tenantError } = useMecList<{ id: string; display_name: string; namespace: string }>(kind === 'tenant' ? '/api/mec/v1/tenants' : '');
  const [busy, setBusy] = useState(false);
  const [output, setOutput] = useState('');
  const [selected, setSelected] = useState('');
  const [command, setCommand] = useState('');
  const role = (id: string) => me?.memberships.find(m => m.kind === kind && m.scope_id === id && accessActive(m))?.role;
  const check = async (id: string) => {
    setBusy(true); setOutput('');
    try {
      if (kind === 'server') {
        const res = await apiPost<RemoteServerStatus>(`/api/remote-servers/${id}/check`, {});
        setOutput(res.success ? JSON.stringify(res.data, null, 2) : res.message);
      } else {
        const res = await mecGet<unknown>(`/api/mec/v1/tenants/${id}/resources`);
        setOutput(res.error ? res.error.message : JSON.stringify(res.data, null, 2));
      }
    } catch { setOutput(t('scope.resources.statusFailed')); }
    finally { setBusy(false); }
  };
  const exec = async (e: React.FormEvent) => {
    e.preventDefault(); setBusy(true); setOutput('');
    try { const res = await apiPost<string>(`/api/remote-servers/${selected}/exec`, { command }); setOutput(res.success ? (res.data || '') : res.message); }
    catch { setOutput(t('scope.resources.execFailed')); }
    finally { setBusy(false); }
  };
  const rows = kind === 'server' ? (servers || []).map(s => ({ id: s.id, name: s.name, detail: `${s.host}:${s.port}` }))
    : (tenants || []).map(x => ({ id: x.id, name: x.display_name, detail: x.namespace }));
  return <div className="panel">
    <h2>{kind === 'server' ? t('scope.resources.serverTitle') : t('scope.resources.tenantTitle')}</h2>
    <p><a href="#administration/scopeSettings">{t('scope.resources.settingsLink')}</a></p>
    <p>{t('scope.resources.lead')}</p>
    {(serverError || tenantError) && <div role="alert" className="message">{serverError || tenantError}</div>}
    <table className="data-table"><thead><tr><th>{t('common.name')}</th><th>{kind === 'server' ? t('scope.resources.host') : 'namespace'}</th><th>{t('scope.resources.myRole')}</th><th>{t('scope.resources.manage')}</th></tr></thead>
      <tbody>{rows.map(r => <tr key={r.id}>
        <td>{r.name}</td><td>{r.detail}</td><td>{t(membershipRole[role(r.id) || 'viewer'])}</td>
        <td><div className="btn-group">
          {kind === 'tenant' && role(r.id) === 'admin' && <button className="btn btn-sm btn-primary" onClick={() => setGrantTenantId(r.id)}>{t('mec.delegation.open')}</button>}
          {kind === 'server' && role(r.id) === 'admin' && <button className="btn btn-sm btn-secondary" onClick={() => setTrustId(r.id)}>{t('remote.trust.title')}</button>}
          <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => check(r.id)}>{t('scope.resources.check')}</button>
          {kind === 'server' && role(r.id) && role(r.id) !== 'viewer' && me?.memberships.find(m => m.kind === kind && m.scope_id === r.id)?.allow_exec !== false && <button className="btn btn-sm btn-primary" onClick={() => setSelected(r.id)}>{t('scope.resources.exec')}</button>}
        </div></td>
      </tr>)}</tbody>
    </table>
    {rows.length === 0 && <p>{t('scope.resources.none')}</p>}
    {kind === 'tenant' && grantTenantId && role(grantTenantId) === 'admin' && <div><button className="btn btn-secondary" onClick={() => setGrantTenantId('')}>{t('mec.tenant.cancel')}</button><DelegatedManagedGrant key={grantTenantId} tenantId={grantTenantId} /></div>}
    {selected && <form className="inline-form" onSubmit={exec}>
      <label>{t('scope.resources.commandFor', { name: rows.find(r => r.id === selected)?.name || '' })}
        <input aria-label={t('scope.resources.commandLabel')} required value={command} onChange={e => setCommand(e.target.value)} />
      </label>
      <button className="btn btn-primary" disabled={busy}>{t('common.run')}</button>
      <button type="button" className="btn btn-secondary" onClick={() => setSelected('')}>{t('common.close')}</button>
    </form>}
    {kind === 'server' && servers?.filter(s => s.id === trustId && role(s.id) === 'admin').map(s => <HostTrustForm key={`${s.id}:${s.host_trust?.fingerprint || ''}`} server={s} onSaved={() => { reloadServers(); setTrustId(''); }} />)}
    {output && <pre className="config-preview" role="status">{output}</pre>}
  </div>;
}
