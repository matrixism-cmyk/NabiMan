import React, { useState } from 'react';
import { apiRequest, useApi } from '../shared/api/useApi';
import { useMecList } from '../hooks/mec/useMecApi';
import { Tenant } from '../types/mec';
import { MemberUser, ScopeOption } from './MembershipEditor';
import { accessActive } from './MemberAccessFields';
import ScopePolicyHistory from './ScopePolicyHistory';
import UserManagementPanel from './UserManagementPanel';
import { confirmDialog } from '../shared/ui/ConfirmDialog';
import { useT } from '../i18n';

interface Settings { revision: string; contact_email: string; member_notice: string; member_limit: number | null; allow_commands: boolean; require_mfa: boolean }
function ScopePolicy({ scope, canManage, tenant }: { scope: ScopeOption; canManage: boolean; tenant?: Tenant }) {
  const endpoint = `/api/scope-settings/${scope.kind}/${scope.id}`;
  const { data, loading, error, refetch } = useApi<Settings>(endpoint);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const [reason, setReason] = useState('');
  const [conflict, setConflict] = useState(false);
  const [historyVersion, setHistoryVersion] = useState(0);
  const { t } = useT();
  const value = draft || data;
  if (loading) return <p>{t('scope.policy.loading')}</p>;
  if (error || !value) return <p role="alert">{error || t('scope.policy.unreadable')}</p>;
  const update = (change: Partial<Settings>) => setDraft({ ...value, ...change });
  return <div className="inline-form">
    <h3>{t('scope.policy.title', { scope: scope.label })}</h3>
    <form className="scope-policy-form" onSubmit={async e => {
      e.preventDefault(); setBusy(true); setMessage('');
      try {
        const { revision, ...settings } = value;
        const res = await apiRequest<string>(endpoint, { method: 'PUT', body: JSON.stringify({ ...settings, expected_revision: revision, change_reason: reason }) });
        setMessage(res.data || res.message);
        if (res.success) { await refetch(); setDraft(null); setReason(''); setConflict(false); setHistoryVersion(v => v + 1); }
        else if (res.code === 'scope_policy_conflict') setConflict(true);
      } catch { setMessage(t('scope.policy.saveFailed')); }
      finally { setBusy(false); }
    }}>
      <fieldset disabled={!canManage || busy} style={{ border: 0, padding: 0 }}>
        <div className="form-row"><label>{t('scope.policy.contact')}<input type="email" maxLength={254} value={value.contact_email} onChange={e => update({ contact_email: e.target.value })} /></label>
          <label>{t('scope.policy.memberLimit')}<input type="number" min={1} max={10000} placeholder={t('scope.policy.noLimit')} value={value.member_limit ?? ''} onChange={e => update({ member_limit: e.target.value ? Number(e.target.value) : null })} /></label></div>
        <p>{t('scope.policy.limitHint')}</p>
        <label>{t('scope.policy.notice')}<textarea rows={3} style={{ width: '100%', boxSizing: 'border-box' }} value={value.member_notice} onChange={e => update({ member_notice: e.target.value })} /></label>
        {scope.kind === 'server' && <p><label className="membership-check"><input type="checkbox" checked={value.allow_commands} onChange={e => update({ allow_commands: e.target.checked })} /> {t('scope.policy.allowCommands')}</label><br />{t('scope.policy.allowCommandsHint')}</p>}
        <p><label className="membership-check"><input type="checkbox" checked={value.require_mfa || false} onChange={e => update({ require_mfa: e.target.checked })} /> {t('scope.policy.requireMfa')}</label><br />
          {t('scope.policy.requireMfaHint')}</p>
        <p>{t('scope.policy.mfaNote')}</p>
        {canManage && <><label>{t('scope.policy.reason')}<input type="text" minLength={3} maxLength={500} required value={reason} onChange={e => setReason(e.target.value)} style={{ display: 'block', width: '100%', textAlign: 'left' }} /></label>
          <button className="btn btn-primary" disabled={!draft || busy || conflict}>{t('scope.policy.save')}</button></>}
      </fieldset>
    </form>
    {message && <p role="status">{message}</p>}
    {conflict && <div role="alert"><p>{t('scope.policy.conflict')}</p>
      <button className="btn btn-secondary" disabled={busy} onClick={async () => {
        if (!(await confirmDialog({ message: t('scope.policy.discardConfirm'), danger: true }))) return;
        setBusy(true); await refetch(); setDraft(null); setConflict(false); setMessage(''); setBusy(false);
      }}>{t('scope.policy.discard')}</button></div>}
    {canManage && <ScopePolicyHistory key={`${endpoint}:${historyVersion}`} endpoint={endpoint} />}
    {tenant && <div>
      <h3>{t('scope.quota.title')}</h3>
      <table className="data-table"><thead><tr><th>{t('scope.quota.cpu')}</th><th>{t('scope.quota.memory')}</th><th>GPU</th><th>namespace</th></tr></thead>
        <tbody><tr><td>{tenant.quota.cpu_requests} / {tenant.quota.cpu_limits}</td><td>{tenant.quota.memory_requests} / {tenant.quota.memory_limits}</td><td>{tenant.quota.gpu}</td><td>{tenant.namespace}</td></tr></tbody></table>
      <p>{t('scope.quota.summary', { storage: tenant.quota.storage, pods: tenant.quota.pods, pvcs: tenant.quota.pvcs, lbs: tenant.quota.lb_services })}</p>
      <p>{t('scope.quota.readOnly')}</p>
      <p>{t('scope.quota.whereBefore')}<a href="#mec/mecTenants">{t('scope.quota.whereLink')}</a>{t('scope.quota.whereAfter')}</p>
    </div>}
  </div>;
}
export default function ScopeSettingsPanel() {
  const { data: me } = useApi<MemberUser>('/api/users/me');
  const { data: servers, error: serverError } = useApi<{ id: string; name: string }[]>('/api/remote-servers');
  const { data: tenants, error: tenantError } = useMecList<Tenant>('/api/mec/v1/tenants');
  const [selected, setSelected] = useState('');
  const [members, setMembers] = useState(false);
  const { t } = useT();
  const options: ScopeOption[] = [
    ...(tenants || []).map(x => ({ kind: 'tenant' as const, id: x.id, label: t('users.scope.tenant', { name: x.display_name }) })),
    ...(servers || []).map(s => ({ kind: 'server' as const, id: s.id, label: t('users.scope.server', { name: s.name }) })),
  ];
  const scope = options.find(s => `${s.kind}:${s.id}` === selected) || options[0];
  const canManage = me?.role === 'admin' || !!me?.memberships.some(m => scope && m.kind === scope.kind && m.scope_id === scope.id && m.role === 'admin' && accessActive(m));
  return <div className="panel scope-settings-panel">
    <h2>{t('scope.title')}</h2>
    <p>{t('scope.lead')}</p>
    {(serverError || tenantError) && <p role="alert">{serverError || tenantError}</p>}
    <label>{t('scope.pick')} <select className="select-input" value={scope ? `${scope.kind}:${scope.id}` : ''} onChange={e => { setSelected(e.target.value); setMembers(false); }}>
      {options.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
    </select></label>
    {scope ? <>
      <div className="btn-group"><button className="btn btn-secondary" onClick={() => setMembers(false)}>{t('scope.policyTab')}</button>
        {canManage && <button className="btn btn-primary" onClick={() => setMembers(true)}>{t('scope.membersTab')}</button>}</div>
      {members && canManage ? <UserManagementPanel key={selected} initialScope={`${scope.kind}:${scope.id}`} />
        : <ScopePolicy key={`${scope.kind}:${scope.id}`} scope={scope} canManage={canManage} tenant={tenants?.find(x => scope.kind === 'tenant' && x.id === scope.id)} />}
    </> : <p>{t('scope.none')}</p>}
  </div>;
}
