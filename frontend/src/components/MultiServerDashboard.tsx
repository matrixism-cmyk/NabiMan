import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { RemoteServer, RemoteServerStatus } from '../types';
import { useT } from '../i18n';

export default function MultiServerDashboard() {
  const { t } = useT();
  const { data: servers, refetch } = useApi<RemoteServer[]>('/api/remote-servers', 30000);
  const [statuses, setStatuses] = useState<Record<string, RemoteServerStatus>>({});
  const [checking, setChecking] = useState(false);

  const checkAll = async () => {
    setChecking(true);
    const res = await apiPost<RemoteServerStatus[]>('/api/remote-servers/check-all', {});
    if (res.success && res.data) {
      const map: Record<string, RemoteServerStatus> = {};
      res.data.forEach(s => { map[s.id] = s; });
      setStatuses(map);
    }
    refetch();
    setChecking(false);
  };

  const list = servers || [];
  const online = list.filter(s => s.status === 'online').length;
  const offline = list.filter(s => s.status === 'offline').length;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('remote.dashboard')}</h2>
        <button className="btn btn-primary btn-sm" onClick={checkAll} disabled={checking}>
          {checking ? t('common.working') : t('remote.checkAll')}
        </button>
      </div>

      <div className="kpi-grid" style={{ marginBottom: 16 }}>
        <div className="kpi-card">
          <div className="kpi-value" style={{ color: 'var(--primary)' }}>{list.length}</div>
          <div className="kpi-label">{t('remote.totalServers')}</div>
        </div>
        <div className="kpi-card">
          <div className="kpi-value" style={{ color: 'var(--success)' }}>{online}</div>
          <div className="kpi-label">{t('common.online')}</div>
        </div>
        <div className="kpi-card">
          <div className="kpi-value" style={{ color: offline > 0 ? 'var(--danger)' : 'var(--text-secondary)' }}>{offline}</div>
          <div className="kpi-label">{t('common.offline')}</div>
        </div>
        <div className="kpi-card">
          <div className="kpi-value" style={{ color: 'var(--text-secondary)' }}>{list.length - online - offline}</div>
          <div className="kpi-label">{t('common.unknown')}</div>
        </div>
      </div>

      {list.length === 0 ? (
        <p className="text-secondary">{t('remote.noServers')}</p>
      ) : (
        <div className="server-grid">
          {list.map(srv => {
            const st = statuses[srv.id];
            const color = srv.status === 'online' ? 'var(--success)' : srv.status === 'offline' ? 'var(--danger)' : 'var(--text-secondary)';
            return (
              <div key={srv.id} className="kpi-card" style={{ textAlign: 'left', borderLeft: `3px solid ${color}` }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
                  <strong>{srv.name}</strong>
                  <span className={`status-badge ${srv.status === 'online' ? 'up' : 'down'}`} style={{ fontSize: 11 }}>
                    {srv.status || 'unknown'}
                  </span>
                </div>
                <div style={{ fontSize: 12, color: 'var(--text-secondary)', marginBottom: 4 }}>
                  {srv.user}@{srv.host}:{srv.port}
                </div>
                {st ? (
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '4px 12px', fontSize: 12, marginTop: 8 }}>
                    <span>CPU: <strong>{st.cpu_usage}</strong></span>
                    <span>MEM: <strong>{st.memory}</strong></span>
                    <span>Disk: <strong>{st.disk}</strong></span>
                    <span>Load: <strong>{st.load}</strong></span>
                    <span style={{ gridColumn: '1 / -1', color: 'var(--text-secondary)', fontSize: 11 }}>
                      {st.os} / up {st.uptime}
                    </span>
                  </div>
                ) : srv.last_checked ? (
                  <div style={{ fontSize: 11, color: 'var(--text-secondary)', marginTop: 4 }}>
                    Last: {srv.last_checked}
                  </div>
                ) : null}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
