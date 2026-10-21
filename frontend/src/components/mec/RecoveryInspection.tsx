import React, { useEffect, useState } from 'react';
import { useT } from '../../i18n';
import { fetchWithRefresh } from '../../shared/api/useApi';
import { StatusBadge } from './common';

export interface RecoveryReport {
  checked_at: string; configured_mode: string; records_conflict: boolean;
  profile: { namespace: string; project_id: string | null; user_id: string | null } | null;
  job_snapshot: { namespace: string; project_id: string | null; user_id: string | null } | null;
  observations: Array<{ resource: string; target: string | null;
    state: 'present' | 'absent' | 'unknown' | 'not_recorded' | 'candidates';
    expected_identity_matches: boolean | null; candidates: string[]; error_code: string | null }>;
}
export default function RecoveryInspection({ jobId }: { jobId: string }) {
  const { t } = useT();
  const [version, setVersion] = useState(0);
  const [report, setReport] = useState<RecoveryReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    const controller = new AbortController();
    setLoading(true); setReport(null); setError(null);
    (async () => {
      try {
        const response = await fetchWithRefresh(`${process.env.REACT_APP_API_URL || ''}/api/mec/v1/jobs/${encodeURIComponent(jobId)}/recovery`, { signal: controller.signal });
        const body = await response.json();
        if (!response.ok || body.error || !body.data) throw new Error(body.error?.message || `HTTP ${response.status}`);
        if (!controller.signal.aborted) setReport(body.data);
      } catch (e) {
        if (!controller.signal.aborted) setError(e instanceof Error ? e.message : t('mec.recovery.requestFailed'));
      } finally { if (!controller.signal.aborted) setLoading(false); }
    })();
    return () => controller.abort();
  }, [jobId, version, t]);
  return <section aria-label={t('mec.recovery.title')} style={{ padding: '14px', border: '1px solid var(--border)', borderRadius: '8px', marginBottom: '16px', background: 'var(--surface-alt)' }}>
    <div style={{ display: 'flex', justifyContent: 'space-between', gap: '12px', alignItems: 'center' }}>
      <h3>{t('mec.recovery.title')}: <code>{jobId}</code></h3>
      <button className="btn btn-secondary" disabled={loading} onClick={() => setVersion(v => v + 1)}>{t('mec.action.refresh')}</button>
    </div>
    <p>{t('mec.recovery.readOnly')}</p>
    {loading && <p role="status">{t('mec.state.loading')}</p>}
    {error && <p role="alert">{error}</p>}
    {report && <>
      <p>{t('mec.recovery.checkedAt')}: {new Date(report.checked_at).toLocaleString()}</p>
      {report.configured_mode === 'mock' && <p role="alert">{t('mec.recovery.demo')}</p>}
      {report.records_conflict && <p role="alert">{t('mec.recovery.conflict')}</p>}
      <div style={{ overflowX: 'auto' }}><table className="data-table" style={{ width: '100%' }}>
        <thead><tr><th>{t('mec.recovery.resource')}</th><th>{t('mec.recovery.target')}</th><th>{t('mec.jobs.colStatus')}</th><th>{t('mec.recovery.identity')}</th></tr></thead>
        <tbody>{report.observations.map(o => <tr key={o.resource}>
          <td>{t(`mec.recovery.resource.${o.resource}`)}</td>
          <td style={{ overflowWrap: 'anywhere' }}>{o.target || '—'}{o.candidates.length > 0 && <div>{t('mec.recovery.candidateIds')}: {o.candidates.join(', ')}</div>}</td>
          <td><StatusBadge tone={o.state === 'present' ? 'info' : o.state === 'unknown' || o.state === 'candidates' ? 'warning' : 'neutral'}>{t(`mec.recovery.state.${o.state}`)}</StatusBadge>{o.error_code && <div>{o.error_code}</div>}</td>
          <td>{o.expected_identity_matches === null ? '—' : t(o.expected_identity_matches ? 'mec.recovery.match' : 'mec.recovery.mismatch')}</td>
        </tr>)}</tbody>
      </table></div>
      <details><summary>{t('mec.recovery.savedRecords')}</summary>
        {[['profile', report.profile], ['job_snapshot', report.job_snapshot]].map(([source, record]) => {
          const r = record as RecoveryReport['profile'];
          return <p key={source as string}>{t(`mec.recovery.source.${source}`)}: {r ? `${r.namespace} / ${r.project_id || '—'} / ${r.user_id || '—'}` : t('mec.recovery.state.not_recorded')}</p>;
        })}
      </details>
    </>}
  </section>;
}
