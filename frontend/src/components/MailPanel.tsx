import React from 'react';
import { useApi } from '../hooks/useApi';
import { useT } from '../i18n';

interface MailStatus { running: boolean; server: string; queue_count: number; queue_entries: MailQueueEntry[]; stats: MailStats; }
interface MailQueueEntry { id: string; size: string; sender: string; recipient: string; status: string; }
interface MailStats { sent_today: number; received_today: number; bounced_today: number; deferred: number; }

export default function MailPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<MailStatus>('/api/mail/status', 15000);

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('mail.title')}</h2>
        <button className="btn btn-secondary btn-sm" onClick={refetch}>{t('ssl.refresh')}</button>
      </div>
      <div className="info-grid">
        <div className="info-item"><span className="info-label">{t('mail.server')}</span><span className="info-value">{data.server}</span></div>
        <div className="info-item"><span className="info-label">{t('mail.queue')}</span><span className="info-value">{data.queue_count}</span></div>
        <div className="info-item"><span className="info-label">{t('mail.sent')}</span><span className="info-value">{data.stats.sent_today}</span></div>
        <div className="info-item"><span className="info-label">{t('mail.received')}</span><span className="info-value">{data.stats.received_today}</span></div>
        <div className="info-item"><span className="info-label">{t('mail.bounced')}</span><span className="info-value">{data.stats.bounced_today}</span></div>
        <div className="info-item"><span className="info-label">{t('mail.deferred')}</span><span className="info-value">{data.stats.deferred}</span></div>
      </div>
      {data.queue_entries.length > 0 && (
        <>
          <h3>{t('mail.queueDetail')}</h3>
          <table className="data-table">
            <thead><tr><th>ID</th><th>{t('mail.sender')}</th><th>{t('mail.recipient')}</th><th>{t('mail.msgSize')}</th></tr></thead>
            <tbody>
              {data.queue_entries.slice(0, 50).map((e, i) => (
                <tr key={i}><td><code>{e.id}</code></td><td>{e.sender}</td><td>{e.recipient}</td><td>{e.size}</td></tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}
