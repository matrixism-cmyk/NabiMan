import React from 'react';
import { useApi } from '../hooks/useApi';
import { useT } from '../i18n';

interface DbStatus { engine: string; running: boolean; version: string; uptime: string; connections: string; databases: string[]; slow_queries: string; }

export default function DatabasePanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<DbStatus>('/api/database/status', 10000);

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('db.title')}</h2>
        <button className="btn btn-secondary btn-sm" onClick={refetch}>{t('ssl.refresh')}</button>
      </div>
      <div className="info-grid">
        <div className="info-item"><span className="info-label">{t('db.engine')}</span><span className="info-value">{data.engine}</span></div>
        <div className="info-item"><span className="info-label">{t('db.version')}</span><span className="info-value">{data.version}</span></div>
        <div className="info-item"><span className="info-label">{t('serverStatus.uptime')}</span><span className="info-value">{data.uptime}</span></div>
        <div className="info-item"><span className="info-label">{t('db.connections')}</span><span className="info-value">{data.connections}</span></div>
        <div className="info-item"><span className="info-label">{t('db.slowQueries')}</span><span className="info-value">{data.slow_queries}</span></div>
      </div>
      <h3>{t('db.databases')}</h3>
      <div className="info-grid">
        {data.databases.map((db, i) => (
          <div key={i} className="info-item"><span className="info-value">{db}</span></div>
        ))}
      </div>
    </div>
  );
}
