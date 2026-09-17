import React, {
  createContext, useCallback, useContext, useEffect, useMemo, useRef, useState,
} from 'react';
import TerminalView, {
  TerminalStatus, TerminalTarget, TerminalViewHandle, forgetTerminalStorage,
  pruneTerminalStorage, storedSessionId,
} from './TerminalView';
import { useTerminalSettings } from './settings';
import TerminalSettingsModal from './TerminalSettingsModal';
import { apiRequest } from '../../hooks/useApi';
import { useT } from '../../i18n';
import './terminal-windows.css';

export interface TermWindow {
  id: string;
  title: string;
  subtitle: string;
  target: TerminalTarget;
  x: number;
  y: number;
  w: number;
  h: number;
  z: number;
  minimized: boolean;
  maximized: boolean;
  /** Geometry to return to when un-maximizing. */
  restore?: { x: number; y: number; w: number; h: number };
}

interface OpenOptions {
  title?: string;
  /** Attach to an existing session instead of starting a new one. */
  joinSessionId?: string;
}

interface Ctx {
  windows: TermWindow[];
  openTerminal: (target: TerminalTarget, options?: OpenOptions) => string;
  closeWindow: (id: string) => void;
  endSession: (id: string) => void;
  focusWindow: (id: string) => void;
  tile: () => void;
  cascade: () => void;
  openSettings: () => void;
}

const TerminalWindowsContext = createContext<Ctx | null>(null);

const LAYOUT_KEY = 'nabiman_term_windows';
const DEFAULT_W = 720;
const DEFAULT_H = 440;
const MIN_W = 320;
const MIN_H = 200;
const TASKBAR_H = 44;

function loadLayout(): TermWindow[] {
  try {
    const raw = localStorage.getItem(LAYOUT_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((w: TermWindow) => w && w.id && w.target);
  } catch {
    return [];
  }
}

function saveLayout(windows: TermWindow[]) {
  try { localStorage.setItem(LAYOUT_KEY, JSON.stringify(windows)); } catch { /* ignore */ }
}

function describe(target: TerminalTarget): string {
  if (target.kind === 'local') return 'local';
  return `${target.user || 'root'}@${target.host || '?'}:${target.port || 22}`;
}

const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);

/**
 * Floating terminal windows.
 *
 * Every window owns one tmux-backed session and keeps its own xterm instance
 * alive for as long as it is open (minimising only hides it), so switching
 * tabs, dragging or losing the network never clears a screen. Positions are
 * remembered across reloads and each window re-attaches to its session.
 */
