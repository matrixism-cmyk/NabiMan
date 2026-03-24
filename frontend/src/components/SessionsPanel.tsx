import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { useT } from '../i18n';

interface LoginSession { user: string; tty: string; from: string; login_time: string; idle: string; what: string; }

export default function SessionsPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<LoginSession[]>('/api/sessions', 10000);
  const { data: lastData } = useApi<string>('/api/sessions/last');
  const [msg, setMsg] = useState('');
  const [tab, setTab] = useState<'active' | 'last'>('active');

  const killSession = async (tty: string) => {
    if (!window.confirm(t('sessions.confirmKill').replace('{tty}', tty))) return;
    const res = await apiPost<string>('/api/sessions/kill', { tty });
    setMsg(res.data || res.message);
    refetch();
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('sessions.title')}</h2>
        <div className="btn-group">
          <button className={`btn btn-sm ${tab === 'active' ? 'btn-primary' : 'btn-secondary'}`} onClick={() => setTab('active')}>{t('sessions.active')}</button>
          <button className={`btn btn-sm ${tab === 'last' ? 'btn-primary' : 'btn-secondary'}`} onClick={() => setTab('last')}>{t('sessions.lastLogins')}</button>
          <button className="btn btn-secondary btn-sm" onClick={refetch}>{t('ssl.refresh')}</button>
        </div>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}
      {tab === 'active' ? (
        data && data.length > 0 ? (
          <table className="data-table">
            <thead><tr><th>{t('sessions.user')}</th><th>TTY</th><th>{t('sessions.from')}</th><th>{t('sessions.loginTime')}</th><th>{t('sessions.idle')}</th><th>{t('sessions.command')}</th><th>{t('common.actions')}</th></tr></thead>
            <tbody>
              {data.map((s, i) => (
                <tr key={i}>
                  <td><strong>{s.user}</strong></td><td>{s.tty}</td><td>{s.from}</td><td>{s.login_time}</td><td>{s.idle}</td><td><code>{s.what}</code></td>
                  <td><button className="btn btn-sm btn-danger" onClick={() => killSession(s.tty)}>{t('sessions.kill')}</button></td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : <p className="text-secondary">{t('sessions.noSessions')}</p>
      ) : (
        <pre className="log-output">{lastData || t('common.loading')}</pre>
      )}
    </div>
  );
}
