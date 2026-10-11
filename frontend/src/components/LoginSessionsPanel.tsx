import React, { useState } from 'react';
import { apiPost, useApi } from '../hooks/useApi';
import { useT } from '../i18n';
interface LoginSession {
  session_id: string; user_id: string; username: string; created_at: string;
  last_active: string; ip: string; is_current: boolean;
}
export default function LoginSessionsPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<LoginSession[]>('/api/auth/sessions');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const revoke = async (session?: LoginSession) => {
    if (!window.confirm(t(session ? 'loginSessions.confirmOne' : 'loginSessions.confirmOthers'))) return;
    setBusy(true); setMessage('');
    try {
      const result = await apiPost<string>(session ? '/api/auth/sessions/revoke' : '/api/auth/sessions/revoke-others',
        session ? { session_id: session.session_id } : {});
      if (!result.success) throw new Error(result.message || t('common.error'));
      setMessage(t('loginSessions.revoked')); refetch();
    } catch (cause) {
      if (!(cause instanceof DOMException && cause.name === 'AbortError')) setMessage(cause instanceof Error ? cause.message : t('common.error'));
    } finally { setBusy(false); }
  };
  return <section className="panel">
    <div className="panel-header"><h2>{t('loginSessions.title')}</h2>
      <div className="btn-group">
        <button className="btn btn-secondary" disabled={busy || loading} onClick={refetch}>{t('common.refresh')}</button>
        <button className="btn btn-danger" disabled={busy || loading || !!error} onClick={() => revoke()}>{t('loginSessions.others')}</button>
      </div>
    </div>
    <p>{t('loginSessions.hint')}</p>
    {message && <p role="status">{message}</p>}
    {loading && <p>{t('common.loading')}</p>}
    {error && <p role="alert">{error}</p>}
    {!error && data && <table className="data-table"><thead><tr>
      <th>{t('sessions.user')}</th><th>IP</th><th>{t('loginSessions.created')}</th>
      <th>{t('loginSessions.refreshed')}</th><th>{t('common.actions')}</th>
    </tr></thead><tbody>{data.map(session => <tr key={session.session_id} data-session-id={session.session_id}>
      <td>{session.username}</td><td>{session.ip}</td><td>{session.created_at}</td><td>{session.last_active}</td>
      <td>{session.is_current ? <strong>{t('loginSessions.current')}</strong> :
        <button className="btn btn-danger btn-sm" disabled={busy} onClick={() => revoke(session)}>{t('loginSessions.revoke')}</button>}</td>
    </tr>)}</tbody></table>}
  </section>;
}
