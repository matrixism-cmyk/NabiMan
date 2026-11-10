import React, { useState, useEffect, useRef } from 'react';
import { useApi, apiPost, apiRequest } from '../shared/api/useApi';
import { SslCertificate } from '../types';
import { useT } from '../i18n';
import { useSortable } from '../hooks/useSortable';
import { confirmDialog } from '../shared/ui/ConfirmDialog';
import { startVisibleInterval } from '../shared/ui/useVisibleInterval';
import DismissibleMessage from '../shared/ui/DismissibleMessage';

interface RenewStatus { domain: string; status: string; message: string; started_at: string; }

export default function SslPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<SslCertificate[]>('/api/ssl/certificates');
  const [renewMsg, setRenewMsg] = useState('');
  const [renewing, setRenewing] = useState('');
  const [renewError, setRenewError] = useState('');
  const pollRef = useRef<(() => void) | null>(null);

  useEffect(() => () => { pollRef.current?.(); }, []);

  // Asks every 3 s how the certbot run for `domain` went, until it is over.
  // A failed ask is said on screen and the polling stops: it would only fail again.
  const pollStatus = (domain: string, doneKey: string) => {
    pollRef.current?.();
    const stop = () => { pollRef.current?.(); pollRef.current = null; setRenewing(''); };
    pollRef.current = startVisibleInterval(async () => {
      try {
        const json = await apiRequest<RenewStatus>(`/api/ssl/renew/${encodeURIComponent(domain)}`);
        if (!json.success) { stop(); setRenewError(`${domain}: ${json.message || t('ssl.statusFailed')}`); return; }
        if (json.data && json.data.status !== 'running') {
          stop();
          setRenewMsg(`${domain}: ${json.data.status === 'success' ? t(doneKey) : json.data.message}`);
          refetch();
        }
      } catch (e) {
        if ((e as Error).name === 'AbortError') return;
        stop();
        setRenewError(`${domain}: ${t('ssl.statusFailed')} (${e instanceof Error ? e.message : String(e)})`);
      }
    }, 3000);
  };

  const start = async (domain: string, path: string, startedKey: string, doneKey: string) => {
    setRenewing(domain);
    setRenewError('');
    setRenewMsg(`${domain}: ${t(startedKey)}`);
    try {
      const res = await apiPost<RenewStatus>(path, { domain });
      if (!res.success) { setRenewing(''); setRenewMsg(''); setRenewError(`${domain}: ${res.message}`); return; }
    } catch (e) {
      setRenewing(''); setRenewMsg(''); setRenewError(`${domain}: ${e instanceof Error ? e.message : String(e)}`);
      return;
    }
    pollStatus(domain, doneKey);
  };

  const handleRenew = async (domain: string) => {
    if (!(await confirmDialog(t('ssl.confirmRenew').replace('{domain}', domain)))) return;
    await start(domain, '/api/ssl/renew', 'ssl.renewStarted', 'ssl.renewed');
  };

  const { sorted, indicator, headerProps } = useSortable(data || [], 'days_left', 'asc');
  const S = (key: string, label: string) => (
    <th {...headerProps(key)}>{label}{indicator(key)}</th>
  );

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  const handleIssue = async (domain: string) => {
    if (!(await confirmDialog(t('ssl.confirmIssue').replace('{domain}', domain)))) return;
    await start(domain, '/api/ssl/issue', 'ssl.issueStarted', 'ssl.issued');
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
      {renewMsg && <DismissibleMessage onDismiss={() => setRenewMsg('')}>{renewMsg}</DismissibleMessage>}
      {renewError && <div role="alert" className="message" onClick={() => setRenewError('')}>{renewError}</div>}
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
