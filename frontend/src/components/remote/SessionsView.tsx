import React, { useState } from 'react';
import { apiPost, apiRequest } from '../../hooks/useApi';
import { useT } from '../../i18n';

export interface SessionInfo {
  id: string; label: string; owner: string; ws_count: number; shared: boolean;
  age_secs: number; timeout_secs: number; scrollback: number; keepalive: boolean; alive: boolean;
  state?: 'alive' | 'ended' | 'unknown' | 'blocked';
  /** A name the operator gave it; empty means it goes by its label. */
  title: string;
  /** The last lines on screen, for the thumbnail. */
  preview: string;
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
  onShare?: (session: SessionInfo) => void;
  /** Heading for the list; the fleet-wide list keeps the default. */
  title?: string;
  /** Extra buttons next to the heading, such as starting a new session. */
  actions?: React.ReactNode;
  /** What to show when nothing is running, in place of the plain notice. */
  empty?: React.ReactNode;
}

/** Every shell running on the server, with a look at what is on each screen. */
export default function SessionsView({
  sessions, currentSessionId, onRefresh, onAttach, onPopOut, onShare, title, actions, empty,
}: Props) {
  const { t } = useT();
  const [renaming, setRenaming] = useState<string | null>(null);
  const [draft, setDraft] = useState('');
  const [message, setMessage] = useState('');

  const startRename = (session: SessionInfo) => {
    setRenaming(session.id);
    setDraft(session.title || '');
  };

  const saveRename = async (session: SessionInfo) => {
    try {
      const res = await apiPost(`/api/terminal/sessions/${session.id}/name`, { name: draft });
      if (!res.success) { setMessage(res.message || t('common.error')); return; }
      setMessage('');
      setRenaming(null);
      onRefresh();
    } catch { setMessage(t('terminal.actionUncertain')); }
  };

  const kill = async (session: SessionInfo) => {
    if (!window.confirm(t('terminal.endSessionConfirm', { name: session.title || session.label }))) return;
    try {
      const res = await apiRequest(`/api/terminal/sessions/${session.id}`, { method: 'DELETE' });
      if (!res.success) { setMessage(res.message || t('common.error')); return; }
      setMessage('');
      onRefresh();
    } catch { setMessage(t('terminal.actionUncertain')); }
  };

  const toggle = async (session: SessionInfo, what: 'keepalive' | 'share') => {
    try {
      const body = what === 'keepalive' ? { keepalive: !session.keepalive } : { shared: !session.shared };
      const res = await apiPost(`/api/terminal/sessions/${session.id}/${what}`, body);
      if (!res.success) { setMessage(res.message || t('common.error')); return; }
      setMessage('');
      onRefresh();
    } catch { setMessage(t('terminal.actionUncertain')); }
  };

  const heading = title || t('terminal.activeSessions');

  if (sessions.length === 0) {
    return (
      <div className="rw-section">
        <div className="rw-section-head">
          <h3>{heading}</h3>
          {actions && <div className="btn-group">{actions}</div>}
        </div>
        {empty || <p className="text-secondary">{t('terminal.noSessions')}</p>}
      </div>
    );
  }

  return (
    <div className="rw-section">
      <div className="rw-section-head">
        <h3>{heading}</h3>
        <div className="btn-group">
          {actions}
          <button className="btn btn-secondary btn-sm" onClick={onRefresh}>{t('common.refresh')}</button>
        </div>
      </div>

      {message && <div role="alert" className="message">{message}</div>}
      <div className="session-grid">
        {sessions.map(session => {
          const state = session.state === undefined ? (session.alive ? 'alive' : 'ended') :
            ['alive', 'ended', 'unknown', 'blocked'].includes(session.state) ? session.state : 'unknown';
          const attachable = session.alive && state === 'alive';
          return (
          <div key={session.id} className={`session-card ${session.id === currentSessionId ? 'is-current' : ''}`}>
            <div className="session-card-head">
              {renaming === session.id ? (
                <form className="session-rename" onSubmit={(e) => { e.preventDefault(); saveRename(session); }}>
                  <input className="filter-input" value={draft} autoFocus maxLength={60}
                    placeholder={t('terminal.namePlaceholder')}
                    onChange={(e) => setDraft(e.target.value)} />
                  <button className="btn btn-primary btn-sm" type="submit">{t('common.save')}</button>
                  <button className="btn btn-secondary btn-sm" type="button" onClick={() => setRenaming(null)}>
                    {t('common.cancel')}
                  </button>
                </form>
              ) : (
                <>
                  <strong className="session-name">{session.title || session.label}</strong>
                  <button className="session-rename-btn" title={t('terminal.rename')} onClick={() => startRename(session)}>✎</button>
                  {state !== 'alive' && <span className="status-badge down">{t(state === 'unknown' ? 'terminal.sessionUnknown' : state === 'blocked' ? 'terminal.sessionBlocked' : 'share.ended')}</span>}
                  {session.id === currentSessionId && <span className="status-badge up">{t('terminal.thisPane')}</span>}
                </>
              )}
            </div>

            <code className="session-label">{session.label}</code>

            <pre className="session-preview" title={t('terminal.previewHint')}>
              {session.preview || t('terminal.noPreview')}
            </pre>

            <div className="session-meta">
              <span>{t('terminal.viewers')} {session.ws_count}</span>
              <span>{formatAge(session.age_secs)}</span>
              <span>{session.scrollback.toLocaleString()}줄</span>
              <span>{session.owner}</span>
            </div>

            <div className="session-actions">
              <button className="btn btn-primary btn-sm" disabled={!attachable} onClick={() => onAttach(session)}>
                {session.id === currentSessionId ? t('terminal.showPane') : t('terminal.join')}
              </button>
              <button className="btn btn-secondary btn-sm" title={t('remote.detachHint')} disabled={!attachable} onClick={() => onPopOut(session)}>⧉</button>
              {onShare && (
                <button className="btn btn-secondary btn-sm" title={t('share.hint')} disabled={!attachable} onClick={() => onShare(session)}>🔗</button>
              )}
              <button className={`btn btn-sm ${session.keepalive ? 'btn-primary' : 'btn-secondary'}`}
                title={t('terminal.keepaliveHint')} onClick={() => toggle(session, 'keepalive')}>
                {t('terminal.keepaliveShort')} {session.keepalive ? 'ON' : 'OFF'}
              </button>
              <button className={`btn btn-sm ${session.shared ? 'btn-warning' : 'btn-secondary'}`}
                disabled={!attachable && !session.shared} onClick={() => toggle(session, 'share')}>
                {session.shared ? t('terminal.shared') : t('terminal.share')}
              </button>
              <button className="btn btn-danger btn-sm" onClick={() => kill(session)}>{t('terminal.endSession')}</button>
            </div>
          </div>
        );})}
      </div>
    </div>
  );
}