export function TerminalWindowsProvider({ children }: { children: React.ReactNode }) {
  const [windows, setWindows] = useState<TermWindow[]>(loadLayout);
  const [showSettings, setShowSettings] = useState(false);
  const topZ = useRef(windows.reduce((m, w) => Math.max(m, w.z), 10));

  useEffect(() => { saveLayout(windows); }, [windows]);

  // Clear screens left behind by windows that are gone (e.g. closed in another tab).
  useEffect(() => {
    pruneTerminalStorage(loadLayout().map((w) => w.id));
  }, []);

  const update = useCallback((id: string, patch: Partial<TermWindow>) => {
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
    const id = `tw_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 7)}`;
    topZ.current += 1;
    // The pane picks its session up from storage when it mounts.
    if (options?.joinSessionId) {
      try { localStorage.setItem(`nabiman_term_sid_${id}`, options.joinSessionId); } catch { /* ignore */ }
    }
    setWindows((prev) => {
      const offset = (prev.length % 6) * 28;
      const maxX = Math.max(0, window.innerWidth - DEFAULT_W - 20);
      const maxY = Math.max(0, window.innerHeight - DEFAULT_H - TASKBAR_H - 20);
      const win: TermWindow = {
        id,
        title: options?.title || (target.kind === 'local' ? 'Local Shell' : describe(target)),
        subtitle: describe(target),
        target,
        x: clamp(60 + offset, 0, maxX),
        y: clamp(70 + offset, 0, maxY),
        w: Math.min(DEFAULT_W, Math.max(MIN_W, window.innerWidth - 40)),
        h: Math.min(DEFAULT_H, Math.max(MIN_H, window.innerHeight - 120)),
        z: topZ.current,
        minimized: false,
        maximized: false,
      };
      return [...prev, win];
    });
    return id;
  }, []);

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
  const endSession = useCallback((id: string) => {
    const sid = storedSessionId(id);
    if (sid) {
      apiRequest(`/api/terminal/sessions/${encodeURIComponent(sid)}`, { method: 'DELETE' })
        .catch(() => { /* already gone */ });
    }
    forgetTerminalStorage(id);
    setWindows((prev) => prev.filter((w) => w.id !== id));
  }, []);

  /** Lay every open window out in a grid that fills the screen. */
  const tile = useCallback(() => {
    setWindows((prev) => {
      const open = prev.filter((w) => !w.minimized);
      if (open.length === 0) return prev;
      const cols = Math.ceil(Math.sqrt(open.length));
      const rows = Math.ceil(open.length / cols);
      const availW = window.innerWidth - 16;
      const availH = window.innerHeight - TASKBAR_H - 16;
      const cellW = Math.max(MIN_W, Math.floor(availW / cols));
      const cellH = Math.max(MIN_H, Math.floor(availH / rows));
      let i = 0;
      return prev.map((w) => {
        if (w.minimized) return w;
        const col = i % cols;
        const row = Math.floor(i / cols);
        i += 1;
        return {
          ...w, maximized: false,
          x: 8 + col * cellW, y: 8 + row * cellH,
          w: cellW - 8, h: cellH - 8,
        };
      });
    });
  }, []);

  const cascade = useCallback(() => {
    setWindows((prev) => {
      let i = 0;
      return prev.map((w) => {
        if (w.minimized) return w;
        const offset = i * 32;
        i += 1;
        return {
          ...w, maximized: false,
          x: 40 + offset, y: 60 + offset,
          w: Math.min(DEFAULT_W, window.innerWidth - offset - 80),
          h: Math.min(DEFAULT_H, window.innerHeight - offset - 140),
        };
      });
    });
  }, []);

  // Keep windows reachable when the browser window shrinks.
  useEffect(() => {
    const onResize = () => {
      setWindows((prev) => prev.map((w) => ({
        ...w,
        x: clamp(w.x, 0, Math.max(0, window.innerWidth - 120)),
        y: clamp(w.y, 0, Math.max(0, window.innerHeight - 80)),
      })));
    };
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  const value = useMemo<Ctx>(() => ({
    windows, openTerminal, closeWindow, endSession, focusWindow, tile, cascade,
    openSettings: () => setShowSettings(true),
  }), [windows, openTerminal, closeWindow, endSession, focusWindow, tile, cascade]);

  return (
    <TerminalWindowsContext.Provider value={value}>
      {children}
      <TerminalWindowLayer
        windows={windows}
        onUpdate={update}
        onFocus={focusWindow}
        onClose={closeWindow}
        onEndSession={endSession}
        onTile={tile}
        onCascade={cascade}
        onOpenSettings={() => setShowSettings(true)}
      />
      {showSettings && <TerminalSettingsModal onClose={() => setShowSettings(false)} />}
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

interface LayerProps {
  windows: TermWindow[];
  onUpdate: (id: string, patch: Partial<TermWindow>) => void;
  onFocus: (id: string) => void;
  onClose: (id: string) => void;
  onEndSession: (id: string) => void;
  onTile: () => void;
  onCascade: () => void;
  onOpenSettings: () => void;
}

function TerminalWindowLayer({
  windows, onUpdate, onFocus, onClose, onEndSession, onTile, onCascade, onOpenSettings,
}: LayerProps) {
  const { t } = useT();
  if (windows.length === 0) return null;
  return (
    <div className="term-window-layer">
      {windows.map((win) => (
        <TerminalWindowFrame
          key={win.id}
          win={win}
          onUpdate={onUpdate}
          onFocus={onFocus}
          onClose={onClose}
          onEndSession={onEndSession}
        />
      ))}
      <div className="term-taskbar">
        <span className="term-taskbar-label">{t('tab.terminal')} {windows.length}</span>
        {windows.map((win) => (
          <button
            key={win.id}
            className={`term-taskbar-item ${win.minimized ? 'minimized' : ''}`}
            onClick={() => onFocus(win.id)}
            title={win.subtitle}
          >
            {win.minimized ? '▣ ' : '▪ '}{win.title}
          </button>
        ))}
        <span className="term-taskbar-spacer" />
        <button className="term-taskbar-btn" onClick={onTile} title={t('terminal.tile')}>⊞ {t('terminal.tile')}</button>
        <button className="term-taskbar-btn" onClick={onCascade} title={t('terminal.cascade')}>⧉ {t('terminal.cascade')}</button>
        <button className="term-taskbar-btn" onClick={onOpenSettings} title={t('terminal.settings')}>⚙ {t('common.settings')}</button>
      </div>
    </div>
  );
}

interface FrameProps {
  win: TermWindow;
  onUpdate: (id: string, patch: Partial<TermWindow>) => void;
  onFocus: (id: string) => void;
  onClose: (id: string) => void;
  onEndSession: (id: string) => void;
}

function TerminalWindowFrame({ win, onUpdate, onFocus, onClose, onEndSession }: FrameProps) {
  const { t } = useT();
  const { settings } = useTerminalSettings();
  const viewRef = useRef<TerminalViewHandle>(null);
  const [status, setStatus] = useState<TerminalStatus>('connecting');

  const beginDrag = (e: React.PointerEvent) => {
    // Pointer-down on a title-bar button must not start a drag: capturing the
    // pointer on the bar would swallow the pointerup and the button would
    // never see a click.
    if ((e.target as HTMLElement).closest('button')) return;
    if (e.button > 0) return; // primary button (or touch) only
    if (win.maximized) return;
    const handle = e.currentTarget as HTMLElement;
    try { handle.setPointerCapture(e.pointerId); } catch { /* unsupported */ }
    const startX = e.clientX;
    const startY = e.clientY;
    const originX = win.x;
    const originY = win.y;
    onFocus(win.id);
    const move = (ev: PointerEvent) => {
      onUpdate(win.id, {
        x: clamp(originX + ev.clientX - startX, -win.w + 120, window.innerWidth - 80),
        y: clamp(originY + ev.clientY - startY, 0, window.innerHeight - 60),
      });
    };
    const up = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', up);
      handle.removeEventListener('lostpointercapture', up);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
    handle.addEventListener('lostpointercapture', up);
  };

  const beginResize = (e: React.PointerEvent, edge: 'se' | 'e' | 's') => {
    e.stopPropagation();
    const handle = e.currentTarget as HTMLElement;
    try { handle.setPointerCapture(e.pointerId); } catch { /* unsupported */ }
    const startX = e.clientX;
    const startY = e.clientY;
    const originW = win.w;
    const originH = win.h;
    onFocus(win.id);
    const move = (ev: PointerEvent) => {
      const patch: Partial<TermWindow> = { maximized: false };
      if (edge !== 's') patch.w = Math.max(MIN_W, originW + ev.clientX - startX);
      if (edge !== 'e') patch.h = Math.max(MIN_H, originH + ev.clientY - startY);
      onUpdate(win.id, patch);
    };
    const up = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', up);
      handle.removeEventListener('lostpointercapture', up);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
    handle.addEventListener('lostpointercapture', up);
  };

  const toggleMaximize = () => {
    if (win.maximized) {
      const r = win.restore;
      onUpdate(win.id, {
        maximized: false,
        ...(r ? { x: r.x, y: r.y, w: r.w, h: r.h } : {}),
      });
    } else {
      onUpdate(win.id, {
        maximized: true,
        restore: { x: win.x, y: win.y, w: win.w, h: win.h },
      });
    }
  };

  const style: React.CSSProperties = win.maximized
    ? { left: 0, top: 0, width: '100vw', height: `calc(100vh - ${TASKBAR_H}px)`, zIndex: win.z }
    : { left: win.x, top: win.y, width: win.w, height: win.h, zIndex: win.z };

  return (
    <div
      className={`term-window ${win.minimized ? 'term-window-minimized' : ''}`}
      style={style}
      onPointerDown={(e) => { if (!(e.target as HTMLElement).closest('.term-window-actions')) onFocus(win.id); }}
    >
      <div
        className="term-window-bar"
        onPointerDown={beginDrag}
        onDoubleClick={(e) => { if (!(e.target as HTMLElement).closest('button')) toggleMaximize(); }}
      >
        <span className={`term-window-dot term-window-dot-${status}`} />
        <span className="term-window-title">{win.title}</span>
        <span className="term-window-sub">{win.subtitle}</span>
        <span className="term-window-actions">
          <button title={t('terminal.reconnect')} onClick={(e) => { e.stopPropagation(); viewRef.current?.reconnect(); }}>↻</button>
          <button title={t('terminal.newSession')} onClick={(e) => { e.stopPropagation(); viewRef.current?.newSession(); }}>+</button>
          <button title={t('terminal.minimize')} onClick={(e) => { e.stopPropagation(); onUpdate(win.id, { minimized: true }); }}>─</button>
          <button title={win.maximized ? t('terminal.restoreSize') : t('terminal.maximize')} onClick={(e) => { e.stopPropagation(); toggleMaximize(); }}>▢</button>
          <button
            title={t('terminal.endSessionHint')}
            onClick={(e) => {
              e.stopPropagation();
              if (window.confirm(t('terminal.endSessionConfirm', { name: win.title }))) {
                onEndSession(win.id);
              }
            }}
          >⏻</button>
          <button
            className="term-window-close"
            title={t('terminal.closeWindowHint')}
            onClick={(e) => { e.stopPropagation(); onClose(win.id); }}
          >✕</button>
        </span>
      </div>
      <div className="term-window-body">
        <TerminalView
          ref={viewRef}
          target={win.target}
          storageKey={win.id}
          settings={settings}
          active={!win.minimized}
          onStatus={setStatus}
        />
      </div>
      <div className="term-resize term-resize-e" onPointerDown={(e) => beginResize(e, 'e')} />
      <div className="term-resize term-resize-s" onPointerDown={(e) => beginResize(e, 's')} />
      <div className="term-resize term-resize-se" onPointerDown={(e) => beginResize(e, 'se')} />
    </div>
  );
}
