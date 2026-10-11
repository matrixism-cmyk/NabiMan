import React, { useState } from 'react';
import { apiRequest, apiPost, useApi } from '../hooks/useApi';
import { useT } from '../i18n';
interface Segment { name: string; valid: boolean; entries_checked: number; malformed_lines: number; hash_mismatches: number; unhashed_entries: number; truncated: boolean; sha256: string | null; error: string | null }
interface Verification { valid: boolean; entries_checked: number; segments: Segment[] }
interface Export { schema_version: number; segment: string; verification: Segment; sanitized: boolean; entries: unknown[] }
export default function AuditIntegrityPanel() {
  const { t } = useT();
  const { data: health, error: healthError } = useApi<{ writable: boolean; unparsed_startup_lines: number; error: string | null; last_persisted_at: string | null }>('/api/audit/health', 10000);
  const [verification, setVerification] = useState<Verification | null>(null);
  const [bundle, setBundle] = useState<Export | null>(null);
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [reviewed, setReviewed] = useState(false);
  const verify = async () => {
    setBusy(true); setError(''); setVerification(null); setBundle(null); setReviewed(false);
    try { const result = await apiRequest<Verification>('/api/audit/verify'); if (!result.success || !result.data) throw new Error(result.message || t('common.error')); setVerification(result.data); }
    catch (e) { setError(e instanceof Error ? e.message : t('common.error')); } finally { setBusy(false); }
  };
  const prepare = async (segment: string) => {
    setBusy(true); setError(''); setBundle(null); setReviewed(false);
    try { const result = await apiPost<Export>('/api/audit/export', { segment }); if (!result.success || !result.data) throw new Error(result.message || t('common.error')); setBundle(result.data); }
    catch (e) { setError(e instanceof Error ? e.message : t('common.error')); } finally { setBusy(false); }
  };
  const download = () => {
    if (!bundle || !reviewed || busy) return;
    const url = URL.createObjectURL(new Blob([JSON.stringify(bundle, null, 2) + '\n'], { type: 'application/json' }));
    const link = document.createElement('a'); link.href = url; link.download = `nabiman-${bundle.segment}-review.json`; link.click(); URL.revokeObjectURL(url);
  };
  return <section aria-label={t('audit.integrity')} style={{ textAlign: 'left' }}>
    <h3>{t('audit.integrity')}</h3>
    <p role={health?.writable ? 'status' : 'alert'}>{healthError ? t('audit.healthUnknown') : !health ? t('common.loading') : health.writable ? t('audit.writable') : t('audit.writeFailed')}{health?.error && <> ({health.error})</>}</p>
    {Boolean(health?.unparsed_startup_lines) && <p role="alert">{t('audit.unparsed')} ({health?.unparsed_startup_lines})</p>}
    <p>{t('audit.limits')}</p>
    <button className="btn btn-secondary" disabled={busy} onClick={verify}>{t('audit.verify')}</button>
    {error && <p role="alert">{error}</p>}
    {verification && <>
      <p role={verification.valid ? 'status' : 'alert'}>{t(verification.valid ? 'audit.consistent' : 'audit.inconsistent')} ({verification.entries_checked})</p>
      <div style={{ overflowX: 'auto' }}><table className="data-table"><thead><tr><th>{t('audit.segment')}</th><th>{t('audit.records')}</th><th>{t('audit.problems')}</th><th>{t('audit.export')}</th></tr></thead><tbody>
        {verification.segments.map(s => <tr key={s.name}><td>{s.name}</td><td>{s.entries_checked}</td><td>{s.error || `${s.hash_mismatches} / ${s.unhashed_entries} / ${s.malformed_lines}${s.truncated ? ' / EOF' : ''}`}</td><td><button className="btn btn-secondary btn-sm" disabled={busy || Boolean(s.error)} onClick={() => prepare(s.name)}>{t('audit.prepare')}</button></td></tr>)}
      </tbody></table></div>
    </>}
    {bundle && <>
      <p>{t('audit.exportLimits')}</p><p>{bundle.segment} · {bundle.entries.length} · SHA-256 <code style={{ overflowWrap: 'anywhere' }}>{bundle.verification.sha256}</code></p>
      <pre aria-label={t('audit.preview')} style={{ maxHeight: 260, overflow: 'auto', whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', textAlign: 'left' }}>{JSON.stringify(bundle.entries.slice(0, 20), null, 2)}</pre>
      <label><input type="checkbox" checked={reviewed} onChange={e => setReviewed(e.target.checked)} /> {t('audit.reviewed')}</label>
      <p><button className="btn btn-secondary" disabled={busy || !reviewed} onClick={download}>{t('audit.download')}</button></p>
    </>}
  </section>;
}
