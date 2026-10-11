import React, { useEffect, useState } from 'react';
import { fetchWithRefresh } from '../../hooks/useApi';
import { mecPost } from '../../hooks/mec/useMecApi';
import { useT } from '../../i18n';
import { ErrorBanner, Section } from './common';
interface Candidate { id: string; username: string; memberships: { kind: string; scope_id: string; role: string }[] }
export default function NewManagedGrant({ tenantId, candidates, onCreated }: { tenantId: string; candidates: Candidate[]; onCreated: () => Promise<void> }) {
  const { t } = useT();
  const [member, setMember] = useState('');
  const [preview, setPreview] = useState<{ member: string; realm: string; remote_id: string; remote_username: string } | null>(null);
  const [role, setRole] = useState('read-only');
  const [reason, setReason] = useState('');
  const [autoRevoke, setAutoRevoke] = useState(false);
  const [scopeReviewed, setScopeReviewed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    setPreview(null);
    if (!member) return () => controller.abort();
    (async () => {
      try {
        const response = await fetchWithRefresh(`/api/users/${encodeURIComponent(member)}/external-identities/rancher`, { signal: controller.signal });
        const body = await response.json();
        if (!response.ok || body.error || !body.data?.realm || !body.data.link?.remote_user_id) throw new Error('Explicit identity is unavailable');
        if (!controller.signal.aborted) setPreview({ member, realm: body.data.realm, remote_id: body.data.link.remote_user_id, remote_username: body.data.link.remote_username });
      } catch { if (!controller.signal.aborted) setError(t('mec.managed.identityRequired')); }
    })();
    return () => controller.abort();
  }, [member, t]);
  const sourceRole = candidates.find(candidate => candidate.id === member)?.memberships.find(m => m.kind === 'tenant' && m.scope_id === tenantId)?.role;
  const roles = sourceRole === 'admin' ? ['read-only', 'project-member', 'project-owner'] : sourceRole === 'operator' ? ['read-only', 'project-member'] : ['read-only'];
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (busy || !member || !preview || preview.member !== member || !autoRevoke || !scopeReviewed || reason.trim().length < 3 || !roles.includes(role)) return;
    setBusy(true); setError(null); setResult(null);
    try {
      const response = await mecPost<{ operation_id: string; binding_id: string; local_user_id: string; automatic_revocation: boolean; state: string; source_access_currently_allowed: boolean }>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/managed-memberships`, { expected_realm: preview.realm, expected_remote_user_id: preview.remote_id, local_user_id: member, role_template: role, reason: reason.trim(), automatic_revocation: true, acknowledge_project_role_scope: true });
      if (response.error || !response.data?.operation_id || !response.data.binding_id || response.data.local_user_id !== member || !response.data.automatic_revocation || response.data.state !== 'completed' || typeof response.data.source_access_currently_allowed !== 'boolean') throw new Error('Managed grant receipt was not confirmed');
      setResult(t(response.data.source_access_currently_allowed ? 'mec.managed.created' : 'mec.managed.createdPendingRevocation'));
      setAutoRevoke(false); setScopeReviewed(false);
      await onCreated();
    } catch { setError(t('mec.managed.createFailed')); }
    finally { setBusy(false); }
  };
  return <Section title={t('mec.managed.newGrant')} marginTop="20px">
    <p>{t('mec.managed.newGrantExplanation')}</p>
    <p>{t('mec.managed.roleWarning')}</p>
    <form onSubmit={submit} style={{ display: 'grid', gap: 12, maxWidth: 700, textAlign: 'left' }}>
      <label>{t('mec.managed.grantMember')}
        <select aria-label={t('mec.managed.grantMember')} value={member} disabled={busy} onChange={event => { setMember(event.target.value); setRole('read-only'); setScopeReviewed(false); setAutoRevoke(false); setError(null); setResult(null); }} style={{ display: 'block', width: '100%' }}>
          <option value="">{t('mec.managed.select')}</option>
          {candidates.map(candidate => <option key={candidate.id} value={candidate.id}>{candidate.username} ({candidate.id})</option>)}
        </select>
      </label>
      {preview?.member === member && <p>{t('mec.managed.reviewedIdentity')}: <code>{preview.realm}</code> · {preview.remote_username} · <code>{preview.remote_id}</code></p>}
      <label>{t('mec.managed.grantRole')}
        <select aria-label={t('mec.managed.grantRole')} value={role} disabled={busy || !member} onChange={event => { setRole(event.target.value); setScopeReviewed(false); }} style={{ display: 'block', width: '100%' }}>
          {roles.map(value => <option key={value} value={value}>{value}</option>)}
        </select>
      </label>
      <label>{t('mec.managed.grantReason')}<input value={reason} maxLength={1024} disabled={busy} onChange={event => setReason(event.target.value)} style={{ display: 'block', width: '100%' }} /></label>
      <label><input type="checkbox" checked={autoRevoke} onChange={event => setAutoRevoke(event.target.checked)} disabled={busy} /> {t('mec.managed.accept')}</label>
      <label><input type="checkbox" checked={scopeReviewed} onChange={event => setScopeReviewed(event.target.checked)} disabled={busy} /> {t('mec.managed.scopeReviewed')}</label>
      <button type="submit" className="btn btn-primary" disabled={busy || !member || preview?.member !== member || !autoRevoke || !scopeReviewed || reason.trim().length < 3}>{t('mec.managed.create')}</button>
    </form>
    <ErrorBanner error={error || undefined} />
    {result && <p role="status">{result}</p>}
  </Section>;
}
