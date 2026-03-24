import React from 'react';
import { useApi } from '../hooks/useApi';
import { useT } from '../i18n';

interface DnsStatus {
  service: { running: boolean; pid: string; memory: string; uptime: string };
  health: { ok: boolean; response: string; response_ms: number };
  records: { total: number; active: number; domains: number };
  users: { total: number; active: number; pending: number; disabled: number };
  metrics: { queries_total: string; cache_hits: string; cache_misses: string; cache_hit_ratio: string; responses_nxdomain: string; rate_limited: string; acl_denied: string } | null;
  recent_changes: { name: string; rtype: string; updated_at: string }[];
}

export default function DnsPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<DnsStatus>('/api/dns/status', 15000);

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  const { service, health, records, users, metrics, recent_changes } = data;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('dns.title')}</h2>
        <button className="btn btn-secondary btn-sm" onClick={refetch}>{t('ssl.refresh')}</button>
      </div>

      {/* Service Status + Health */}
      <div className="info-grid">
        <div className="info-item">
          <span className="info-label">{t('dns.service')}</span>
          <span className={`status-badge ${service.running ? 'up' : 'down'}`}>
            {service.running ? t('dns.running') : t('dns.stopped')}
          </span>
        </div>
        <div className="info-item">
          <span className="info-label">PID</span>
          <span className="info-value">{service.pid}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('dns.memory')}</span>
          <span className="info-value">{service.memory}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('dns.startedAt')}</span>
          <span className="info-value">{service.uptime}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('dns.healthCheck')}</span>
          <span className={`status-badge ${health.ok ? 'up' : 'down'}`}>
            {health.ok ? t('dns.healthy') : t('dns.unhealthy')}
          </span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('dns.responseTime')}</span>
          <span className="info-value">{health.response_ms}ms</span>
        </div>
      </div>

      {/* Record & User Stats */}
      <h3>{t('dns.statistics')}</h3>
      <div className="info-grid">
        <div className="info-item">
          <span className="info-label">{t('dns.totalRecords')}</span>
          <span className="info-value">{records.total}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('dns.activeRecords')}</span>
          <span className="info-value">{records.active}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('dns.domains')}</span>
          <span className="info-value">{records.domains}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('dns.totalUsers')}</span>
          <span className="info-value">{users.total}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('dns.activeUsers')}</span>
          <span className="info-value">{users.active}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('dns.pendingUsers')}</span>
          <span className="info-value">{users.pending}</span>
        </div>
      </div>

      {/* Metrics (if available) */}
      {metrics && (
        <>
          <h3>{t('dns.metrics')}</h3>
          <div className="info-grid">
            <div className="info-item"><span className="info-label">{t('dns.queries')}</span><span className="info-value">{metrics.queries_total}</span></div>
            <div className="info-item"><span className="info-label">{t('dns.cacheHits')}</span><span className="info-value">{metrics.cache_hits}</span></div>
            <div className="info-item"><span className="info-label">{t('dns.cacheMisses')}</span><span className="info-value">{metrics.cache_misses}</span></div>
            <div className="info-item"><span className="info-label">{t('dns.cacheHitRatio')}</span><span className="info-value">{metrics.cache_hit_ratio}</span></div>
            <div className="info-item"><span className="info-label">NXDOMAIN</span><span className="info-value">{metrics.responses_nxdomain}</span></div>
            <div className="info-item"><span className="info-label">{t('dns.rateLimited')}</span><span className="info-value">{metrics.rate_limited}</span></div>
          </div>
        </>
      )}

      {/* Recent Changes */}
      <h3>{t('dns.recentChanges')}</h3>
      {recent_changes.length > 0 ? (
        <table className="data-table">
          <thead><tr><th>{t('common.name')}</th><th>{t('dns.recordType')}</th><th>{t('dns.updatedAt')}</th></tr></thead>
          <tbody>
            {recent_changes.map((r, i) => (
              <tr key={i}><td><strong>{r.name}</strong></td><td><code>{r.rtype}</code></td><td>{r.updated_at}</td></tr>
            ))}
          </tbody>
        </table>
      ) : <p className="text-secondary">{t('dns.noChanges')}</p>}
    </div>
  );
}
