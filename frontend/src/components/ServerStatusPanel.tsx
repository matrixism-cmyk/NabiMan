import React from 'react';
import { useApi } from '../hooks/useApi';
import { ServerStatus } from '../types';
import { useT } from '../i18n';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return `${d}d ${h}h ${m}m`;
}

function ProgressBar({ value, max, label }: { value: number; max: number; label: string }) {
  const pct = max > 0 ? (value / max) * 100 : 0;
  const color = pct > 90 ? '#e74c3c' : pct > 70 ? '#f39c12' : '#2ecc71';
  return (
    <div className="progress-item">
      <div className="progress-label">
        <span>{label}</span>
        <span>{pct.toFixed(1)}%</span>
      </div>
      <div className="progress-bar">
        <div className="progress-fill" style={{ width: `${pct}%`, backgroundColor: color }} />
      </div>
    </div>
  );
}

export default function ServerStatusPanel() {
  const { t } = useT();
  const { data, loading, error } = useApi<ServerStatus>('/api/server/status', 5000);

  if (loading) return <div className="panel loading">{t('serverStatus.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  const alerts: { level: string; msg: string }[] = [];
  const cpuPct = data.cpu_usage;
  const memPct = data.memory_total > 0 ? (data.memory_used / data.memory_total) * 100 : 0;
  const diskPct = data.disk_total > 0 ? (data.disk_used / data.disk_total) * 100 : 0;
  if (cpuPct > 90) alerts.push({ level: 'danger', msg: t('serverStatus.alertCpu').replace('{v}', cpuPct.toFixed(1)) });
  else if (cpuPct > 70) alerts.push({ level: 'warning', msg: t('serverStatus.alertCpu').replace('{v}', cpuPct.toFixed(1)) });
  if (memPct > 90) alerts.push({ level: 'danger', msg: t('serverStatus.alertMem').replace('{v}', memPct.toFixed(1)) });
  else if (memPct > 80) alerts.push({ level: 'warning', msg: t('serverStatus.alertMem').replace('{v}', memPct.toFixed(1)) });
  if (diskPct > 90) alerts.push({ level: 'danger', msg: t('serverStatus.alertDisk').replace('{v}', diskPct.toFixed(1)) });
  else if (diskPct > 80) alerts.push({ level: 'warning', msg: t('serverStatus.alertDisk').replace('{v}', diskPct.toFixed(1)) });

  return (
    <div className="panel">
      <h2>{t('serverStatus.title')}</h2>
      {alerts.length > 0 && (
        <div className="alerts-banner">
          {alerts.map((a, i) => (
            <div key={i} className={`alert-item alert-${a.level}`}>{a.msg}</div>
          ))}
        </div>
      )}
      <div className="info-grid">
        <div className="info-item">
          <span className="info-label">{t('serverStatus.hostname')}</span>
          <span className="info-value">{data.hostname}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('serverStatus.os')}</span>
          <span className="info-value">{data.os}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('serverStatus.uptime')}</span>
          <span className="info-value">{formatUptime(data.uptime)}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('serverStatus.loadAvg')}</span>
          <span className="info-value">
            {data.load_average.map(v => v.toFixed(2)).join(' / ')}
          </span>
        </div>
      </div>
      <div className="progress-section">
        <ProgressBar value={data.cpu_usage} max={100} label={t('serverStatus.cpu')} />
        <ProgressBar value={data.memory_used} max={data.memory_total}
          label={`${t('serverStatus.memory')} (${formatBytes(data.memory_used)} / ${formatBytes(data.memory_total)})`} />
        <ProgressBar value={data.disk_used} max={data.disk_total}
          label={`${t('serverStatus.disk')} (${formatBytes(data.disk_used)} / ${formatBytes(data.disk_total)})`} />
      </div>
    </div>
  );
}
