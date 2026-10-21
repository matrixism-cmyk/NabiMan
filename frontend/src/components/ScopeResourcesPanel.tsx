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
    } catch { setOutput('상태를 조회하지 못했습니다.'); }
    finally { setBusy(false); }
  };
  const exec = async (e: React.FormEvent) => {
    e.preventDefault(); setBusy(true); setOutput('');
    try { const res = await apiPost<string>(`/api/remote-servers/${selected}/exec`, { command }); setOutput(res.success ? (res.data || '') : res.message); }
    catch { setOutput('명령을 실행하지 못했습니다.'); }
    finally { setBusy(false); }
  };
  const rows = kind === 'server' ? (servers || []).map(s => ({ id: s.id, name: s.name, detail: `${s.host}:${s.port}` }))
    : (tenants || []).map(t => ({ id: t.id, name: t.display_name, detail: t.namespace }));
  return <div className="panel">
    <h2>{kind === 'server' ? '내 소속 서버' : '내 소속 tenant'}</h2>
    <p><a href="#administration/scopeSettings">소속 운영 설정·회원 관리</a></p>
    <p>승인된 소속과 권한에 따라 자원을 조회하고 관리합니다.</p>
    {(serverError || tenantError) && <div role="alert" className="message">{serverError || tenantError}</div>}
    <table className="data-table"><thead><tr><th>이름</th><th>{kind === 'server' ? '호스트' : 'namespace'}</th><th>내 권한</th><th>관리</th></tr></thead>
      <tbody>{rows.map(r => <tr key={r.id}>
        <td>{r.name}</td><td>{r.detail}</td><td>{role(r.id) ? membershipRole[role(r.id)!] : '조회자'}</td>
        <td><div className="btn-group">
          {kind === 'tenant' && role(r.id) === 'admin' && <button className="btn btn-sm btn-primary" onClick={() => setGrantTenantId(r.id)}>{t('mec.delegation.open')}</button>}
          {kind === 'server' && role(r.id) === 'admin' && <button className="btn btn-sm btn-secondary" onClick={() => setTrustId(r.id)}>{t('remote.trust.title')}</button>}
          <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => check(r.id)}>상태 확인</button>
          {kind === 'server' && role(r.id) && role(r.id) !== 'viewer' && me?.memberships.find(m => m.kind === kind && m.scope_id === r.id)?.allow_exec !== false && <button className="btn btn-sm btn-primary" onClick={() => setSelected(r.id)}>명령 실행</button>}
        </div></td>
      </tr>)}</tbody>
    </table>
    {rows.length === 0 && <p>배정된 소속이 없습니다. 최고관리자에게 소속 지정을 요청하세요.</p>}
    {kind === 'tenant' && grantTenantId && role(grantTenantId) === 'admin' && <div><button className="btn btn-secondary" onClick={() => setGrantTenantId('')}>{t('mec.tenant.cancel')}</button><DelegatedManagedGrant key={grantTenantId} tenantId={grantTenantId} /></div>}
    {selected && <form className="inline-form" onSubmit={exec}>
      <label>{rows.find(r => r.id === selected)?.name} · 실행할 명령
        <input aria-label="원격 명령" required value={command} onChange={e => setCommand(e.target.value)} />
      </label>
      <button className="btn btn-primary" disabled={busy}>실행</button>
      <button type="button" className="btn btn-secondary" onClick={() => setSelected('')}>닫기</button>
    </form>}
    {kind === 'server' && servers?.filter(s => s.id === trustId && role(s.id) === 'admin').map(s => <HostTrustForm key={`${s.id}:${s.host_trust?.fingerprint || ''}`} server={s} onSaved={() => { reloadServers(); setTrustId(''); }} />)}
    {output && <pre className="config-preview" role="status">{output}</pre>}
  </div>;
}
