import React, { useState } from 'react';
import { useMecList, mecPost } from '../../hooks/mec/useMecApi';
import { useT } from '../../i18n';
import { Section, ErrorBanner } from './common';
interface Record {
 operation_id: string; local_user_id: string; username: string | null; realm: string; project_id: string; remote_user_id: string; role_template: string; binding_id: string;
 ownership_state: string; ownership_updated_at: string; source_access_currently_allowed: boolean; context_matches: boolean; unresolved_change: boolean; can_revoke: boolean; delegation_version: string | null;
 latest_change: { id: string; operation: string; state: string; updated_at: string } | null;
}
export default function ScopedManagedMemberships({ tenantId }: { tenantId: string }) {
 const { t } = useT();
 const { data, error, loading, refetch } = useMecList<Record>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/scoped-managed-memberships`, 30_000);
 return <Section title={t('mec.scopedManaged.title')} marginTop="20px">
  <p>{t('mec.scopedManaged.explanation')}</p>
  <button className="btn btn-secondary btn-small" onClick={refetch}>{t('mec.action.refresh')}</button>
  <ErrorBanner error={error || undefined} />
  {loading && <p>{t('mec.state.loading')}</p>}
  {!error && data?.map(row => <details key={row.operation_id} style={{ marginTop: 12 }}>
   <summary>{row.username || row.local_user_id} · {row.role_template} · {t(row.ownership_state === 'revoked' ? 'mec.managed.revoked' : 'mec.scopedManaged.recordedActive')}</summary>
   <dl className="external-identity-details">
    <dt>{t('mec.managed.member')}</dt><dd><code>{row.local_user_id}</code></dd>
    <dt>{t('mec.membershipOperations.operationId')}</dt><dd><code>{row.operation_id}</code></dd>
    <dt>Rancher</dt><dd>{row.realm} · {row.project_id}</dd>
    <dt>{t('mec.pendingManaged.account')}</dt><dd><code>{row.remote_user_id}</code></dd>
    <dt>{t('mec.tenant.members.colBindingId')}</dt><dd><code>{row.binding_id}</code></dd>
    <dt>{t('mec.scopedManaged.sourcePolicy')}</dt><dd>{t(row.source_access_currently_allowed ? 'mec.scopedManaged.allowed' : 'mec.scopedManaged.denied')}</dd>
   </dl>
   {!row.context_matches && <p>{t('mec.scopedManaged.contextChanged')}</p>}
   {row.latest_change && <p>{t('mec.scopedManaged.latest')}: <code>{row.latest_change.id}</code> · {t(`mec.membershipOperations.${row.latest_change.operation}`)} · {t(`mec.membershipOperations.${row.latest_change.state}`)}</p>}
   {row.unresolved_change && <p role="status">{t('mec.scopedManaged.pending')}</p>}
   {row.can_revoke && !row.unresolved_change && row.ownership_state === 'active' && <RevokeForm key={row.operation_id + row.ownership_updated_at + row.delegation_version + row.latest_change?.updated_at} tenantId={tenantId} record={row} onChanged={refetch} />}
  </details>)}
  {!error && !loading && data?.length === 0 && <p>{t('mec.scopedManaged.empty')}</p>}
 </Section>;
}
function RevokeForm({ tenantId, record, onChanged }: { tenantId: string; record: Record; onChanged: () => Promise<void> }) {
 const { t } = useT();const [reason,setReason] = useState('');const [confirmation,setConfirmation] = useState('');const [accepted,setAccepted] = useState(false);const [busy,setBusy] = useState(false);const [unconfirmed,setUnconfirmed] = useState(false);const [confirmed,setConfirmed] = useState(false);
 const submit = async (event: React.FormEvent) => {
  event.preventDefault();if (busy || unconfirmed || confirmed || confirmation !== record.operation_id || reason.trim().length < 3 || !accepted) return;
  setBusy(true);
  try {
   const response = await mecPost<{ operation_id: string; grant_operation_id: string; binding_id: string; state: string; ownership_state: string; direct_binding_absence_confirmed: boolean; all_external_access_revoked: boolean }>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/managed-memberships/${encodeURIComponent(record.operation_id)}/revoke`, { confirm_operation_id: confirmation, expected_binding_id: record.binding_id, expected_updated_at: record.ownership_updated_at, expected_delegation_version: record.delegation_version, reason: reason.trim(), acknowledge_direct_binding_only: accepted });
   if (response.error || !response.data?.operation_id || response.data.grant_operation_id !== record.operation_id || response.data.binding_id !== record.binding_id || response.data.state !== 'completed' || response.data.ownership_state !== 'revoked' || response.data.direct_binding_absence_confirmed !== true || response.data.all_external_access_revoked !== false) throw new Error('Revocation not confirmed');
   setConfirmed(true);await onChanged();
  } catch { setUnconfirmed(true);await onChanged(); }
  finally { setBusy(false); }
 };
 return <form onSubmit={submit} style={{ display:'grid',gap:12,maxWidth:700,textAlign:'left' }}>
  <p>{t('mec.scopedManaged.revokeExplanation')}</p>
  <label>{t('mec.scopedManaged.reason')}<input value={reason} maxLength={1024} disabled={busy || confirmed || unconfirmed} onChange={e=>setReason(e.target.value)} style={{ display:'block',width:'100%' }} /></label>
  <label>{t('mec.scopedManaged.confirm')}<input value={confirmation} disabled={busy || confirmed || unconfirmed} onChange={e=>setConfirmation(e.target.value)} style={{ display:'block',width:'100%' }} /></label>
  <label><input type="checkbox" checked={accepted} disabled={busy || confirmed || unconfirmed} onChange={e=>setAccepted(e.target.checked)} /> {t('mec.scopedManaged.accept')}</label>
  <button type="submit" className="btn btn-danger" disabled={busy || confirmed || unconfirmed || confirmation !== record.operation_id || reason.trim().length < 3 || !accepted}>{t('mec.scopedManaged.revoke')}</button>
  {unconfirmed && <p role="alert">{t('mec.scopedManaged.unconfirmed')}</p>}
  {confirmed && <p role="status">{t('mec.managed.revoked')}</p>}
 </form>;
}
