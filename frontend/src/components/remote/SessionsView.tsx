import React, { useState } from 'react';
import { apiPost, apiRequest } from '../../hooks/useApi';
import { useT } from '../../i18n';

export interface SessionInfo {
  id: string; label: string; owner: string; ws_count: number; shared: boolean;
  age_secs: number; timeout_secs: number; scrollback: number; keepalive: boolean; alive: boolean;
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
}

/** Every shell running on the server, with a look at what is on each screen. */
export default function SessionsView({
  sessions, currentSessionId, onRefresh, onAttach, onPopOut, onShare,
}: Props) {
  const { t } = useT();
  const [renaming, setRenaming] = useState<string | null>(null);
  const [draft, setDraft] = useState('');

  const startRename = (session: SessionInfo) => {
    setRenaming(session.id);
    setDraft(session.title || '');
  };

  const saveRename = async (session: SessionInfo) => {
    await apiPost(`/api/terminal/sessions/${session.id}/name`, { name: draft });
    setRenaming(null);
    onRefresh();
  };

  const kill = async (session: SessionInfo) => {
    if (!window.confirm(t('terminal.endSessionConfirm', { name: session.title || session.label }))) return;
    await apiRequest(`/api/terminal/sessions/${session.id}`, { method: 'DELETE' });
    onRefresh();
  };

  const toggle = async (session: SessionInfo, what: 'keepalive' | 'share') => {
    const body = what === 'keepalive' ? { keepalive: !session.keepalive } : { shared: !session.shared };
    await apiPost(`/api/terminal/sessions/${session.id}/${what}`, body);
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

      <div className="session-grid">
        {sessions.map(session => (
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
                  {!session.alive && <span className="status-badge down">{t('share.ended')}</span>}
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
              {session.id !== currentSessionId && (
                <button className="btn btn-primary btn-sm" onClick={() => onAttach(session)}>{t('terminal.join')}</button>
              )}
              <button className="btn btn-secondary btn-sm" title={t('remote.detachHint')} onClick={() => onPopOut(session)}>⧉</button>
              {onShare && (
                <button className="btn btn-secondary btn-sm" title={t('share.hint')} onClick={() => onShare(session)}>🔗</button>
              )}
              <button className={`btn btn-sm ${session.keepalive ? 'btn-primary' : 'btn-secondary'}`}
                title={t('terminal.keepaliveHint')} onClick={() => toggle(session, 'keepalive')}>
                {t('terminal.keepaliveShort')} {session.keepalive ? 'ON' : 'OFF'}
              </button>
              <button className={`btn btn-sm ${session.shared ? 'btn-warning' : 'btn-secondary'}`}
                onClick={() => toggle(session, 'share')}>
                {session.shared ? t('terminal.shared') : t('terminal.share')}
              </button>
              <button className="btn btn-danger btn-sm" onClick={() => kill(session)}>✕</button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
