import React, { useState } from 'react';
import { mecPost, useMecList } from '../../hooks/mec/useMecApi';
import { useT } from '../../i18n';
import { ErrorBanner, Section } from './common';
interface Operation { id: string; operation: 'create' | 'delete'; state: 'applying' | 'completed' | 'unknown_result'; realm: string; remote_user_id: string; role_template: string; binding_id: string | null; actor: string; updated_at: string; error: string | null }
export default function MembershipOperations({ tenantId, revision }: { tenantId: string; revision: number }) {
  const { t, lang } = useT();
  // Remount on an attempted mutation: an error can still have an intent record.
  return <History key={revision} tenantId={tenantId} t={t} lang={lang} />;
}
function History({ tenantId, t, lang }: { tenantId: string; t: ReturnType<typeof useT>['t']; lang: ReturnType<typeof useT>['lang'] }) {
  const { data, error, loading, refetch } = useMecList<Operation>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/membership-operations`, 30_000);
  return <Section title={t('mec.membershipOperations.title')} marginTop="20px">
    <p>{t('mec.membershipOperations.explanation')}</p>
    <button className="btn btn-secondary btn-small" onClick={refetch}>{t('mec.action.refresh')}</button>
    <ErrorBanner error={error || undefined} />
    {loading && <p>{t('mec.state.loading')}</p>}
    {!error && data?.map(op => <details key={op.id} style={{ marginTop: 12 }}>
      <summary><code>{op.remote_user_id}</code> · {op.role_template} · {t(`mec.membershipOperations.${op.operation}`)} · {t(`mec.membershipOperations.${op.state}`)}</summary>
      <dl className="external-identity-details">
        <dt>{t('mec.membershipOperations.operationId')}</dt><dd><code>{op.id}</code></dd>
        <dt>Rancher</dt><dd>{op.realm}</dd>
        <dt>{t('mec.tenant.members.colBindingId')}</dt><dd>{op.binding_id || t('mec.membershipOperations.unverified')}</dd>
        <dt>{t('mec.membershipOperations.actor')}</dt><dd>{op.actor}</dd>
        <dt>{t('mec.membershipOperations.updated')}</dt><dd>{new Date(op.updated_at).toLocaleString(lang)}</dd>
      </dl>
      {op.state !== 'completed' && <p role="status">{t('mec.membershipOperations.doNotRetry')}</p>}
      {op.error && <p>{op.error}</p>}
      {op.state === 'unknown_result' && (op.binding_id
        ? <ReconciliationForm tenantId={tenantId} operation={op} onConfirmed={refetch} />
        : <p>{t('mec.membershipOperations.noCandidate')}</p>)}
    </details>)}
    {data?.length === 0 && !loading && !error && <p>{t('mec.membershipOperations.empty')}</p>}
  </Section>;
}

function ReconciliationForm({ tenantId, operation, onConfirmed }: { tenantId: string; operation: Operation; onConfirmed: () => Promise<void> }) {
  const { t } = useT();
  const [reason, setReason] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (busy || confirmed || confirmation !== operation.id || reason.trim().length < 3) return;
    setBusy(true); setError(null);
    try {
      const response = await mecPost<{ operation_id: string; state: string; provider_write_replayed: boolean }>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/membership-operations/${encodeURIComponent(operation.id)}/reconcile`, {
        confirm_operation_id: confirmation, expected_binding_id: operation.binding_id,
        expected_updated_at: operation.updated_at, reason: reason.trim(),
      });
      if (response.error) throw new Error(response.error.message);
      if (response.data?.operation_id !== operation.id || response.data.state !== 'completed' || response.data.provider_write_replayed !== false) throw new Error('Invalid confirmation receipt');
      setConfirmed(true);
      await onConfirmed();
    } catch { setError(t('mec.membershipOperations.reconcileError')); }
    finally { setBusy(false); }
  };
  return <form onSubmit={submit} style={{ display: 'grid', gap: 10, marginTop: 12, maxWidth: 700, textAlign: 'left' }}>
    <p>{t('mec.membershipOperations.reconcileExplanation')}</p>
    <label>{t('mec.membershipOperations.reviewReason')}
      <input value={reason} onChange={event => setReason(event.target.value)} maxLength={1024} disabled={busy || confirmed} style={{ display: 'block', width: '100%' }} />
    </label>
    <label>{t('mec.membershipOperations.confirmId')}
      <input value={confirmation} onChange={event => setConfirmation(event.target.value)} autoComplete="off" disabled={busy || confirmed} style={{ display: 'block', width: '100%' }} />
    </label>
    <button className="btn btn-secondary" type="submit" disabled={busy || confirmed || confirmation !== operation.id || reason.trim().length < 3}>{busy ? t('mec.state.loading') : t('mec.membershipOperations.reconcile')}</button>
    {confirmed && <p role="status">{t('mec.membershipOperations.reconciled')}</p>}
    <ErrorBanner error={error || undefined} />
  </form>;
}
