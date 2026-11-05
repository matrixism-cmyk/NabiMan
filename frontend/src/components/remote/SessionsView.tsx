import React, { useRef, useState } from 'react';
import { apiPost, apiRequest } from '../../shared/api/useApi';
import ClaudeAddress, { ClaudePeer } from '../terminal/ClaudeAddress';
import SessionLinks, { SessionLink } from './SessionLinks';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import { useGrants, useMember, useSharing } from './member';

export interface SessionInfo {
  id: string; label: string; owner: string; ws_count: number; shared: boolean;
  age_secs: number; timeout_secs: number; scrollback: number; keepalive: boolean; alive: boolean;
  state?: 'alive' | 'ended' | 'unknown' | 'blocked';
  /** A name the operator gave it; empty means it goes by its label. */
  title: string;
  /** The last lines on screen, for the thumbnail. */
  preview: string;
  /** Claude Code sessions running in it. */
  claude?: ClaudePeer[];
  /** A Claude session in it is working on a turn. */
  busy?: boolean;
  /** Sessions its Claude has lately handed work to, by id. */
  directs?: string[];
  /** Its Claude hands out work here and takes it from none. */
  orchestrator?: boolean;
}

/** How long a changed screen counts as output just now: a little over one refresh. */
const OUTPUT_MS = 11000;

/**
 * Which sessions' screens changed lately, seen by comparing each refresh's
 * preview with the one before: a shell at work, a build, a log being followed.
 */
function useScreenChanges(sessions: SessionInfo[]): (id: string) => boolean {
  const seen = useRef(new Map<string, { preview: string; changed: number }>());
  const now = Date.now();
  const next = new Map<string, { preview: string; changed: number }>();
  for (const s of sessions) {
    const before = seen.current.get(s.id);
    const changed = before && before.preview !== s.preview && s.preview ? now : (before?.changed ?? 0);
    next.set(s.id, { preview: s.preview, changed });
  }
  seen.current = next;
  return (id) => now - (next.get(id)?.changed ?? 0) < OUTPUT_MS;
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
  const member = useMember();
  const sharing = useSharing();
  // "세션 유지" only when an administrator allows it and set no idle limit.
  const grants = useGrants();
  const canKeep = !grants || (grants.keep_sessions && !grants.max_idle_minutes);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [draft, setDraft] = useState('');
  const [message, setMessage] = useState('');
  const recentOutput = useScreenChanges(sessions);
  // A state, not a ref: the arrows redraw once the grid is there.
  const [grid, setGrid] = useState<HTMLDivElement | null>(null);
  const shown = new Set(sessions.map((s) => s.id));
  const links: SessionLink[] = sessions.flatMap((s) => (s.directs || [])
    .filter((to) => shown.has(to))
    .map((to) => ({ from: s.id, to, orchestrator: !!s.orchestrator })));
  const leader = new Map<string, SessionInfo>();
  for (const l of links) {
    const from = sessions.find((s) => s.id === l.from);
    if (from && (!leader.has(l.to) || from.orchestrator)) leader.set(l.to, from);
  }
  const nameOf = (s: SessionInfo) => s.claude?.[0]?.name || s.title || s.label;

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
    if (!(await confirmDialog({ message: t('terminal.endSessionConfirm', { name: session.title || session.label }), danger: true }))) return;
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
      <div className="session-grid" ref={setGrid}>
        <SessionLinks grid={grid} links={links} />
        {sessions.map(session => {
          const state = session.state === undefined ? (session.alive ? 'alive' : 'ended') :
            ['alive', 'ended', 'unknown', 'blocked'].includes(session.state) ? session.state : 'unknown';
          const attachable = session.alive && state === 'alive';
          // The orchestrator blinks red; Claude at work blinks green; any
          // other screen that just changed glows.
          const directs = links.filter((l) => l.from === session.id).length;
          const orchestrating = attachable && !!session.orchestrator && directs > 0;
          const working = attachable && !orchestrating && !!session.busy;
          const output = attachable && !orchestrating && !working && recentOutput(session.id);
          const ledBy = attachable ? leader.get(session.id) : undefined;
          return (
          <div key={session.id} data-session-id={session.id}
            className={`session-card ${session.id === currentSessionId ? 'is-current' : ''} ${orchestrating ? 'is-orchestrator' : ''} ${working ? 'is-working' : ''} ${output ? 'is-output' : ''}`}>
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
                  {orchestrating && (
                    <span className={`session-work-badge is-orchestrator ${session.busy ? '' : 'is-still'}`} title={t('claudePeer.orchestratorHint')}>
                      {t('claudePeer.orchestrator', { count: directs })}
                    </span>
                  )}
                  {ledBy && (
                    <span className="session-work-badge is-led" title={t('claudePeer.ledHint', { name: nameOf(ledBy) })}>
                      {t('claudePeer.led', { name: nameOf(ledBy) })}
                    </span>
                  )}
                  {working && <span className="session-work-badge" title={t('claudePeer.workingHint')}>{t('claudePeer.working')}</span>}
                  {output && <span className="session-work-badge is-output" title={t('claudePeer.outputHint')}>{t('claudePeer.output')}</span>}
                </>
              )}
            </div>

            <code className="session-label">{session.label}</code>
            {attachable && <ClaudeAddress variant="card" peers={session.claude || []} />}

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
              <button className="btn btn-secondary btn-sm rw-wide-only" title={t('remote.detachHint')} disabled={!attachable} onClick={() => onPopOut(session)}>⧉</button>
              {onShare && sharing.sessions && (
                <button className="btn btn-secondary btn-sm" title={t('share.hint')} disabled={!attachable} onClick={() => onShare(session)}>🔗</button>
              )}
              <button className={`btn btn-sm ${session.keepalive ? 'btn-primary' : 'btn-secondary'}`}
                disabled={!canKeep && !session.keepalive}
                title={t(canKeep ? 'terminal.keepaliveHint' : 'terminal.keepaliveNotAllowed')} onClick={() => toggle(session, 'keepalive')}>
                {t('terminal.keepaliveShort')} {session.keepalive ? 'ON' : 'OFF'}
              </button>
              {!member && (
                <button className={`btn btn-sm ${session.shared ? 'btn-warning' : 'btn-secondary'}`}
                  disabled={!attachable && !session.shared} onClick={() => toggle(session, 'share')}>
                  {session.shared ? t('terminal.shared') : t('terminal.share')}
                </button>
              )}
              <button className="btn btn-danger btn-sm" onClick={() => kill(session)}>{t('terminal.endSession')}</button>
            </div>
          </div>
        );})}
      </div>
    </div>
  );
}
