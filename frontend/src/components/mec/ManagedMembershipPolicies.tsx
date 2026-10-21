import ScopedManagedMemberships from './ScopedManagedMemberships';
import TenantAccessDelegation from './TenantAccessDelegation';
import PendingManagedGrants from './PendingManagedGrants';
import NewManagedGrant from './NewManagedGrant';
import React, { useState } from 'react';
import { useApi } from '../../shared/api/useApi';
import { mecPost, useMecList } from '../../hooks/mec/useMecApi';
import { useT } from '../../i18n';
import { ErrorBanner, Section } from './common';
interface Member { id: string; username: string; status: string; memberships: { kind: string; scope_id: string; role: string }[] }
interface ManagedBinding { operation_id: string; local_user_id: string; role_template: string; binding_id: string; ownership_state: string; source_role: string }
export default function ManagedMembershipPolicies({ tenantId, onChanged }: { tenantId: string; onChanged?: () => Promise<void> }) {
  const { t } = useT();
  const { data: users, error: userError } = useApi<Member[]>('/api/users');
  const { data, error, refetch } = useMecList<ManagedBinding>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/managed-memberships`, 30_000);
  const [operationId, setOperationId] = useState('');
  const [userId, setUserId] = useState('');
  const [reason, setReason] = useState('');
  const [accepted, setAccepted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!accepted || !userId || !operationId || reason.trim().length < 3 || busy) return;
    setBusy(true); setFailure(null); setResult(null);
    try {
      const response = await mecPost<{ operation_id: string; local_user_id: string; automatic_revocation: boolean }>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/membership-operations/${encodeURIComponent(operationId)}/associate`, { local_user_id: userId, confirm_operation_id: operationId, automatic_revocation: true, reason: reason.trim() });
      if (response.error || response.data?.operation_id !== operationId || response.data.local_user_id !== userId || response.data.automatic_revocation !== true) throw new Error('Association not confirmed');
      setResult(t('mec.managed.associated')); setOperationId(''); setAccepted(false);
      await refetch();
    } catch { setFailure(t('mec.managed.failed')); }
    finally { setBusy(false); }
  };
  const candidates = users?.filter(user => user.status === 'active' && user.memberships.some(m => m.kind === 'tenant' && m.scope_id === tenantId)) || [];
  return <><NewManagedGrant tenantId={tenantId} candidates={userError ? [] : candidates} onCreated={async () => { await refetch(); await onChanged?.(); }} /><Section title={t('mec.managed.title')} marginTop="20px">
    <p>{t('mec.managed.explanation')}</p>
    <ErrorBanner error={error || userError || failure || undefined} />
    {!error && data?.map(grant => <p key={grant.operation_id}><code>{grant.local_user_id}</code> · {grant.source_role} · <code>{grant.binding_id}</code> · {t(grant.ownership_state === 'revoked' ? 'mec.managed.revoked' : 'mec.managed.monitored')}</p>)}
    <form onSubmit={submit} style={{ display: 'grid', gap: 12, maxWidth: 700, textAlign: 'left' }}>
      <label>{t('mec.managed.member')}
        <select value={userId} onChange={event => setUserId(event.target.value)} disabled={busy || !!userError} style={{ display: 'block', width: '100%' }}>
          <option value="">{t('mec.managed.select')}</option>
          {candidates.map(user => <option key={user.id} value={user.id}>{user.username} ({user.id})</option>)}
        </select>
      </label>
      <label>{t('mec.managed.operation')}<input value={operationId} onChange={event => setOperationId(event.target.value.trim())} disabled={busy} style={{ display: 'block', width: '100%' }} /></label>
      <label>{t('mec.managed.reason')}<input value={reason} onChange={event => setReason(event.target.value)} maxLength={1024} disabled={busy} style={{ display: 'block', width: '100%' }} /></label>
      <label><input type="checkbox" checked={accepted} onChange={event => setAccepted(event.target.checked)} disabled={busy} /> {t('mec.managed.accept')}</label>
      <button type="submit" className="btn btn-secondary" disabled={busy || !accepted || !userId || !operationId || reason.trim().length < 3}>{t('mec.managed.attach')}</button>
    </form>
    {result && <p role="status">{result}</p>}
    <p>{t('mec.managed.limits')}</p>
  </Section><PendingManagedGrants tenantId={tenantId} /><TenantAccessDelegation tenantId={tenantId} /><ScopedManagedMemberships tenantId={tenantId} /></>;
}
