import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { apiRequest } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';
import { useConsoleSnapshots } from '../remote/consoleSnapshots';
import { addressOf, hasFiles, hasTerminal, sessionLabel } from '../remote/protocols';
import { RemoteServer } from '../remote/remoteTypes';
import { popOutConsole } from '../remote/consoleHandOver';
import { listConsoles, VmConsole } from '../remote/vncApi';
import { openInWorkspace } from '../remote/workspaceState';
import { ConsoleCard, SessionCard, StartSession } from './StartMenuParts';
import { moveFocus } from './startMenuKeys';
import type { TerminalTarget } from './TerminalView';
import { storedSessionId } from './terminalStorage';
import './start-menu.css';

interface Win { id: string; kind?: string; console?: { name: string } }

interface Props {
  windows: Win[];
  /** Opens a terminal window; with `joinSessionId` it shows a running session (or raises its window). */
  open: (target: TerminalTarget, options?: { title?: string; joinSessionId?: string }) => void;
  /** Opens a VM console as a window on this page (or raises the one it has). */
  openConsole: (c: VmConsole) => void;
}

const targetOf = (server: RemoteServer): TerminalTarget =>
  ({ kind: 'ssh', host: server.host, port: server.port, user: server.user, serverId: server.id, noFiles: !hasFiles(server) });

const LAMPS: Record<string, string> = { online: 'is-up', offline: 'is-down', partial: 'is-partial' };

/**
 * The start button of the terminal bar: every running session, every VM
 * console and every server, one click away — without going back to the remote
 * page, which a maximised window covers. Sessions show the last lines on their
 * screen and consoles the screen this tab already has open, to tell them apart.
 */
