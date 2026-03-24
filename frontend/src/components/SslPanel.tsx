import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { SslCertificate } from '../types';
import { useT } from '../i18n';

export default function SslPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<SslCertificate[]>('/api/ssl/certificates');
  const [renewMsg, setRenewMsg] = useState('');
  const [renewing, setRenewing] = useState('');

  const handleRenew = async (domain: string) => {
    if (!window.confirm(t('ssl.confirmRenew').replace('{domain}', domain))) return;
    setRenewing(domain);
    setRenewMsg('');
    const res = await apiPost<string>('/api/ssl/renew', { domain });
    setRenewMsg(res.success ? `${domain}: ${t('ssl.renewed')}` : `${domain}: ${res.message}`);
    setRenewing('');
    refetch();
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  const statusBadge = (cert: SslCertificate) => {
    const cls = cert.status === 'expired' ? 'down' : cert.status === 'warning' ? 'down' : 'up';
    const label = cert.status === 'expired' ? t('ssl.expired')
      : cert.status === 'warning' ? t('ssl.expiring') : t('ssl.valid');
    return <span className={`status-badge ${cls}`}>{label}</span>;
  };

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('ssl.title')}</h2>
        <button className="btn btn-secondary btn-sm" onClick={refetch}>{t('ssl.refresh')}</button>
      </div>
      {renewMsg && <div className="message" onClick={() => setRenewMsg('')}>{renewMsg}</div>}
      {data.length === 0 ? (
        <p className="text-secondary">{t('ssl.noCerts')}</p>
      ) : (
        <table className="data-table">
          <thead><tr>
            <th>{t('ssl.domain')}</th>
            <th>{t('common.status')}</th>
            <th>{t('ssl.expiry')}</th>
            <th>{t('ssl.daysLeft')}</th>
            <th>{t('common.actions')}</th>
          </tr></thead>
          <tbody>
            {data.map((cert, i) => (
              <tr key={i}>
                <td><strong>{cert.domain}</strong></td>
                <td>{statusBadge(cert)}</td>
                <td><code>{cert.expiry.split('+')[0]}</code></td>
                <td className={cert.days_left < 14 ? 'text-danger' : cert.days_left < 30 ? 'text-warning' : ''}>
                  {cert.days_left}{t('ssl.days')}
                </td>
                <td>
                  <button
                    className="btn btn-sm btn-primary"
                    onClick={() => handleRenew(cert.domain)}
                    disabled={renewing === cert.domain}
                  >
                    {renewing === cert.domain ? t('common.working') : t('ssl.renew')}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
