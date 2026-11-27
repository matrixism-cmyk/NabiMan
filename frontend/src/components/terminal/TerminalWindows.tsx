import React, {
  createContext, useCallback, useContext, useEffect, useMemo, useRef, useState,
} from 'react';
import { TerminalTarget } from './TerminalView';
import { toggleAllWindows } from './hideAll';
import { forgetTerminalStorage, pruneTerminalStorage, storedSessionId } from './terminalStorage';
import TerminalSettingsModal from './TerminalSettingsModal';
import TerminalWindowFrame from './TerminalWindowFrame';
import TerminalTaskbar from './TerminalTaskbar';
import { CONSOLE_H, CONSOLE_W, TermWindow, WindowPatch } from './windowGeometry';
import {
  cascadeWindows, clampWindows, consoleWindowOf, focusedWindowId, keepListedConsoles, LAYOUT_KEY, loadLayout, placeNew, saveLayout, tileWindows,
} from './windowStore';
import ConsoleWindowFrame from '../remote/ConsoleWindowFrame';
import { onConsolePoppedOut } from '../remote/consoleHandOver';
import { listConsoles, VmConsole } from '../remote/vncApi';
import { apiRequest } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';
import { noticeDialog } from '../../shared/ui/ConfirmDialog';
import './terminal-windows.css';

interface OpenOptions {
  title?: string;
  /** Attach to an existing session instead of starting a new one. */
  joinSessionId?: string;
}

interface Ctx {
  windows: TermWindow[];
  openTerminal: (target: TerminalTarget, options?: OpenOptions) => string;
  /** Opens a VM console as a window, or raises the window it already has. */
  openConsole: (c: VmConsole) => string;
  /** The consoles the account may open have loaded: windows of others go. */
  syncConsoles: (listed: VmConsole[] | null) => void;
  closeWindow: (id: string) => void;
  endSession: (id: string) => void;
  focusWindow: (id: string) => void;
  tile: () => void;
  cascade: () => void;
  openSettings: () => void;
}

const TerminalWindowsContext = createContext<Ctx | null>(null);

function describe(target: TerminalTarget): string {
  if (target.kind === 'local') return 'local';
  return `${target.user || 'root'}@${target.host || '?'}:${target.port || 22}`;
}


/**
 * Floating windows: terminals and VM consoles.
 *
 * A terminal window owns one tmux-backed session and keeps its own xterm
 * instance alive for as long as it is open (minimising only hides it), so
 * switching tabs, dragging or losing the network never clears a screen. A
 * console window shows a VM console whose connection is the console keeper's;
 * a console has at most one window. Positions are remembered across reloads
 * and each window re-attaches to its session.
 */
