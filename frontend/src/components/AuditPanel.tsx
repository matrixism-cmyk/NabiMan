import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { useT } from '../i18n';

interface AuditEntry { timestamp: string; user: string; method: string; path: string; status: number; ip: string; }

export default function AuditPanel() {
  const { t } = useT();
  const [userFilter, setUserFilter] = useState('');
  const [methodFilter, setMethodFilter] = useState('');
  const [pathFilter, setPathFilter] = useState('');

  const params = new URLSearchParams();
  if (userFilter) params.set('user', userFilter);
  if (methodFilter) params.set('method', methodFilter);
  if (pathFilter) params.set('path_filter', pathFilter);
  const qs = params.toString();

  const { data, loading, error, refetch } = useApi<AuditEntry[]>(`/api/audit${qs ? '?' + qs : ''}`, 10000);

  const handleClear = async () => {
    if (!window.confirm(t('audit.confirmClear'))) return;
    await apiPost<string>('/api/audit/clear', {});
    refetch();
  };

  if (loading && !data) return <div className="panel loading">{t('common.loading')}</div>;
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
      <div className="filter-row">
        <input className="filter-input" placeholder={t('audit.filterUser')} value={userFilter}
          onChange={e => setUserFilter(e.target.value)} style={{ maxWidth: 150 }} />
        <select className="select-input" value={methodFilter} onChange={e => setMethodFilter(e.target.value)}>
          <option value="">{t('common.all')}</option>
          <option value="GET">GET</option><option value="POST">POST</option>
          <option value="PUT">PUT</option><option value="DELETE">DELETE</option>
        </select>
        <input className="filter-input" placeholder={t('audit.filterPath')} value={pathFilter}
          onChange={e => setPathFilter(e.target.value)} />
      </div>
      {data && data.length > 0 ? (
        <table className="data-table">
          <thead><tr>
            <th>{t('audit.time')}</th><th>{t('audit.user')}</th><th>Method</th>
            <th>{t('audit.path')}</th><th>Status</th><th>IP</th>
          </tr></thead>
          <tbody>
            {data.map((e, i) => (
              <tr key={i}>
                <td style={{ whiteSpace: 'nowrap' }}>{e.timestamp}</td>
                <td><strong>{e.user}</strong></td>
                <td><code>{e.method}</code></td>
                <td><code>{e.path}</code></td>
                <td className={e.status >= 400 ? 'text-danger' : ''}>{e.status}</td>
                <td>{e.ip}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : <p className="text-secondary">{t('audit.noEntries')}</p>}
    </div>
  );
}
