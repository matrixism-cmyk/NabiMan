import React, { useState } from 'react';
import { useApi, fetchWithRefresh } from '../../shared/api/useApi';
import { useMecApi } from '../../hooks/mec/useMecApi';
import { useT } from '../../i18n';
import { Section, ErrorBanner } from './common';
interface Manager { id: string; username: string; role: string; status: string; memberships: { kind: string; scope_id: string; role: string; enabled: boolean; expires_at?: string | null }[] }
interface Policy { version: string; enabled: boolean; manager_ids: string[]; allowed_roles: string[]; allowed_actions?: string[]; expires_at: string | null }
interface Snapshot { policy: Policy | null; realm: string; project_id: string | null }
export default function TenantAccessDelegation({ tenantId }: { tenantId: string }) {
 const { t } = useT();
 const { data, error, refetch } = useMecApi<Snapshot>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/access-delegation`);
 const { data: users, error: userError } = useApi<Manager[]>('/api/users');
 const [saved, setSaved] = useState(false);
 return <Section title={t('mec.delegation.title')} marginTop="20px">
  <p>{t('mec.delegation.explanation')}</p><p>{t('mec.managed.roleWarning')}</p>
  <ErrorBanner error={error || userError || undefined} />
  {!error && data && <><p>{t(data.policy?.enabled ? 'mec.delegation.enabled' : 'mec.delegation.disabled')} · <code>{data.realm}</code> · <code>{data.project_id}</code></p>
   {data.policy && <p>{t('mec.delegation.current')}: {data.policy.manager_ids.join(', ')} · {data.policy.allowed_roles.join(', ')} · {(data.policy.allowed_actions || ['grant']).join(', ')} · {data.policy.expires_at || '—'}</p>}
   <DelegationForm key={(data.policy?.version || 'initial') + data.realm + data.project_id} tenantId={tenantId} data={data} users={users || []} userError={userError} onSaved={async () => { setSaved(true); await refetch(); }} />
  </>}
  {saved && <p role="status">{t('mec.delegation.saved')}</p>}
 </Section>;
}
function DelegationForm({ tenantId, data, users, userError, onSaved }: { tenantId: string; data: Snapshot; users: Manager[]; userError: string | null; onSaved: () => Promise<void> }) {
 const { t } = useT();
 const [managers, setManagers] = useState<string[]>(data.policy?.manager_ids || []);
 const [enabled, setEnabled] = useState(data.policy?.enabled || false);
 const [grantAction, setGrantAction] = useState(data.policy?.allowed_actions?.includes('grant') ?? true);
 const [revokeAction, setRevokeAction] = useState(data.policy?.allowed_actions?.includes('revoke') || false);
 const [memberRole, setMemberRole] = useState(data.policy?.allowed_roles.includes('project-member') || false);
 const [expiry, setExpiry] = useState(() => { if (!data.policy?.expires_at) return ''; const date = new Date(data.policy.expires_at); return Number.isNaN(date.getTime()) ? '' : new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0,16); });
 const [reason, setReason] = useState('');
 const [confirmation, setConfirmation] = useState('');
 const [reviewed, setReviewed] = useState(false);
 const [busy, setBusy] = useState(false);
 const [failure, setFailure] = useState<string | null>(null);
 const candidates = users?.filter(u => u.status === 'active' && u.role !== 'admin' && u.memberships.some(m => m.kind === 'tenant' && m.scope_id === tenantId && m.role === 'admin' && m.enabled && (!m.expires_at || Date.parse(m.expires_at) > Date.now()))) || [];
 const submit = async (event: React.FormEvent) => {
  event.preventDefault();
  if (busy || userError || !data?.project_id || confirmation !== tenantId || !reviewed || reason.trim().length < 3) return;
  setBusy(true); setFailure(null);
  try {
   const response = await fetchWithRefresh(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/access-delegation`, { method: 'PUT', body: JSON.stringify({ expected_version: data.policy?.version || null, expected_realm: data.realm, expected_project_id: data.project_id, confirm_tenant_id: confirmation, enabled, manager_ids: managers, allowed_roles: memberRole ? ['read-only', 'project-member'] : ['read-only'], allowed_actions: [...(grantAction ? ['grant'] : []), ...(revokeAction ? ['revoke'] : [])], expires_at: expiry ? new Date(expiry).toISOString() : null, reason: reason.trim(), acknowledge_project_role_scope: reviewed }) });
   const body = await response.json();
   if (!response.ok || !body.data?.version || body.data.tenant_id !== tenantId || body.data.enabled !== enabled) throw new Error('Delegation not confirmed');
   setReviewed(false); await onSaved();
  } catch { setFailure(t('mec.delegation.failed')); }
  finally { setBusy(false); }
 };
 return <><ErrorBanner error={failure || undefined} />
   <form onSubmit={submit} style={{ display: 'grid', gap: 12, maxWidth: 700, textAlign: 'left' }}>
    <label><input type="checkbox" checked={enabled} onChange={e => { setEnabled(e.target.checked); setReviewed(false); }} disabled={busy} /> {t('mec.delegation.enable')}</label>
    <fieldset disabled={busy || !!userError}><legend>{t('mec.delegation.managers')}</legend>{candidates.map(u => <label key={u.id} style={{ display: 'block' }}><input type="checkbox" checked={managers.includes(u.id)} onChange={e => { setManagers(previous => e.target.checked ? [...previous, u.id] : previous.filter(id => id !== u.id)); setReviewed(false); }} /> {u.username} ({u.id})</label>)}</fieldset>
    <label><input type="checkbox" checked={grantAction} onChange={e => { setGrantAction(e.target.checked); setReviewed(false); }} disabled={busy} /> {t('mec.scopedManaged.allowGrant')}</label>
    <label><input type="checkbox" checked={revokeAction} onChange={e => { setRevokeAction(e.target.checked); setReviewed(false); }} disabled={busy} /> {t('mec.scopedManaged.allowRevoke')}</label>
    <p>read-only</p>
    <label><input type="checkbox" checked={memberRole} onChange={e => { setMemberRole(e.target.checked); setReviewed(false); }} disabled={busy} /> {t('mec.delegation.memberRole')}</label>
    <label>{t('mec.delegation.expiry')}<input type="datetime-local" value={expiry} onChange={e => { setExpiry(e.target.value); setReviewed(false); }} disabled={busy} style={{ display: 'block', width: '100%' }} /></label>
    <label>{t('mec.delegation.reason')}<input value={reason} maxLength={1024} onChange={e => setReason(e.target.value)} disabled={busy} style={{ display: 'block', width: '100%' }} /></label>
    <label>{t('mec.delegation.confirm')}<input value={confirmation} onChange={e => setConfirmation(e.target.value)} disabled={busy} style={{ display: 'block', width: '100%' }} /></label>
    <label><input type="checkbox" checked={reviewed} onChange={e => setReviewed(e.target.checked)} disabled={busy} /> {t('mec.delegation.review')}</label>
    <button type="submit" className="btn btn-secondary" disabled={busy || !!userError || !data.project_id || confirmation !== tenantId || !reviewed || reason.trim().length < 3 || (enabled && (!managers.length || !expiry || (!grantAction && !revokeAction)))}>{t('mec.delegation.save')}</button>
   </form>
 </>;
}
