import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { apiRequest } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';
import { hasFiles, hasTerminal, sessionLabel } from '../remote/protocols';
import { RemoteServer } from '../remote/remoteTypes';
import type { TerminalTarget } from './TerminalView';
import { storedSessionId } from './terminalStorage';
import './start-menu.css';

/** What the menu needs of a session (see SessionInfo in the remote workspace). */
interface Session { id: string; label: string; title: string; alive: boolean; busy?: boolean; agents?: string[]; preview?: string }
interface Win { id: string }

interface Props {
  windows: Win[];
  /** Opens a terminal window; with `joinSessionId` it shows a running session (or raises its window). */
  open: (target: TerminalTarget, options?: { title?: string; joinSessionId?: string }) => void;
}

/** What an unnamed session is showing, to tell it from its neighbours: the last line on its screen. */
const lastLine = (preview = '') => preview.split('\n').map(line => line.trim()).filter(Boolean).pop()?.slice(-80) || '';

const targetOf = (server: RemoteServer): TerminalTarget =>
  ({ kind: 'ssh', host: server.host, port: server.port, user: server.user, serverId: server.id, noFiles: !hasFiles(server) });

/**
 * The start button of the terminal bar: every running session and every
 * server, one click from a window of its own — without going back to the
 * remote page, which a maximised window covers.
 */
export default function StartMenu({ windows, open }: Props) {
  const { t } = useT();
  const [shown, setShown] = useState(false);
  const [query, setQuery] = useState('');
  const [sessions, setSessions] = useState<Session[] | null>(null);
  const [servers, setServers] = useState<RemoteServer[]>([]);
  const root = useRef<HTMLDivElement>(null);
  const menu = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!shown) return undefined;
    let live = true;
    const load = () => {
      apiRequest<Session[]>('/api/terminal/sessions').then(r => { if (live) setSessions(r.success && r.data ? r.data.filter(s => s.alive) : []); })
        .catch(() => { if (live) setSessions([]); });
      apiRequest<RemoteServer[]>('/api/remote-servers').then(r => { if (live && r.success && r.data) setServers(r.data); })
        .catch(() => { /* the sessions still list, under their labels */ });
    };
    load();
    const timer = window.setInterval(load, 10000);
    const inside = (node: Node) => Boolean(root.current?.contains(node) || menu.current?.contains(node));
    const away = (e: MouseEvent) => { if (!inside(e.target as Node)) setShown(false); };
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') setShown(false); };
    document.addEventListener('mousedown', away);
    document.addEventListener('keydown', key);
    return () => { live = false; window.clearInterval(timer); document.removeEventListener('mousedown', away); document.removeEventListener('keydown', key); };
  }, [shown]);

  const byLabel = useMemo(() => new Map(servers.map(s => [sessionLabel(s), s])), [servers]);
  const windowed = new Set(windows.map(w => storedSessionId(w.id)));
  const words = query.trim().toLowerCase();
  const has = (...texts: string[]) => !words || texts.some(text => text.toLowerCase().includes(words));
  const nameOf = (s: Session) => byLabel.get(s.label)?.name || (s.label === 'local' ? t('remote.localShell') : s.label);
  const running = (sessions || []).filter(s => has(s.title, nameOf(s), s.label));
  const places = servers.filter(s => hasTerminal(s) && has(s.name, s.host, s.user));

  const join = (s: Session) => {
    const server = byLabel.get(s.label);
    open(server ? targetOf(server) : { kind: s.label === 'local' ? 'local' : 'ssh' }, { title: server?.name || nameOf(s), joinSessionId: s.id });
    setShown(false);
  };
  const fresh = (target: TerminalTarget, title: string) => { open(target, { title }); setShown(false); };

  return (
    <div className="term-start" ref={root}>
      <button type="button" className={`term-start-btn ${shown ? 'is-open' : ''}`} aria-haspopup="menu" aria-expanded={shown}
        title={t('start.title')} onClick={() => { setShown(v => !v); setQuery(''); }}>
        <img src={`${process.env.PUBLIC_URL}/favicon.svg`} alt="" width="18" height="18" />
        <span>{t('tab.terminal')} {windows.length}</span>
      </button>
      {/* The bar scrolls sideways and blurs what is behind it, either of which would clip a menu inside it: the menu goes on the page itself. */}
      {shown && createPortal(
        <div className="term-start-menu" role="menu" aria-label={t('start.title')} ref={menu}>
          <input className="term-start-search" autoFocus value={query} placeholder={t('start.search')}
            aria-label={t('start.search')} onChange={e => setQuery(e.target.value)} />
          <div className="term-start-lists">
            <section>
              <h4>{t('start.running')} {sessions ? running.length : ''}</h4>
              {!sessions ? <p className="term-start-note">{t('common.loading')}</p>
                : running.length === 0 ? <p className="term-start-note">{t('start.none')}</p>
                : running.map(s => (
                  <button type="button" role="menuitem" key={s.id} className="term-start-item" onClick={() => join(s)} title={s.label}>
                    <span className={`term-start-dot ${s.busy ? 'is-busy' : ''}`} />
                    <span className="term-start-name">{s.title || nameOf(s)}</span>
                    <span className="term-start-sub">{s.title ? nameOf(s) : lastLine(s.preview)}</span>
                    {(s.agents || []).length > 0 && <span className="term-start-tag">{(s.agents || []).join(' · ')}</span>}
                    {windowed.has(s.id) && <span className="term-start-tag is-open">{t('start.windowed')}</span>}
                  </button>
                ))}
            </section>
            <section>
              <h4>{t('start.fresh')}</h4>
              {has(t('remote.localShell'), 'local') && (
                <button type="button" role="menuitem" className="term-start-item" onClick={() => fresh({ kind: 'local' }, t('remote.localShell'))}>
                  <span className="term-start-plus">＋</span><span className="term-start-name">{t('remote.localShell')}</span>
                </button>
              )}
              {places.map(s => (
                <button type="button" role="menuitem" key={s.id} className="term-start-item" onClick={() => fresh(targetOf(s), s.name)} title={`${s.user}@${s.host}:${s.port}`}>
                  <span className="term-start-plus">＋</span><span className="term-start-name">{s.name}</span>
                  <span className="term-start-sub">{s.user}@{s.host}</span>
                </button>
              ))}
            </section>
          </div>
        </div>,
        document.body,
      )}
    </div>
  );
}