export function TerminalWindowsProvider({ children, owner, active = true }: { children: React.ReactNode; owner?: string; active?: boolean }) {
  const { t } = useT();
  const layoutKey = owner ? `${LAYOUT_KEY}:${owner}` : LAYOUT_KEY;
  const [windows, setWindows] = useState<TermWindow[]>(() => active ? loadLayout(layoutKey) : []);
  const [showSettings, setShowSettings] = useState(false);
  const topZ = useRef(windows.reduce((m, w) => Math.max(m, w.z), 10));

  useEffect(() => { if (active) saveLayout(windows, layoutKey); }, [windows, active, layoutKey]);

  // Clear screens left behind by windows that are gone (e.g. closed in another tab).
  useEffect(() => {
    if (active && !owner) pruneTerminalStorage(loadLayout(layoutKey).map((w) => w.id));
  }, [active, owner, layoutKey]);

  // Console windows that came back from storage wait for the list of consoles
  // the account may open: until it has loaded they do not connect, and only
  // then are the ones no longer listed dropped (never during the load).
  const [consolesKnown, setConsolesKnown] = useState(() => !windows.some((w) => w.kind === 'console'));
  const syncConsoles = useCallback((listed: VmConsole[] | null) => {
    setWindows((prev) => keepListedConsoles(prev, listed));
    setConsolesKnown(true);
  }, []);
  useEffect(() => {
    if (consolesKnown) return undefined;
    let live = true;
    // Refused (an account that may not open consoles): none stays. Unreachable:
    // the windows stay, and each says so itself with a way to reconnect.
    listConsoles().then((r) => { if (live) syncConsoles(r.success ? r.data || [] : null); })
      .catch(() => { if (live) setConsolesKnown(true); });
    return () => { live = false; };
  }, [consolesKnown, syncConsoles]);

  const update = useCallback((id: string, patch: WindowPatch) => {
    setWindows((prev) => prev.map((w) => (w.id === id ? { ...w, ...patch } : w)));
  }, []);

  const focusWindow = useCallback((id: string) => {
    setWindows((prev) => {
      const win = prev.find((w) => w.id === id);
      if (!win) return prev;
      const isTop = prev.every((w) => w.id === id || w.z < win.z);
      // Clicking inside an already focused window must not re-render the stack
      // (it would fight with text selection inside the terminal).
      if (isTop && !win.minimized) return prev;
      topZ.current += 1;
      const z = topZ.current;
      return prev.map((w) => (w.id === id ? { ...w, z, minimized: false } : w));
    });
  }, []);

  const openTerminal = useCallback((target: TerminalTarget, options?: OpenOptions): string => {
    if (!active) return '';
    if (options?.joinSessionId) {
      const existing = windows.find(w => storedSessionId(w.id) === options.joinSessionId);
      if (existing) { focusWindow(existing.id); return existing.id; }
    }
    const id = `tw_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 7)}`;
    topZ.current += 1;
    // The pane picks its session up from storage when it mounts.
    if (options?.joinSessionId) {
      try { localStorage.setItem(`nabiman_term_sid_${id}`, options.joinSessionId); } catch { /* storage blocked or full: only what this browser remembers is lost */ }
    }
    setWindows((prev) => [...prev, {
      id,
      kind: 'terminal',
      title: options?.title || (target.kind === 'local' ? 'Local Shell' : describe(target)),
      subtitle: describe(target),
      target,
      ...placeNew(prev.length, window.innerWidth, window.innerHeight),
      z: topZ.current,
      minimized: false,
      maximized: false,
    }]);
    return id;
  }, [windows, focusWindow, active]);

  const openConsole = useCallback((c: VmConsole): string => {
    if (!active) return '';
    // One connection, one place: a console that has a window is raised, not opened twice.
    const existing = consoleWindowOf(windows, c.name);
    if (existing) { focusWindow(existing.id); return existing.id; }
    const id = `cw_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 7)}`;
    topZ.current += 1;
    const protocol = c.protocol === 'rdp' ? 'rdp' : 'vnc';
    setWindows((prev) => (consoleWindowOf(prev, c.name) ? prev : [...prev, {
      id,
      kind: 'console',
      title: c.label,
      subtitle: protocol.toUpperCase(),
      console: { name: c.name, label: c.label, protocol, builtin: Boolean(c.builtin) },
      ...placeNew(prev.length, window.innerWidth, window.innerHeight, { w: CONSOLE_W, h: CONSOLE_H }),
      z: topZ.current,
      minimized: false,
      maximized: false,
    }]));
    return id;
  }, [windows, focusWindow, active]);

  // "새창으로 분리": the console went to a browser window, so its window here goes.
  useEffect(() => onConsolePoppedOut((name) => {
    setWindows((prev) => prev.filter((w) => !(w.kind === 'console' && w.console.name === name)));
  }), []);

  /**
   * Close the window but leave the session running server-side — it stays in
   * the session list and can be popped out again. The window's own saved
   * screen goes away with it, so storage does not grow without bound.
   */
  const closeWindow = useCallback((id: string) => {
    forgetTerminalStorage(id);
    setWindows((prev) => prev.filter((w) => w.id !== id));
  }, []);

  /** Close the window and kill its tmux session for good. */
  const endSession = useCallback(async (id: string) => {
    const sid = storedSessionId(id);
    if (sid) {
      // The window stays open when the session could not be ended, and says why.
      const res = await apiRequest(`/api/terminal/sessions/${encodeURIComponent(sid)}`, { method: 'DELETE' })
        .catch((e: unknown) => ({ success: false, message: e instanceof Error ? e.message : String(e) }));
      if (!res.success) { await noticeDialog(`${t('terminal.endFailed')}\n${res.message || t('common.error')}`); return; }
    }
    forgetTerminalStorage(id);
    setWindows((prev) => prev.filter((w) => w.id !== id));
  }, [t]);

  /** Lay every open window out in a grid that fills the screen. */
  const tile = useCallback(() => setWindows((prev) => tileWindows(prev, window.innerWidth, window.innerHeight)), []);

  /** The windows the bar's "모두 숨기기" took off the page, to bring the same ones back. */
  const hiddenByAll = useRef<string[]>([]);
  const toggleAll = useCallback(() => {
    setWindows((prev) => { const next = toggleAllWindows(prev, hiddenByAll.current); hiddenByAll.current = next.remembered; return next.windows; });
  }, []);

  const cascade = useCallback(() => setWindows((prev) => cascadeWindows(prev, window.innerWidth, window.innerHeight)), []);

  // Keep windows reachable when the browser window shrinks.
  useEffect(() => {
    const onResize = () => setWindows((prev) => clampWindows(prev, window.innerWidth, window.innerHeight));
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  const value = useMemo<Ctx>(() => ({
    windows, openTerminal, openConsole, syncConsoles, closeWindow, endSession, focusWindow, tile, cascade,
    openSettings: () => setShowSettings(true),
  }), [windows, openTerminal, openConsole, syncConsoles, closeWindow, endSession, focusWindow, tile, cascade]);
  const focused = focusedWindowId(windows);

  return (
    <TerminalWindowsContext.Provider value={value}>
      {children}
      {active && (
        <div className="term-window-layer">
          {windows.map((win) => (win.kind === 'console'
            ? <ConsoleWindowFrame key={win.id} win={win} focused={focused === win.id} ready={consolesKnown}
              onUpdate={update} onFocus={focusWindow} onClose={closeWindow} />
            : <TerminalWindowFrame key={win.id} win={win} onUpdate={update} onFocus={focusWindow} onClose={closeWindow} onEndSession={endSession} />
          ))}
          <TerminalTaskbar windows={windows} onFocus={focusWindow} onTile={tile} onCascade={cascade}
            onOpenSettings={() => setShowSettings(true)} onOpen={openTerminal} onOpenConsole={openConsole} onToggleAll={toggleAll} />
        </div>
      )}
      {active && showSettings && <TerminalSettingsModal onClose={() => setShowSettings(false)} />}
    </TerminalWindowsContext.Provider>
  );
}

export function useTerminalWindows(): Ctx {
  const ctx = useContext(TerminalWindowsContext);
  if (!ctx) {
    throw new Error('useTerminalWindows must be used inside <TerminalWindowsProvider>');
  }
  return ctx;
}
