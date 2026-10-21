import React, { useEffect, useState } from 'react';
import { fetchWithRefresh } from '../shared/api/useApi';
import { useT } from '../i18n';
import './membership-management.css';
interface Binding { id: string; user_id: string; role_template: string }
interface Report {
  local_username: string; inspected_at: string; source_access_allowed: boolean;
  realm: string; project_id: string | null; reference_project_role: string | null;
  identity_link: { remote_user_id: string; remote_username: string } | null;
  remote_account_enabled: boolean | null; direct_project_role_matches: boolean;
  direct_bindings: Binding[]; blockers: string[]; unverified_access_sources: string[];
}
export default function TenantAccessInspection({ tenantId, userId, onClose, onInspected }: { tenantId: string; userId: string; onClose: () => void; onInspected?: () => void }) {
  const { t, lang } = useT();
  const [report, setReport] = useState<Report | null>(null);
  const [error, setError] = useState(false);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    setReport(null); setError(false);
    (async () => {
      try {
        const response = await fetchWithRefresh(`/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/members/${encodeURIComponent(userId)}/access-plan`, { signal: controller.signal });
        const body = await response.json();
        if (!response.ok || body.error || !body.data || !Array.isArray(body.data.direct_bindings)) throw new Error();
        if (!controller.signal.aborted) setReport(body.data);
      } catch { if (!controller.signal.aborted) setError(true); }
      finally { if (!controller.signal.aborted) onInspected?.(); }
    })();
    return () => controller.abort();
  }, [tenantId, userId, revision, onInspected]);
  const booleanLabel = (value: boolean | null) => value === null ? t('membership.access.unknown') : t(value ? 'membership.access.allowed' : 'membership.access.denied');
  return <section className="inline-form external-identity-editor" aria-label={t('membership.access.title')}>
    <h3>{t('membership.access.title')} · {report?.local_username || userId}</h3>
    <p>{t('membership.access.explanation')}</p>
    {error && <p role="alert">{t('membership.access.error')}</p>}
    {!report && !error && <p>{t('membership.access.loading')}</p>}
    {report && <>
      <dl className="external-identity-details">
        <dt>{t('membership.access.checkedAt')}</dt><dd>{new Date(report.inspected_at).toLocaleString(lang)}</dd>
        <dt>{t('membership.access.localAccess')}</dt><dd>{booleanLabel(report.source_access_allowed)}</dd>
        <dt>Rancher / project</dt><dd>{report.realm} · {report.project_id || t('membership.access.unknown')}</dd>
        <dt>{t('membership.access.identity')}</dt><dd>{report.identity_link ? `${report.identity_link.remote_username} · ${report.identity_link.remote_user_id}` : t('membership.access.blocker.identity_unlinked')}</dd>
        <dt>{t('membership.access.remoteActive')}</dt><dd>{booleanLabel(report.remote_account_enabled)}</dd>
        <dt>{t('membership.access.referenceRole')}</dt><dd>{report.reference_project_role || '—'}</dd>
        <dt>{t('membership.access.directMatch')}</dt><dd>{t(report.direct_project_role_matches ? 'membership.access.matches' : 'membership.access.differs')}</dd>
      </dl>
      {!!report.blockers.length && <ul>{report.blockers.map(code => <li key={code}>{t(`membership.access.blocker.${code}`)}</li>)}</ul>}
      {!report.source_access_allowed && report.direct_bindings.length > 0 && <p role="status">{t('membership.access.remainingAccess')}</p>}
      <h4>{t('membership.access.bindings')}</h4>
      {report.direct_bindings.length ? <ul>{report.direct_bindings.map(binding => <li key={binding.id}><code>{binding.role_template}</code> · {binding.id}</li>)}</ul> : <p>{t('membership.access.noBindings')}</p>}
      <p>{t('membership.access.unverified')}</p>
      <ul className="tenant-access-inspection-sources">{report.unverified_access_sources.map(code => <li key={code}>{t(`membership.access.source.${code}`)}</li>)}</ul>
      {report.reference_project_role && report.reference_project_role !== 'read-only' && <p>{t('membership.access.roleWarning')}</p>}
    </>}
    <div className="btn-group">
      <button className="btn btn-secondary" onClick={() => setRevision(revision + 1)}>{t('membership.access.refresh')}</button>
      <button className="btn btn-secondary" onClick={onClose}>{t('membership.access.close')}</button>
    </div>
  </section>;
}
