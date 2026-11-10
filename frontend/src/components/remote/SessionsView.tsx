import React, { useRef, useState } from 'react';
import { apiPost, apiRequest } from '../../shared/api/useApi';
import ClaudeAddress, { ClaudePeer } from '../terminal/ClaudeAddress';
import SessionLinks, { SessionLink } from './SessionLinks';
import { arrange, moved, ORDER_MODES, OrderMode, useSessionOrder } from './sessionOrder';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import SessionGraphView from './graph/SessionGraphView';
import { ErrorBoundary } from '../../shared/ui/ErrorBoundary';
import { readView, saveView, VIEW_MODES, ViewMode } from './graph/graphModel';
import AgentIcons from './graph/AgentIcons';
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
  /** The AI agents running in it, by kind (`claude`, `codex`, `gemini`…). */
  agents?: string[];
  /** Sessions its Claude has lately handed work to, by id. */
  directs?: string[];
  /** Its Claude hands out work here and takes it from none. */
  orchestrator?: boolean;
  /** The session the operator made the lead of all Claude work (총괄). */
  lead?: boolean;
  /** For each of `directs`: the first line of the work last sent there, and when. */
  notes?: { to: string; text: string; at: number }[];
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

/** A finished hand-off's arrow fades from FADED_START to FAINTEST over FADE_SECS. */
const FADE_SECS = 2 * 60 * 60;
const FADED_START = 0.85;
const FAINTEST = 0.35;

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
  /** Every session, when `sessions` is one target's: badges count work handed
   *  to or from sessions not on this list (arrows join only those shown). */
  context?: SessionInfo[];
}

