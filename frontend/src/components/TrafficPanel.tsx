import React from 'react';
import { useApi } from '../hooks/useApi';
import { TrafficSnapshot, TrafficSummary } from '../types';
import { useT } from '../i18n';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function TrafficBar({ label, value, max }: { label: string; value: number; max: number }) {
  const pct = max > 0 ? Math.min((value / max) * 100, 100) : 0;
  return (
    <div className="traffic-bar-item">
      <span className="traffic-label">{label}</span>
      <div className="progress-bar">
        <div className="progress-fill traffic-fill" style={{ width: `${pct}%` }} />
      </div>
      <span className="traffic-value">{formatBytes(value)}/s</span>
    </div>
  );
}

export default function TrafficPanel() {
  const { t } = useT();
  const { data: snapshots, loading: loadSnap } =
    useApi<TrafficSnapshot[]>('/api/traffic/current', 3000);
  const { data: summary, loading: loadSum } =
    useApi<TrafficSummary>('/api/traffic/summary', 5000);

  if (loadSnap || loadSum) return <div className="panel loading">{t('traffic.loading')}</div>;

  const maxRate = snapshots
    ? Math.max(...snapshots.map(s => Math.max(s.rx_bytes_per_sec, s.tx_bytes_per_sec)), 1)
    : 1;

  return (
    <div className="panel">
      <h2>{t('traffic.title')}</h2>

      {summary && (
        <div className="info-grid">
          <div className="info-item">
            <span className="info-label">{t('traffic.totalRx')}</span>
            <span className="info-value">{summary.total_rx_mb} MB</span>
          </div>
          <div className="info-item">
            <span className="info-label">{t('traffic.totalTx')}</span>
            <span className="info-value">{summary.total_tx_mb} MB</span>
          </div>
          <div className="info-item">
            <span className="info-label">{t('traffic.activeConns')}</span>
            <span className="info-value">{summary.active_connections}</span>
          </div>
          <div className="info-item">
            <span className="info-label">{t('traffic.tcpStates')}</span>
            <span className="info-value">
              EST: {summary.tcp_connections.established} /
              LISTEN: {summary.tcp_connections.listen} /
              TW: {summary.tcp_connections.time_wait}
            </span>
          </div>
        </div>
      )}

      <h3>{t('traffic.realtime')}</h3>
      {(snapshots || []).map((snap) => (
        <div key={snap.interface} className="traffic-interface">
          <h4>{snap.interface}</h4>
          <TrafficBar label="RX" value={snap.rx_bytes_per_sec} max={maxRate * 1.2} />
          <TrafficBar label="TX" value={snap.tx_bytes_per_sec} max={maxRate * 1.2} />
          <div className="traffic-meta">
            <span>{t('traffic.connections')}: {snap.active_connections}</span>
            <span>{snap.timestamp}</span>
          </div>
        </div>
      ))}
    </div>
  );
}
