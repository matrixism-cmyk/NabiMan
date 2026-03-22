import React from 'react';
import { useApi } from '../hooks/useApi';
import { DiskStatus } from '../types';

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
  const { data, loading, error } = useApi<DiskStatus>('/api/disks/status', 5000);

  if (loading) return <div className="panel loading">Loading disk status...</div>;
  if (error) return <div className="panel error">Error: {error}</div>;
  if (!data) return null;

  return (
    <div className="panel">
      <h2>Disk / Storage</h2>

      <h3>Partitions</h3>
      <table className="data-table">
        <thead>
          <tr>
            <th>Filesystem</th>
            <th>Mount</th>
            <th>Type</th>
            <th>Total</th>
            <th>Used</th>
            <th>Available</th>
            <th>Usage</th>
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
          <h3>Disk I/O</h3>
          <table className="data-table">
            <thead>
              <tr>
                <th>Device</th>
                <th>Reads/s</th>
                <th>Writes/s</th>
                <th>Read</th>
                <th>Write</th>
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
