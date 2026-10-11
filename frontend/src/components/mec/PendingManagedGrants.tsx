import React from 'react';
import { useMecList } from '../../hooks/mec/useMecApi';
import { useT } from '../../i18n';
import { ErrorBanner, Section } from './common';
interface PendingGrant {
  local_user_id: string;
  source_role: string;
  operation: { id: string; realm: string; project_id: string; remote_user_id: string; role_template: string; binding_id: string | null; state: string };
}
export default function PendingManagedGrants({ tenantId }: { tenantId: string }) {
  const { t } = useT();
  const { data, error, loading, refetch } = useMecList<PendingGrant>(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/pending-managed-grants`, 30_000);
  return <Section title={t('mec.pendingManaged.title')} marginTop="20px">
    <p>{t('mec.pendingManaged.explanation')}</p>
    <button className="btn btn-secondary btn-small" onClick={refetch}>{t('mec.action.refresh')}</button>
    <ErrorBanner error={error || undefined} />
    {loading && <p>{t('mec.state.loading')}</p>}
    {!error && data?.filter(row => row.operation.state === 'unknown_result').map(row => <details key={row.operation.id} style={{ marginTop: 12 }}>
      <summary><code>{row.local_user_id}</code> · {row.operation.role_template} · {t('mec.membershipOperations.unknown_result')}</summary>
      <dl className="external-identity-details">
        <dt>{t('mec.membershipOperations.operationId')}</dt><dd><code>{row.operation.id}</code></dd>
        <dt>Rancher</dt><dd>{row.operation.realm}</dd>
        <dt>{t('mec.pendingManaged.project')}</dt><dd><code>{row.operation.project_id}</code></dd>
        <dt>{t('mec.pendingManaged.account')}</dt><dd><code>{row.operation.remote_user_id}</code></dd>
        <dt>{t('mec.tenant.members.colBindingId')}</dt><dd>{row.operation.binding_id || t('mec.membershipOperations.unverified')}</dd>
      </dl>
      <p>{t(row.operation.binding_id ? 'mec.pendingManaged.candidate' : 'mec.pendingManaged.noCandidate')}</p>
    </details>)}
    {!error && !loading && data?.length === 0 && <p>{t('mec.pendingManaged.empty')}</p>}
  </Section>;
}
