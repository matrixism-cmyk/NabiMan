import React, { useState, useEffect, useRef } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { SslCertificate } from '../types';
import { useT } from '../i18n';
import { useSortable } from '../hooks/useSortable';

interface RenewStatus { domain: string; status: string; message: string; started_at: string; }

export default function SslPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<SslCertificate[]>('/api/ssl/certificates');
  const [renewMsg, setRenewMsg] = useState('');
  const [renewing, setRenewing] = useState('');
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => { return () => { if (pollRef.current) clearInterval(pollRef.current); }; }, []);

  const handleRenew = async (domain: string) => {
    if (!window.confirm(t('ssl.confirmRenew').replace('{domain}', domain))) return;
    setRenewing(domain);
    setRenewMsg(`${domain}: ${t('ssl.renewStarted')}`);
    await apiPost<RenewStatus>('/api/ssl/renew', { domain });
    // Poll for completion
    pollRef.current = setInterval(async () => {
      const res = await fetch(`/api/ssl/renew/${domain}`, {
        headers: { 'Authorization': `Bearer ${sessionStorage.getItem('nabiman_token')}` },
      });
      const json = await res.json();
      if (json.success && json.data) {
        const s = json.data as RenewStatus;
        if (s.status !== 'running') {
          if (pollRef.current) clearInterval(pollRef.current);
          setRenewing('');
          setRenewMsg(`${domain}: ${s.status === 'success' ? t('ssl.renewed') : s.message}`);
          refetch();
        }
      }
    }, 3000);
  };

  const { sorted, toggle, indicator } = useSortable(data || [], 'days_left', 'asc');
  const S = (key: string, label: string) => (
    <th className="sortable" onClick={() => toggle(key)}>{label}{indicator(key)}</th>
  );

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  const handleIssue = async (domain: string) => {
    if (!window.confirm(t('ssl.confirmIssue').replace('{domain}', domain))) return;
    setRenewing(domain);
    setRenewMsg(`${domain}: ${t('ssl.issueStarted')}`);
    await apiPost<any>('/api/ssl/issue', { domain });
    pollRef.current = setInterval(async () => {
      const res = await fetch(`/api/ssl/renew/${domain}`, {
        headers: { 'Authorization': `Bearer ${sessionStorage.getItem('nabiman_token')}` },
      });
      const json = await res.json();
      if (json.success && json.data && json.data.status !== 'running') {
        if (pollRef.current) clearInterval(pollRef.current);
        setRenewing('');
        setRenewMsg(`${domain}: ${json.data.status === 'success' ? t('ssl.issued') : json.data.message}`);
        refetch();
      }
    }, 3000);
  };

  const statusBadge = (cert: SslCertificate) => {
    if (cert.status === 'none') return <span className="status-badge down">{t('ssl.notIssued')}</span>;
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
            {S('domain', t('ssl.domain'))}{S('status', t('common.status'))}
            <th>{t('ssl.expiry')}</th>{S('days_left', t('ssl.daysLeft'))}<th>{t('common.actions')}</th>
          </tr></thead>
          <tbody>
            {sorted.map((cert, i) => (
              <tr key={i}>
                <td><strong>{cert.domain}</strong></td>
                <td>{statusBadge(cert)}</td>
                <td><code>{cert.expiry.split('+')[0]}</code></td>
                <td className={cert.days_left < 14 ? 'text-danger' : cert.days_left < 30 ? 'text-warning' : ''}>
                  {cert.days_left}{t('ssl.days')}
                </td>
                <td>
                  {cert.status === 'none' ? (
                    <button className="btn btn-sm btn-success" onClick={() => handleIssue(cert.domain)}
                      disabled={renewing === cert.domain}>
                      {renewing === cert.domain ? t('ssl.issuing') : t('ssl.issue')}
                    </button>
                  ) : (
                    <button className="btn btn-sm btn-primary" onClick={() => handleRenew(cert.domain)}
                      disabled={renewing === cert.domain}>
                      {renewing === cert.domain ? t('ssl.renewing') : t('ssl.renew')}
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
