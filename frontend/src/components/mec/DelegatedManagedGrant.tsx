import ScopedManagedMemberships from './ScopedManagedMemberships';
import React, { useState } from 'react';
import { useMecApi, mecPost } from '../../hooks/mec/useMecApi';
import { useT } from '../../i18n';
import { Section, ErrorBanner } from './common';
interface Context { version: string; realm: string; project_id: string; allowed_roles: string[]; expires_at: string; candidates: { id: string; username: string; source_role: string; link: { remote_user_id: string; remote_username: string } | null }[] }
export default function DelegatedManagedGrant({ tenantId }: { tenantId: string }) {
 const { t } = useT();
 const [ledgerRevision, setLedgerRevision] = useState(0);
 const { data, error, refetch } = useMecApi<Context>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/delegated-access-context`, 30_000);
 const [member, setMember] = useState(''); const [role, setRole] = useState('read-only'); const [reason, setReason] = useState('');
 const [reviewedVersion, setReviewedVersion] = useState('');
 const [auto, setAuto] = useState(false); const [reviewed, setReviewed] = useState(false); const [busy, setBusy] = useState(false); const [failure, setFailure] = useState<string | null>(null); const [saved, setSaved] = useState(false);
 const candidate = !error ? data?.candidates.find(c => c.id === member) : undefined;
 const roles = data?.allowed_roles.filter(r => r === 'read-only' || r === 'project-member' && candidate?.source_role === 'operator') || [];
 const reviewKey = JSON.stringify([data?.version, data?.realm, data?.project_id, candidate?.id, candidate?.source_role, candidate?.link?.remote_user_id, roles]);
 const submit = async (event: React.FormEvent) => {
  event.preventDefault(); if (busy || error || !data || !candidate?.link || !roles.includes(role) || !auto || !reviewed || reviewedVersion !== reviewKey || reason.trim().length < 3) return;
  setBusy(true); setFailure(null); setSaved(false);
  try {
   const response = await mecPost<{ operation_id: string; local_user_id: string; automatic_revocation: boolean; state: string; binding_id: string; source_access_currently_allowed: boolean }>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/managed-memberships`, { expected_delegation_version: data.version, expected_realm: data.realm, expected_remote_user_id: candidate.link.remote_user_id, local_user_id: member, role_template: role, reason: reason.trim(), automatic_revocation: true, acknowledge_project_role_scope: true });
   if (response.error || !response.data?.operation_id || response.data.local_user_id !== member || response.data.automatic_revocation !== true || response.data.state !== 'completed' || !response.data.binding_id || typeof response.data.source_access_currently_allowed !== 'boolean') throw new Error('Grant outcome not confirmed');
   setSaved(true); setAuto(false); setReviewed(false); await refetch();
  } catch { setFailure(t('mec.managed.createFailed')); await refetch(); }
  finally { setLedgerRevision(v => v + 1); setBusy(false); }
 };
 return <><Section title={t('mec.delegation.delegatedTitle')} marginTop="20px">
  <p>{t('mec.delegation.delegatedExplanation')}</p><button className="btn btn-secondary btn-small" onClick={refetch}>{t('mec.action.refresh')}</button>
  <ErrorBanner error={(error ? t('mec.scopedManaged.grantUnavailable') : failure) || undefined} />
  {!error && data && <><p>{data.realm} · {data.project_id} · {data.expires_at}</p><p>{t('mec.managed.roleWarning')}</p>
   <form onSubmit={submit} style={{ display: 'grid', gap: 12, maxWidth: 700, textAlign: 'left' }}>
    <label>{t('mec.managed.grantMember')}<select aria-label={t('mec.managed.grantMember')} value={member} disabled={busy} onChange={e => { setMember(e.target.value); setRole('read-only'); setReviewed(false); setAuto(false); setSaved(false); }} style={{ display: 'block', width: '100%' }}><option value="">{t('mec.managed.select')}</option>{data.candidates.map(c => <option key={c.id} value={c.id}>{c.username} ({c.id})</option>)}</select></label>
    {candidate && <p>{candidate.link ? <>{t('mec.managed.reviewedIdentity')}: {candidate.link.remote_username} · <code>{candidate.link.remote_user_id}</code></> : t('mec.managed.identityRequired')}</p>}
    <label>{t('mec.managed.grantRole')}<select aria-label={t('mec.managed.grantRole')} value={role} disabled={busy || !candidate} onChange={e => { setRole(e.target.value); setReviewed(false); }} style={{ display: 'block', width: '100%' }}>{roles.map(r => <option key={r} value={r}>{r}</option>)}</select></label>
    <label>{t('mec.managed.grantReason')}<input value={reason} maxLength={1024} disabled={busy} onChange={e => setReason(e.target.value)} style={{ display: 'block', width: '100%' }} /></label>
    <label><input type="checkbox" checked={auto} onChange={e => setAuto(e.target.checked)} disabled={busy} /> {t('mec.managed.accept')}</label>
    <label><input type="checkbox" checked={reviewed && reviewedVersion === reviewKey} onChange={e => { setReviewed(e.target.checked); setReviewedVersion(reviewKey); }} disabled={busy} /> {t('mec.managed.scopeReviewed')}</label>
    <button type="submit" className="btn btn-primary" disabled={busy || !candidate?.link || !roles.includes(role) || !auto || !reviewed || reviewedVersion !== reviewKey || reason.trim().length < 3}>{t('mec.managed.create')}</button>
   </form></>}
  {saved && <p role="status">{t('mec.managed.created')}</p>}
 </Section><ScopedManagedMemberships key={ledgerRevision} tenantId={tenantId} /></>;
}
