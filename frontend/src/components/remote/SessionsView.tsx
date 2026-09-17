import React from 'react';
import { apiPost, apiRequest } from '../../hooks/useApi';
import { useT } from '../../i18n';

export interface SessionInfo {
  id: string; label: string; owner: string; ws_count: number; shared: boolean;
  age_secs: number; timeout_secs: number; scrollback: number; keepalive: boolean; alive: boolean;
}

function formatAge(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}

interface Props {
  sessions: SessionInfo[];
  currentSessionId?: string;
  onRefresh: () => void;
  onAttach: (session: SessionInfo) => void;
  onPopOut: (session: SessionInfo) => void;
}

/** Every shell running on the server, wherever it was started from. */
export default function SessionsView({ sessions, currentSessionId, onRefresh, onAttach, onPopOut }: Props) {
  const { t } = useT();

  const kill = async (session: SessionInfo) => {
    if (!window.confirm(t('terminal.endSessionConfirm', { name: session.label }))) return;
    await apiRequest(`/api/terminal/sessions/${session.id}`, { method: 'DELETE' });
    onRefresh();
  };

  const toggleKeepalive = async (session: SessionInfo) => {
    await apiPost(`/api/terminal/sessions/${session.id}/keepalive`, { keepalive: !session.keepalive });
    onRefresh();
  };

  const toggleShare = async (session: SessionInfo) => {
    await apiPost(`/api/terminal/sessions/${session.id}/share`, { shared: !session.shared });
    onRefresh();
  };

  if (sessions.length === 0) {
    return (
      <div className="rw-section">
        <div className="rw-section-head"><h3>{t('terminal.activeSessions')}</h3></div>
        <p className="text-secondary">{t('terminal.noSessions')}</p>
      </div>
    );
  }

  return (
    <div className="rw-section">
      <div className="rw-section-head">
        <h3>{t('terminal.activeSessions')}</h3>
        <button className="btn btn-secondary btn-sm" onClick={onRefresh}>{t('common.refresh')}</button>
      </div>
      <div style={{ overflowX: 'auto' }}>
        <table className="data-table" style={{ fontSize: 12 }}>
          <thead>
            <tr>
              <th>{t('terminal.label')}</th><th>{t('remote.user')}</th><th>{t('terminal.viewers')}</th>
              <th>{t('terminal.age')}</th><th>{t('terminal.scrollbackShort')}</th>
              <th>{t('terminal.keepalive')}</th><th>{t('common.actions')}</th>
            </tr>
          </thead>
          <tbody>
            {sessions.map(s => (
              <tr key={s.id} style={{ background: s.id === currentSessionId ? 'rgba(59,130,246,0.1)' : undefined }}>
                <td>
                  <strong>{s.label}</strong>
                  {!s.alive && <span className="status-badge down" style={{ marginLeft: 6 }}>dead</span>}
                </td>
                <td>{s.owner}</td>
                <td>{s.ws_count}</td>
                <td>{formatAge(s.age_secs)}</td>
                <td>{s.scrollback.toLocaleString()}</td>
                <td>
                  <button className={`btn btn-sm ${s.keepalive ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => toggleKeepalive(s)}>{s.keepalive ? 'ON' : 'OFF'}</button>
                </td>
                <td>
                  {s.id !== currentSessionId && (
                    <button className="btn btn-primary btn-sm" style={{ marginRight: 4 }} onClick={() => onAttach(s)}>
                      {t('terminal.join')}
                    </button>
                  )}
                  <button className="btn btn-secondary btn-sm" style={{ marginRight: 4 }}
                    title={t('remote.detach')} onClick={() => onPopOut(s)}>⧉</button>
                  <button className={`btn btn-sm ${s.shared ? 'btn-warning' : 'btn-secondary'}`}
                    style={{ marginRight: 4 }} onClick={() => toggleShare(s)}>
                    {s.shared ? t('terminal.shared') : t('terminal.share')}
                  </button>
                  <button className="btn btn-danger btn-sm" onClick={() => kill(s)}>✕</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
