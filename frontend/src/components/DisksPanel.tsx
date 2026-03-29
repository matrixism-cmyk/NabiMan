import React from 'react';
import { useApi } from '../hooks/useApi';
import { DiskStatus } from '../types';
import { useT } from '../i18n';
import { useSortable } from '../hooks/useSortable';

interface HistoryPoint { timestamp: number; disk_read_bytes_sec: number; disk_write_bytes_sec: number; }

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function fmtSpeed(b: number): string {
  if (b < 1024) return b + ' B/s';
  if (b < 1048576) return (b / 1024).toFixed(1) + ' KB/s';
  return (b / 1048576).toFixed(1) + ' MB/s';
}

function UsageBar({ percent }: { percent: number }) {
  const color = percent > 90 ? '#e74c3c' : percent > 70 ? '#f39c12' : '#2ecc71';
  return (
    <div className="progress-bar" style={{ width: '120px', display: 'inline-block' }}>
      <div className="progress-fill" style={{ width: `${percent}%`, backgroundColor: color }} />
    </div>
  );
}

function IoTrendChart({ data, valueKey, color, label }: {
  data: HistoryPoint[]; valueKey: 'disk_read_bytes_sec' | 'disk_write_bytes_sec'; color: string; label: string;
}) {
  if (data.length < 2) return <div className="text-secondary">Collecting data...</div>;
  const values = data.map(d => d[valueKey]);
  const max = Math.max(...values, 1);
  const w = 100 / values.length;
  const latest = values[values.length - 1];
  return (
    <div className="chart-box">
      <div className="chart-header"><span>{label}</span><span className="chart-value">{fmtSpeed(latest)}</span></div>
      <svg viewBox="0 0 100 30" preserveAspectRatio="none" className="chart-svg">
        <polyline fill="none" stroke={color} strokeWidth="0.5"
          points={values.map((v, i) => `${i * w},${30 - (v / max) * 28}`).join(' ')} />
        <polyline fill={color} fillOpacity="0.1" stroke="none"
          points={`0,30 ${values.map((v, i) => `${i * w},${30 - (v / max) * 28}`).join(' ')} ${(values.length - 1) * w},30`} />
      </svg>
    </div>
  );
}

export default function DisksPanel() {
  const { t } = useT();
  const { data, loading, error } = useApi<DiskStatus>('/api/disks/status', 5000);
  const { data: history } = useApi<HistoryPoint[]>('/api/server/history?hours=1', 10000);

  const { sorted, toggle, indicator } = useSortable(data?.partitions || [], 'use_percent', 'desc');
  const S = (key: string, label: string) => (
    <th className="sortable" onClick={() => toggle(key)}>{label}{indicator(key)}</th>
  );

  if (loading) return <div className="panel loading">{t('disks.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  const pts = history || [];

  return (
    <div className="panel">
      <h2>{t('disks.title')}</h2>

      <h3>{t('disks.partitions')}</h3>
      <table className="data-table">
        <thead>
          <tr>
            {S('filesystem', t('disks.filesystem'))}
            {S('mount_point', t('disks.mount'))}
            <th>{t('disks.type')}</th>
            {S('total', t('disks.total'))}
            {S('used', t('disks.used'))}
            <th>{t('disks.available')}</th>
            {S('use_percent', t('disks.usage'))}
          </tr>
        </thead>
        <tbody>
          {sorted.map((p, i) => (
            <tr key={i}>
              <td><code>{p.filesystem}</code></td>
              <td><strong>{p.mount_point}</strong></td>
              <td>{p.fs_type}</td>
              <td>{formatBytes(p.total)}</td>
              <td>{formatBytes(p.used)}</td>
              <td>{formatBytes(p.available)}</td>
              <td>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <UsageBar percent={p.use_percent} />
                  <span className={p.use_percent > 90 ? 'text-danger' : p.use_percent > 70 ? 'text-warning' : ''}>
                    {p.use_percent.toFixed(1)}%
                  </span>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {data.io.length > 0 && (
        <>
          <h3>{t('disks.diskIo')}</h3>
          <table className="data-table">
            <thead>
              <tr>
                <th>{t('disks.device')}</th>
                <th>{t('disks.readsPerSec')}</th>
                <th>{t('disks.writesPerSec')}</th>
                <th>{t('disks.read')}</th>
                <th>{t('disks.write')}</th>
              </tr>
            </thead>
            <tbody>
              {data.io.map((d, i) => (
                <tr key={i}>
                  <td><strong>{d.device}</strong></td>
                  <td>{d.reads_per_sec.toFixed(1)}</td>
                  <td>{d.writes_per_sec.toFixed(1)}</td>
                  <td>{formatBytes(d.read_bytes_per_sec)}/s</td>
                  <td>{formatBytes(d.write_bytes_per_sec)}/s</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {pts.length > 1 && (
        <>
          <h3>{t('disks.ioTrend')}</h3>
          <div className="charts-grid">
            <IoTrendChart data={pts} valueKey="disk_read_bytes_sec" color="#a78bfa" label={t('charts.diskRead')} />
            <IoTrendChart data={pts} valueKey="disk_write_bytes_sec" color="#f472b6" label={t('charts.diskWrite')} />
          </div>
        </>
      )}
    </div>
  );
}