/** Every shell running on the server, with a look at what is on each screen. */
export default function SessionsView({
  sessions, currentSessionId, onRefresh, onAttach, onPopOut, onShare, title, actions, empty, context,
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
  const [view, setView] = useState<ViewMode>(readView);
  // The account's own order, or a sort; dragging a tile by its handle sets the order.
  const { order, save: saveOrder, error: orderError } = useSessionOrder();
  const shownInOrder = arrange(sessions, order);
  const shownIds = shownInOrder.map((s) => s.id);
  const [drag, setDrag] = useState<{ id: string; over: string | null; after: boolean } | null>(null);
  const place = (id: string, before: string | null) => {
    saveOrder({ mode: 'custom', ids: moved(order.mode === 'custom' ? order.ids : [], shownIds, id, before) });
  };
  const dragOver = (e: React.PointerEvent) => {
    if (!drag) return;
    const card = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('[data-session-id]');
    const over = card?.dataset.sessionId;
    if (!card || !over || over === drag.id) { if (drag.over) setDrag({ ...drag, over: null }); return; }
    const r = card.getBoundingClientRect();
    const after = e.clientX > r.left + r.width / 2;
    if (over !== drag.over || after !== drag.after) setDrag({ ...drag, over, after });
  };
  const drop = () => {
    if (drag?.over) {
      const rest = shownIds.filter((x) => x !== drag.id);
      const i = rest.indexOf(drag.over);
      place(drag.id, drag.after ? rest[i + 1] ?? null : drag.over);
    }
    setDrag(null);
  };
  // The handle moves its tile with the arrow keys too.
  const nudge = (e: React.KeyboardEvent, id: string) => {
    const i = shownIds.indexOf(id);
    if (e.key === 'Escape' && drag) { setDrag(null); return; }
    if ((e.key === 'ArrowLeft' || e.key === 'ArrowUp') && i > 0) { e.preventDefault(); place(id, shownIds[i - 1]); }
    if ((e.key === 'ArrowRight' || e.key === 'ArrowDown') && i < shownIds.length - 1) { e.preventDefault(); place(id, shownIds[i + 2] ?? null); }
  };
  // A state, not a ref: the arrows redraw once the grid is there.
  const [grid, setGrid] = useState<HTMLDivElement | null>(null);
  // Only NabiMan's sessions are tiles: a Claude no tile holds (a background
  // agent, one started on a server itself) is not shown, nor arrows to it.
  type Node = Pick<SessionInfo, 'id' | 'directs' | 'orchestrator' | 'busy' | 'claude' | 'notes'> & { title?: string; label?: string };
  const shown = new Set(sessions.map((s) => s.id));
  // Every session, for the badges: a target's list still says who leads whom elsewhere.
  const everyone: Node[] = Array.from(new Map([...(context || []), ...sessions].map((n) => [n.id, n] as const)).values());
  const busy = new Set(everyone.filter((s) => s.busy).map((s) => s.id));
  // Once both ends are waiting, the work handed over is done for now: the arrow fades.
  const nameOf = (s: Node) => s.claude?.[0]?.name || s.title || s.label || '';
  const nodeOf = new Map(everyone.map((n) => [n.id, n]));
  // Hovering an arrow says what was handed over: the last request's first line.
  const nowSecs = Math.floor(Date.now() / 1000);
  const noteFor = (s: Node, to: string) => {
    const target = nodeOf.get(to);
    const toName = target ? nameOf(target) : to;
    const lines = [`${nameOf(s)} → ${toName}`];
    const note = s.notes?.find((n) => n.to === to);
    if (note?.text) lines.push(t('claudePeer.linkLast', { ago: formatAge(Math.max(0, nowSecs - note.at)), text: note.text }));
    lines.push(t(busy.has(to) ? 'claudePeer.linkBusy' : 'claudePeer.linkIdle', { name: toName }));
    return lines.join('\n');
  };
  // Strong only while the session it went to works on it; after that it
  // fades over FADE_SECS from when that session stopped (or the last request).
  const idleSince = (n: Node | undefined, to: string, from: Node) => {
    const own = Math.max(0, ...(n?.claude || []).map((p) => p.since || 0));
    return own || from.notes?.find((x) => x.to === to)?.at || 0;
  };
  const strengthOf = (from: Node, to: string) => {
    if (busy.has(to)) return 1;
    const since = idleSince(nodeOf.get(to), to, from);
    if (!since) return FAINTEST;
    const left = Math.max(0, 1 - (nowSecs - since) / FADE_SECS);
    return FAINTEST + (FADED_START - FAINTEST) * left;
  };
  const related: SessionLink[] = everyone.flatMap((s) => (s.directs || [])
    .filter((to) => nodeOf.has(to))
    .map((to) => ({ from: s.id, to, orchestrator: !!s.orchestrator, live: busy.has(to), strength: strengthOf(s, to), note: noteFor(s, to) })));
  // Arrows only between cards on this list.
  const links = related.filter((l) => shown.has(l.from) && shown.has(l.to));
  const leader = new Map<string, Node>();
  for (const l of related) {
    const from = nodeOf.get(l.from);
    if (from && (!leader.has(l.to) || from.orchestrator)) leader.set(l.to, from);
  }

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
          <div className="btn-group session-view-switch" role="group" aria-label={t('graph.view')}>
            {VIEW_MODES.map((m) => (
              <button key={m} type="button" className={`btn btn-sm ${view === m ? 'btn-primary' : 'btn-secondary'}`} aria-pressed={view === m}
                onClick={() => { setView(m); saveView(m); }}>{t(`graph.view.${m}`)}</button>
            ))}
          </div>
          <select className="session-sort" aria-label={t('sessionOrder.label')} title={t('sessionOrder.hint')} value={order.mode}
            onChange={(e) => saveOrder({ ...order, mode: e.target.value as OrderMode })}>
            {ORDER_MODES.map((m) => <option key={m} value={m}>{t(`sessionOrder.${m}`)}</option>)}
          </select>
          <button className="btn btn-secondary btn-sm" onClick={onRefresh}>{t('common.refresh')}</button>
        </div>
      </div>

      {message && <div role="alert" className="message">{message}</div>}
      {orderError && <div role="alert" className="message">{t('sessionOrder.saveFailed')}</div>}
      {view !== 'tiles' && (
        <ErrorBoundary region={`graph:${view}`} resetKey={view}>
          <SessionGraphView mode={view} sessions={shownInOrder} links={links} onAttach={onAttach} onPopOut={onPopOut} />
        </ErrorBoundary>
      )}
      <div className={`session-grid ${links.length ? 'has-links' : ''}`} ref={setGrid} hidden={view !== 'tiles'}>
        <SessionLinks grid={grid} links={links} layout={shownIds.join(',')} />
        {shownInOrder.map(session => {
          const state = session.state === undefined ? (session.alive ? 'alive' : 'ended') :
            ['alive', 'ended', 'unknown', 'blocked'].includes(session.state) ? session.state : 'unknown';
          const attachable = session.alive && state === 'alive';
          // The orchestrator blinks red; Claude at work blinks green; any
          // other screen that just changed glows.
          const directs = related.filter((l) => l.from === session.id).length;
          const orchestrating = attachable && !!session.orchestrator && directs > 0;
          // It blinks while it or any session it handed work to is working.
          const resting = orchestrating && !related.some((l) => l.from === session.id && l.live);
          const working = attachable && !orchestrating && !!session.busy;
          const output = attachable && !orchestrating && !working && recentOutput(session.id);
          const ledBy = attachable ? leader.get(session.id) : undefined;
          return (
          <div key={session.id} data-session-id={session.id}
            className={`session-card ${session.id === currentSessionId ? 'is-current' : ''} ${orchestrating ? 'is-orchestrator' : ''} ${resting ? 'is-resting' : ''} ${working ? 'is-working' : ''} ${output ? 'is-output' : ''} ${drag?.id === session.id ? 'is-dragging' : ''} ${drag?.over === session.id ? (drag.after ? 'drop-after' : 'drop-before') : ''}`}>
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
                  <button type="button" className="session-drag-handle" aria-label={t('sessionOrder.handle', { name: session.title || session.label })}
                    title={t('sessionOrder.handleHint')}
                    onPointerDown={(e) => {
                      if (e.button > 0) return;
                      e.preventDefault();
                      e.currentTarget.setPointerCapture?.(e.pointerId);
                      setDrag({ id: session.id, over: null, after: false });
                    }}
                    onPointerMove={dragOver} onPointerUp={drop} onPointerCancel={() => setDrag(null)}
                    onKeyDown={(e) => nudge(e, session.id)}>⠿</button>
                  <strong className="session-name">{session.title || session.label}</strong>
                  <button className="session-rename-btn" title={t('terminal.rename')} onClick={() => startRename(session)}>✎</button>
                  {state !== 'alive' && <span className="status-badge down">{t(state === 'unknown' ? 'terminal.sessionUnknown' : state === 'blocked' ? 'terminal.sessionBlocked' : 'share.ended')}</span>}
                  {session.id === currentSessionId && <span className="status-badge up">{t('terminal.thisPane')}</span>}
                  {orchestrating && (
                    <span className={`session-work-badge is-orchestrator ${session.busy ? '' : 'is-still'}`} title={t('claudePeer.orchestratorHint', { count: directs })}>
                      {t(session.lead ? 'claudePeer.lead' : 'claudePeer.orchestrator', { count: directs })}
                    </span>
                  )}
                  {ledBy && (
                    <span className="session-work-badge is-led" title={t('claudePeer.ledHint', { name: nameOf(ledBy) })}>
                      {t('claudePeer.led', { name: nameOf(ledBy) })}
                    </span>
                  )}
                  <AgentIcons kinds={session.agents} claude={session.claude?.[0]} />
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
