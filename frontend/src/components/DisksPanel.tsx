import React from 'react';
import { useApi } from '../hooks/useApi';
import { DiskStatus } from '../types';
import { useT } from '../i18n';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function UsageBar({ percent }: { percent: number }) {
  const color = percent > 90 ? '#e74c3c' : percent > 70 ? '#f39c12' : '#2ecc71';
  return (
    <div className="progress-bar" style={{ width: '120px', display: 'inline-block' }}>
      <div className="progress-fill" style={{ width: `${percent}%`, backgroundColor: color }} />
    </div>
  );
}

export default function DisksPanel() {
  const { t } = useT();
  const { data, loading, error } = useApi<DiskStatus>('/api/disks/status', 5000);

  if (loading) return <div className="panel loading">{t('disks.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  return (
    <div className="panel">
      <h2>{t('disks.title')}</h2>

      <h3>{t('disks.partitions')}</h3>
      <table className="data-table">
        <thead>
          <tr>
            <th>{t('disks.filesystem')}</th>
            <th>{t('disks.mount')}</th>
            <th>{t('disks.type')}</th>
            <th>{t('disks.total')}</th>
            <th>{t('disks.used')}</th>
            <th>{t('disks.available')}</th>
            <th>{t('disks.usage')}</th>
          </tr>
        </thead>
        <tbody>
          {data.partitions.map((p, i) => (
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
    </div>
  );
}
