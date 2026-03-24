import React from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { useT } from '../i18n';

interface AuditEntry { timestamp: string; action: string; path: string; detail: string; }

export default function AuditPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<AuditEntry[]>('/api/audit');

  const handleClear = async () => {
    if (!window.confirm(t('audit.confirmClear'))) return;
    await apiPost<string>('/api/audit/clear', {});
    refetch();
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('audit.title')}</h2>
        <div className="btn-group">
          <button className="btn btn-secondary btn-sm" onClick={refetch}>{t('ssl.refresh')}</button>
          <button className="btn btn-danger btn-sm" onClick={handleClear}>{t('audit.clear')}</button>
        </div>
      </div>
      {data && data.length > 0 ? (
        <table className="data-table">
          <thead><tr><th>{t('audit.time')}</th><th>{t('audit.action')}</th><th>{t('audit.path')}</th><th>{t('audit.detail')}</th></tr></thead>
          <tbody>
            {data.map((e, i) => (
              <tr key={i}><td>{e.timestamp}</td><td>{e.action}</td><td><code>{e.path}</code></td><td>{e.detail}</td></tr>
            ))}
          </tbody>
        </table>
      ) : <p className="text-secondary">{t('audit.noEntries')}</p>}
    </div>
  );
}