export default function StartMenu({ windows, open, openConsole }: Props) {
  const { t } = useT();
  const [shown, setShown] = useState(false);
  const [query, setQuery] = useState('');
  const [sessions, setSessions] = useState<StartSession[] | null>(null);
  const [servers, setServers] = useState<RemoteServer[]>([]);
  const [consoles, setConsoles] = useState<VmConsole[]>([]);
  const root = useRef<HTMLDivElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const shots = useConsoleSnapshots();

  useEffect(() => {
    if (!shown) return undefined;
    let live = true;
    const load = () => {
      apiRequest<StartSession[]>('/api/terminal/sessions').then(r => { if (live) setSessions(r.success && r.data ? r.data.filter(s => s.alive) : []); })
        .catch(() => { if (live) setSessions([]); });
      apiRequest<RemoteServer[]>('/api/remote-servers').then(r => { if (live && r.success && r.data) setServers(r.data); })
        .catch(() => { /* the sessions still list, under their labels */ });
      // An account that may not open consoles is refused the list: then there is no such section.
      listConsoles().then(r => (r.success && r.data ? r.data : [])).catch(() => [] as VmConsole[]).then(list => {
        if (!live) return;
        setConsoles(list);
        const names = list.map(c => c.name);
        shots.retain(names);
        // Only of consoles already connected in this tab: a picture is never a reason to connect.
        shots.refresh(names);
      });
    };
    load();
    const timer = window.setInterval(load, 10000);
    const inside = (node: Node) => Boolean(root.current?.contains(node) || menu.current?.contains(node));
    const away = (e: MouseEvent) => { if (!inside(e.target as Node)) setShown(false); };
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') setShown(false); };
    document.addEventListener('mousedown', away);
    document.addEventListener('keydown', key);
    return () => { live = false; window.clearInterval(timer); document.removeEventListener('mousedown', away); document.removeEventListener('keydown', key); };
  }, [shown, shots]);

  const byLabel = useMemo(() => new Map(servers.map(s => [sessionLabel(s), s])), [servers]);
  const windowed = new Set(windows.map(w => storedSessionId(w.id)));
  const words = query.trim().toLowerCase();
  const has = (...texts: (string | undefined)[]) => !words || texts.some(text => text?.toLowerCase().includes(words));
  const nameOf = (s: StartSession) => byLabel.get(s.label)?.name || (s.label === 'local' ? t('remote.localShell') : s.label);
  const running = (sessions || []).filter(s => has(s.title, nameOf(s), s.label, s.preview, ...(s.agents || [])));
  const vms = consoles.filter(c => has(c.label, c.name, c.protocol || 'vnc', c.host));
  const places = servers.filter(s => hasTerminal(s) && has(s.name, s.host, s.user));
  const stores = servers.filter(s => !hasTerminal(s) && hasFiles(s) && has(s.name, s.host, s.user));
  const local = has(t('remote.localShell'), 'local');

  const join = (s: StartSession) => {
    const server = byLabel.get(s.label);
    open(server ? targetOf(server) : { kind: s.label === 'local' ? 'local' : 'ssh' }, { title: server?.name || nameOf(s), joinSessionId: s.id });
    setShown(false);
  };
  const fresh = (target: TerminalTarget, title: string) => { open(target, { title }); setShown(false); };
  // As the rail's "분리": a window on this page, the same one when asked again;
  // and its "새창으로 분리": a browser window of its own, which takes the connection over.
  const windowedConsoles = new Set(windows.map(w => (w.kind === 'console' ? w.console?.name : undefined)));
  const showConsole = (c: VmConsole) => { openConsole(c); setShown(false); };
  const popOut = (c: VmConsole) => { popOutConsole(c); setShown(false); };
  const openFiles = (s: RemoteServer) => { openInWorkspace({ kind: 'server', id: s.id }); setShown(false); };

  return (
    <div className="term-start" ref={root}>
      <button type="button" className={`term-start-btn ${shown ? 'is-open' : ''}`} aria-haspopup="menu" aria-expanded={shown}
        title={t('start.title')} onClick={() => { setShown(v => !v); setQuery(''); }}>
        <img src={`${process.env.PUBLIC_URL}/favicon.svg`} alt="" width="18" height="18" />
        <span>{t('terminal.windows')} {windows.length}</span>
      </button>
      {/* The bar scrolls sideways and blurs what is behind it, either of which would clip a menu inside it: the menu goes on the page itself. */}
      {shown && createPortal(
        <div className={`term-start-menu ${consoles.length ? '' : 'no-consoles'}`} role="menu" aria-label={t('start.title')} ref={menu}
          onKeyDown={e => { if (menu.current) moveFocus(e, menu.current); }}>
          <input className="term-start-search" autoFocus value={query} placeholder={t('start.search')}
            aria-label={t('start.search')} onChange={e => setQuery(e.target.value)} />
          <div className="term-start-areas">
            <section className="term-start-running" role="group" aria-label={t('start.running')}>
              <h4>{t('start.running')} {sessions ? running.length : ''}</h4>
              {!sessions ? <p className="term-start-note">{t('common.loading')}</p>
                : running.length === 0 ? <p className="term-start-note">{t('start.none')}</p>
                : (
                  <div className="term-start-grid">
                    {running.map(s => (
                      <SessionCard key={s.id} session={s} name={s.title || nameOf(s)} windowed={windowed.has(s.id)} onOpen={() => join(s)}
                        sub={s.title ? nameOf(s) : byLabel.has(s.label) ? s.label.replace(/^[a-z]+:/, '') : ''} />
                    ))}
                  </div>
                )}
            </section>
            {consoles.length > 0 && (
              <section className="term-start-consoles" role="group" aria-label={t('start.consoles')}>
                <h4>{t('start.consoles')} {vms.length}</h4>
                <div className="term-start-grid">
                  {vms.map(c => (
                    <ConsoleCard key={c.name} console={c} shot={shots.shot(c.name)} fresh={shots.fresh(c.name)} held={shots.held(c.name)}
                      windowed={windowedConsoles.has(c.name)} onOpen={() => showConsole(c)} onPopOut={() => popOut(c)} />
                  ))}
                </div>
              </section>
            )}
            <section className="term-start-new" role="group" aria-label={t('start.fresh')}>
              <h4>{t('start.fresh')}</h4>
              {local && (
                <button type="button" role="menuitem" className="term-start-item" onClick={() => fresh({ kind: 'local' }, t('remote.localShell'))}>
                  <span className="term-start-plus">＋</span><span className="term-start-name">{t('remote.localShell')}</span>
                </button>
              )}
              {places.map(s => (
                <button type="button" role="menuitem" key={s.id} className="term-start-item" onClick={() => fresh(targetOf(s), s.name)} title={`${s.user}@${s.host}:${s.port}`}>
                  <span className="term-start-plus">＋</span><span className="term-start-name">{s.name}</span>
                  <span className="term-start-sub">{s.user}@{s.host}</span>
                  {LAMPS[s.status] && <span className={`term-start-lamp ${LAMPS[s.status]}`} aria-hidden="true" />}
                </button>
              ))}
              {stores.length > 0 && <h4>{t('start.files')}</h4>}
              {stores.map(s => (
                <button type="button" role="menuitem" key={s.id} className="term-start-item" onClick={() => openFiles(s)} title={t('start.filesHint', { name: s.name })}>
                  <span className="term-start-plus">▤</span><span className="term-start-name">{s.name}</span>
                  <span className="term-start-sub">{addressOf(s)}</span>
                  {LAMPS[s.status] && <span className={`term-start-lamp ${LAMPS[s.status]}`} aria-hidden="true" />}
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
