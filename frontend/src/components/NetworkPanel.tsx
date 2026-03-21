import React from 'react';
import { useApi } from '../hooks/useApi';
import { NetworkStatus } from '../types';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

export default function NetworkPanel() {
  const { data, loading, error } = useApi<NetworkStatus>('/api/network/status', 5000);

  if (loading) return <div className="panel loading">Loading network info...</div>;
  if (error) return <div className="panel error">Error: {error}</div>;
  if (!data) return null;

  return (
    <div className="panel">
      <h2>Network Status</h2>
      <div className="info-item">
        <span className="info-label">Open Connections</span>
        <span className="info-value">{data.open_connections}</span>
      </div>
      <div className="info-item">
        <span className="info-label">DNS Servers</span>
        <span className="info-value">{data.dns_servers.join(', ') || 'N/A'}</span>
      </div>

      <h3>Interfaces</h3>
      <table className="data-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>IP</th>
            <th>MAC</th>
            <th>RX</th>
            <th>TX</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {data.interfaces.map((iface) => (
            <tr key={iface.name}>
              <td>{iface.name}</td>
              <td>{iface.ip_address}</td>
              <td><code>{iface.mac_address}</code></td>
              <td>{formatBytes(iface.rx_bytes)}</td>
              <td>{formatBytes(iface.tx_bytes)}</td>
              <td>
                <span className={`status-badge ${iface.is_up ? 'up' : 'down'}`}>
                  {iface.is_up ? 'UP' : 'DOWN'}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
