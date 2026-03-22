import React from 'react';
import { useApi } from '../hooks/useApi';
import { NetworkStatus } from '../types';
import { useT } from '../i18n';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

export default function NetworkPanel() {
  const { t } = useT();
  const { data, loading, error } = useApi<NetworkStatus>('/api/network/status', 5000);

  if (loading) return <div className="panel loading">{t('network.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  return (
    <div className="panel">
      <h2>{t('network.title')}</h2>
      <div className="info-item">
        <span className="info-label">{t('network.openConns')}</span>
        <span className="info-value">{data.open_connections}</span>
      </div>
      <div className="info-item">
        <span className="info-label">{t('network.dnsServers')}</span>
        <span className="info-value">{data.dns_servers.join(', ') || 'N/A'}</span>
      </div>

      <h3>{t('network.interfaces')}</h3>
      <table className="data-table">
        <thead>
          <tr>
            <th>{t('common.name')}</th>
            <th>{t('network.ip')}</th>
            <th>{t('network.mac')}</th>
            <th>{t('network.rx')}</th>
            <th>{t('network.tx')}</th>
            <th>{t('common.status')}</th>
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
                  {iface.is_up ? t('network.up') : t('network.down')}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
