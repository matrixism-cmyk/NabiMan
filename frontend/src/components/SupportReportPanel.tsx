import React, { useState } from 'react';
import { apiPost, useApi } from '../hooks/useApi';
import { useT } from '../i18n';
interface Section { state: 'ready' | 'busy' | 'unavailable' | 'too_large'; data: Record<string, number | boolean> | null }
interface Report {
  schema_version: number; report_id: string; generated_at: string;
  process: { runs_as_root: boolean };
  accounts: Section; login_sessions: Section; terminal_registry: Section; server_inventory: Section; auth_storage: Section; recent_jobs: Section;
}
export default function SupportReportPanel() {
  const { t } = useT();
  const { data: me } = useApi<{ role: string }>('/api/users/me');
  const [report, setReport] = useState<Report | null>(null);
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [reviewed, setReviewed] = useState(false);
  if (me?.role !== 'admin') return null;
  const text = report ? JSON.stringify(report, null, 2) + '\n' : '';
  const collect = async () => {
    setBusy(true); setError(''); setReport(null); setReviewed(false);
    try {
      const result = await apiPost<Report>('/api/diagnostics/support-report', {});
      if (!result.success || !result.data) throw new Error(result.message || t('common.error'));
      setReport(result.data);
    } catch (cause) {
      if (!(cause instanceof DOMException && cause.name === 'AbortError')) setError(cause instanceof Error ? cause.message : t('common.error'));
    } finally { setBusy(false); }
  };
  const download = () => {
    if (!report || !reviewed || busy) return;
    const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
    const link = document.createElement('a'); link.href = url; link.download = `nabiman-support-${report.report_id}.json`; link.click(); URL.revokeObjectURL(url);
  };
  const partial = report && [report.accounts, report.login_sessions, report.terminal_registry, report.server_inventory, report.auth_storage, report.recent_jobs].some(s => s.state !== 'ready');
  return <section aria-label={t('support.title')}>
    <h3>{t('support.title')}</h3>
    <p>{t('support.description')}</p><p>{t('support.localOnly')}</p>
    <button className="btn btn-primary" disabled={busy} onClick={collect}>{t(busy ? 'support.collecting' : 'support.collect')}</button>
    {error && <p role="alert">{error}</p>}
    {report && <>
      <p>{t('support.limits')}</p>
      {report.process.runs_as_root && <p role="status">{t('support.root')}</p>}
      {partial && <p role="alert">{t('support.partial')}</p>}
      <pre aria-label={t('support.preview')} style={{ maxHeight: 400, overflow: 'auto', whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', textAlign: 'left' }}>{text}</pre>
      <label><input type="checkbox" checked={reviewed} onChange={e => setReviewed(e.target.checked)} /> {t('support.reviewed')}</label>
      <p><button className="btn btn-secondary" disabled={!reviewed || busy} onClick={download}>{t('support.download')}</button></p>
    </>}
  </section>;
}
