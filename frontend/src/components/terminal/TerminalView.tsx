import React, { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import 'xterm/css/xterm.css';
import { apiRequest } from '../../hooks/useApi';
import { useT } from '../../i18n';
import { TerminalSettings } from './settings';

export interface TerminalTarget {
  kind: 'local' | 'ssh';
  host?: string;
  port?: number;
  user?: string;
  /** Saved remote server id — lets the backend use its stored password. */
  serverId?: string;
  /** Connect as if no password were stored, so it can be typed in the pane. */
  skipSavedPassword?: boolean;
}

export type TerminalStatus = 'connecting' | 'connected' | 'reconnecting' | 'disconnected' | 'ended';

export interface TerminalViewHandle {
  focus: () => void;
  fit: () => void;
  clear: () => void;
  reconnect: () => void;
  /** Abandon the current tmux session and start a brand new one. */
  newSession: () => void;
  sessionId: () => string;
}

interface Props {
  target: TerminalTarget;
  /** Stable identity of this pane: keys the stored session id and screen buffer. */
  storageKey: string;
  settings: TerminalSettings;
  /** Attach to this existing session instead of the stored/new one. */
  joinSessionId?: string;
  active?: boolean;
  onStatus?: (status: TerminalStatus) => void;
  onSessionId?: (id: string) => void;
}

const THEME = {
  background: '#0f1923', foreground: '#e0e6ed', cursor: '#3b82f6', cursorAccent: '#0f1923',
  selectionBackground: '#3b82f655', selectionForeground: '#ffffff', selectionInactiveBackground: '#3b82f633',
  black: '#1a2634', red: '#e74c3c', green: '#2ecc71', yellow: '#f39c12', blue: '#3b82f6',
  magenta: '#9b59b6', cyan: '#1abc9c', white: '#e0e6ed',
  brightBlack: '#4a5a6a', brightRed: '#ff6b6b', brightGreen: '#6bff8d', brightYellow: '#ffd93d',
  brightBlue: '#6bb3ff', brightMagenta: '#c471ed', brightCyan: '#45e6c6', brightWhite: '#ffffff',
};

/** Raw output kept per window in localStorage so a reload still shows history.
 *  512 KB comfortably holds the 5,000-line default without crowding the
 *  browser's storage quota when many windows are open. */
const MAX_BUFFER_CHARS = 512 * 1024;
const SAVE_INTERVAL_MS = 3000;
const MAX_RECONNECT_DELAY_MS = 15000;
/** Retries for a socket that has never opened — a broken target, not a blip. */
const MAX_COLD_ATTEMPTS = 8;
/** The server says the session is gone for good; do not start a new one. */
const ENDED_MSG = '\x02ENDED';

const bufKey = (key: string) => `nabiman_term_buf_${key}`;
const sidKey = (key: string) => `nabiman_term_sid_${key}`;

function readStored(key: string): string {
  try { return localStorage.getItem(key) || ''; } catch { return ''; }
}

function writeStored(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Out of quota (many windows open): drop every other saved screen and try
    // once more, so at least the pane being written keeps its history.
    try {
      const stale: string[] = [];
      for (let i = 0; i < localStorage.length; i++) {
        const k = localStorage.key(i);
        if (k && k.startsWith(bufKey('')) && k !== key) stale.push(k);
      }
      stale.forEach((k) => localStorage.removeItem(k));
      localStorage.setItem(key, value);
    } catch { /* private mode or still too large — history is best effort */ }
  }
}

/**
 * One terminal pane: an xterm.js screen wired to a tmux-backed PTY over a
 * WebSocket.
 *
 * The xterm instance is created once and never disposed while the pane is
 * mounted, so a dropped connection leaves the screen (and its scrollback)
 * exactly as it was. Output is also mirrored to localStorage, capped at the
 * configured scrollback, so a page reload restores what was on screen even
 * before the socket is back.
 */
const TerminalView = forwardRef<TerminalViewHandle, Props>(function TerminalView(
  { target, storageKey, settings, joinSessionId, active, onStatus, onSessionId }, ref,
) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const chunksRef = useRef<string[]>([]);
  const dirtyRef = useRef(false);
  const decoderRef = useRef(new TextDecoder());
  const closedRef = useRef(false);
  const endedRef = useRef(false);
  const everConnectedRef = useRef(false);
  const attemptsRef = useRef(0);
  const retryTimerRef = useRef<number | null>(null);
  const sessionIdRef = useRef<string>('');
  const settingsRef = useRef(settings);
  const targetRef = useRef(target);
  const joinRef = useRef(joinSessionId);
  const [status, setStatus] = useState<TerminalStatus>('connecting');
  const { t } = useT();

  settingsRef.current = settings;
  targetRef.current = target;
  joinRef.current = joinSessionId;

  const report = useCallback((s: TerminalStatus) => {
    setStatus(s);
    onStatus?.(s);
  }, [onStatus]);

  const saveBuffer = useCallback(() => {
    if (!dirtyRef.current) return;
    dirtyRef.current = false;
    let text = chunksRef.current.join('');
    if (text.length > MAX_BUFFER_CHARS) text = text.slice(text.length - MAX_BUFFER_CHARS);
    const limit = settingsRef.current.scrollback_lines;
    const lines = text.split('\n');
    if (lines.length > limit) text = lines.slice(lines.length - limit).join('\n');
    chunksRef.current = [text];
    writeStored(bufKey(storageKey), text);
  }, [storageKey]);

  const record = useCallback((text: string) => {
    chunksRef.current.push(text);
    dirtyRef.current = true;
  }, []);

  const buildUrl = useCallback((sessionId: string) => {
    const token = sessionStorage.getItem('nabiman_token') || '';
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const s = settingsRef.current;
    const t = targetRef.current;
    let url = `${protocol}//${window.location.host}/api/terminal?token=${encodeURIComponent(token)}`;
    url += `&scrollback=${s.scrollback_lines}`;
    // Create/attach at the size this pane already has: a mismatch would make
    // tmux reflow the pane on attach and scroll the first lines out of view.
    const term = termRef.current;
    if (term && term.cols > 0 && term.rows > 0) url += `&cols=${term.cols}&rows=${term.rows}`;
    url += `&keepalive=${s.keepalive ? 1 : 0}`;
    if (!s.keepalive) url += `&timeout=${s.idle_timeout_secs}`;
    if (sessionId) url += `&session_id=${encodeURIComponent(sessionId)}`;
    // An explicit join must attach to *that* session or report it gone; only a
    // remembered id may quietly fall back to starting a fresh session.
    if (sessionId && joinRef.current) url += '&join=1';
    if (t.kind === 'ssh') {
      if (t.serverId) {
        url += `&server_id=${encodeURIComponent(t.serverId)}`;
        if (t.skipSavedPassword) url += '&no_saved_password=1';
      } else if (t.host) {
        url += `&ssh_host=${encodeURIComponent(t.host)}`;
        url += `&ssh_port=${encodeURIComponent(String(t.port || 22))}`;
        url += `&ssh_user=${encodeURIComponent(t.user || 'root')}`;
      }
    }
    return url;
  }, []);

  const connect = useCallback(async () => {
    const term = termRef.current;
    if (!term || closedRef.current) return;
    if (retryTimerRef.current) { window.clearTimeout(retryTimerRef.current); retryTimerRef.current = null; }
    if (wsRef.current) {
      const stale = wsRef.current;
      wsRef.current = null;
      stale.onclose = null;
      stale.close();
    }

    endedRef.current = false;
    report(attemptsRef.current > 0 ? 'reconnecting' : 'connecting');

    if (attemptsRef.current > 0) {
      // A long-lived pane can outlive its access token. One authenticated REST
      // call refreshes it (see fetchWithRefresh) before the socket carries it.
      await apiRequest('/api/terminal/settings');
      if (closedRef.current) return;
    }

    const sessionId = joinRef.current || sessionIdRef.current || readStored(sidKey(storageKey));

    // Reopening a window with an empty screen: pull the history tmux kept
    // while nothing was attached, so the previous output is there again.
    if (sessionId && settingsRef.current.restore_scrollback && chunksRef.current.length === 0) {
      const res = await apiRequest<string>(
        `/api/terminal/sessions/${encodeURIComponent(sessionId)}/scrollback?lines=${settingsRef.current.scrollback_lines}`,
      );
      if (res.success && res.data) {
        const text = res.data.replace(/\r?\n/g, '\r\n');
        term.write(text);
        record(text);
      }
      // The pane may have been closed while the history was in flight.
      if (closedRef.current) return;
    }

    const ws = new WebSocket(buildUrl(sessionId));
    ws.binaryType = 'arraybuffer';
    wsRef.current = ws;

    ws.onopen = () => {
      attemptsRef.current = 0;
      everConnectedRef.current = true;
      report('connected');
      window.setTimeout(() => {
        if (ws.readyState === WebSocket.OPEN) ws.send(`\x01RESIZE:${term.cols}:${term.rows}`);
      }, 150);
    };

    ws.onmessage = (event) => {
      if (typeof event.data === 'string') {
        if (event.data.startsWith(ENDED_MSG)) {
          // The shell exited or the session was killed: stop retrying and let
          // the operator start a new one deliberately.
          endedRef.current = true;
          sessionIdRef.current = '';
          joinRef.current = undefined;
          writeStored(sidKey(storageKey), '');
          term.write(`\r\n\x1b[33m[${t('terminal.sessionEnded')}]\x1b[0m\r\n`);
          report('ended');
          return;
        }
        if (event.data.startsWith('\x02SESSION:')) {
          const sid = event.data.slice(9);
          sessionIdRef.current = sid;
          writeStored(sidKey(storageKey), sid);
          onSessionId?.(sid);
          return;
        }
        term.write(event.data);
        record(event.data);
        return;
      }
      const bytes = new Uint8Array(event.data as ArrayBuffer);
      term.write(bytes);
      record(decoderRef.current.decode(bytes, { stream: true }));
    };

    ws.onerror = () => { /* onclose follows and drives the retry */ };

    ws.onclose = () => {
      if (wsRef.current === ws) wsRef.current = null;
      saveBuffer();
      if (closedRef.current) return;
      if (endedRef.current) { report('ended'); return; }
      // The tmux session keeps running server-side, so reconnecting simply
      // re-attaches to it — with keep-alive on we retry indefinitely. A socket
      // that never opened is a different story: stop after a few tries instead
      // of hammering an endpoint that cannot serve this pane.
      const attempt = attemptsRef.current++;
      const maxAttempts = everConnectedRef.current
        ? (settingsRef.current.keepalive ? Infinity : 5)
        : MAX_COLD_ATTEMPTS;
      if (attempt >= maxAttempts) { report('disconnected'); return; }
      const delay = Math.min(MAX_RECONNECT_DELAY_MS, 1000 * Math.pow(2, Math.min(attempt, 4)));
      report('reconnecting');
      retryTimerRef.current = window.setTimeout(() => { connect(); }, delay);
    };
  }, [buildUrl, onSessionId, record, report, saveBuffer, storageKey, t]);

  // Create the terminal once, restore the saved screen, then connect.
  useEffect(() => {
    const term = new Terminal({
      cursorBlink: true, cursorStyle: 'block', fontSize: settingsRef.current.font_size,
      fontFamily: "'Fira Code', 'Cascadia Code', 'JetBrains Mono', 'Menlo', monospace",
      fontWeight: '400', fontWeightBold: '700', lineHeight: 1.15, theme: THEME,
      allowProposedApi: true, scrollback: settingsRef.current.scrollback_lines,
      rightClickSelectsWord: true, convertEol: true,
    });
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());
    termRef.current = term;
    fitRef.current = fitAddon;
    closedRef.current = false;
    attemptsRef.current = 0;

    if (containerRef.current) {
      term.open(containerRef.current);
      try { fitAddon.fit(); } catch { /* container not laid out yet */ }
    }

    const saved = readStored(bufKey(storageKey));
    if (saved) {
      chunksRef.current = [saved];
      term.write(saved);
      term.write('\r\n\x1b[90m──────── 이전 화면 복원됨 / restored ────────\x1b[0m\r\n');
    }

    term.attachCustomKeyEventHandler((ev: KeyboardEvent) => {
      if (ev.type !== 'keydown') return true;
      if (ev.ctrlKey && ev.shiftKey && ev.key === 'C') {
        const sel = term.getSelection();
        if (sel) navigator.clipboard.writeText(sel).catch(() => {});
        return false;
      }
      if (ev.ctrlKey && ev.shiftKey && ev.key === 'V') {
        navigator.clipboard.readText().then((text) => {
          if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(text);
        }).catch(() => {});
        return false;
      }
      return true;
    });

    // Right-click copies the selection, or pastes when nothing is selected —
    // the same muscle memory as PuTTY and the previous terminal panel.
    const onContextMenu = (ev: MouseEvent) => {
      ev.preventDefault();
      const sel = term.getSelection();
      if (sel) {
        navigator.clipboard.writeText(sel).catch(() => {});
        term.clearSelection();
      } else {
        navigator.clipboard.readText().then((text) => {
          if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(text);
        }).catch(() => {});
      }
    };
    containerRef.current?.addEventListener('contextmenu', onContextMenu);

    term.onData((data) => {
      if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(data);
    });
    term.onBinary((data) => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        const buf = new Uint8Array(data.length);
        for (let i = 0; i < data.length; i++) buf[i] = data.charCodeAt(i);
        wsRef.current.send(buf.buffer);
      }
    });
    term.onResize(({ cols, rows }) => {
      if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(`\x01RESIZE:${cols}:${rows}`);
    });

    connect();

    const saveTimer = window.setInterval(saveBuffer, SAVE_INTERVAL_MS);
    const container = containerRef.current;
    return () => {
      closedRef.current = true;
      container?.removeEventListener('contextmenu', onContextMenu);
      window.clearInterval(saveTimer);
      if (retryTimerRef.current) window.clearTimeout(retryTimerRef.current);
      dirtyRef.current = true;
      saveBuffer();
      wsRef.current?.close();
      wsRef.current = null;
      term.dispose();
      termRef.current = null;
    };
    // Mount-only: the pane keeps its terminal for its whole lifetime.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [storageKey]);

  // Keep the screen sized to its container (window drag/resize, layout change).
  useEffect(() => {
    const el = containerRef.current;
    if (!el || typeof ResizeObserver === 'undefined') return;
    let frame = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        try { fitRef.current?.fit(); } catch { /* hidden container */ }
      });
    });
    observer.observe(el);
    return () => { cancelAnimationFrame(frame); observer.disconnect(); };
  }, []);

  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    term.options.fontSize = settings.font_size;
    term.options.scrollback = settings.scrollback_lines;
    try { fitRef.current?.fit(); } catch { /* ignore */ }
  }, [settings.font_size, settings.scrollback_lines]);

  useEffect(() => {
    if (!active) return;
    const id = window.setTimeout(() => {
      try { fitRef.current?.fit(); } catch { /* ignore */ }
      termRef.current?.focus();
    }, 50);
    return () => window.clearTimeout(id);
  }, [active]);

  // Ctrl + wheel zooms; a plain wheel goes to tmux (mouse mode) as scrollback.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      e.stopPropagation();
      const term = termRef.current;
      if (!term) return;
      const next = Math.min(28, Math.max(8, (term.options.fontSize || 14) + (e.deltaY < 0 ? 1 : -1)));
      term.options.fontSize = next;
      try { fitRef.current?.fit(); } catch { /* ignore */ }
    };
    el.addEventListener('wheel', onWheel, { passive: false, capture: true });
    return () => el.removeEventListener('wheel', onWheel, { capture: true } as EventListenerOptions);
  }, []);

  useImperativeHandle(ref, () => ({
    focus: () => termRef.current?.focus(),
    fit: () => { try { fitRef.current?.fit(); } catch { /* ignore */ } },
    clear: () => {
      termRef.current?.clear();
      chunksRef.current = [];
      dirtyRef.current = true;
      saveBuffer();
    },
    reconnect: () => { attemptsRef.current = 0; endedRef.current = false; connect(); },
    newSession: () => {
      endedRef.current = false;
      everConnectedRef.current = false;
      sessionIdRef.current = '';
      joinRef.current = undefined;
      writeStored(sidKey(storageKey), '');
      chunksRef.current = [];
      dirtyRef.current = true;
      saveBuffer();
      termRef.current?.reset();
      attemptsRef.current = 0;
      connect();
    },
    sessionId: () => sessionIdRef.current,
  }), [connect, saveBuffer, storageKey]);

  const statusLabel = status === 'reconnecting' ? t('terminal.statusReconnecting')
    : status === 'connecting' ? t('terminal.statusConnecting')
    : status === 'ended' ? t('terminal.statusEnded')
    : t('terminal.statusDisconnected');

  const retry = () => {
    attemptsRef.current = 0;
    if (status === 'ended') {
      // A finished session cannot be re-attached; start a fresh one.
      endedRef.current = false;
      everConnectedRef.current = false;
      chunksRef.current = [];
      termRef.current?.reset();
    }
    endedRef.current = false;
    connect();
  };

  return (
    <div className="terminal-view">
      <div ref={containerRef} className="terminal-view-screen" />
      {status !== 'connected' && (
        <button
          type="button"
          className={`terminal-view-status terminal-view-status-${status}`}
          onClick={retry}
          title={t('terminal.clickToRetry')}
        >
          {statusLabel}
        </button>
      )}
    </div>
  );
});

export default TerminalView;

/** Remove the persisted screen + session id of a pane that is going away. */
export function forgetTerminalStorage(storageKey: string) {
  try {
    localStorage.removeItem(bufKey(storageKey));
    localStorage.removeItem(sidKey(storageKey));
  } catch { /* ignore */ }
}

/** Drop stored screens/session ids for panes that no longer exist. */
export function pruneTerminalStorage(liveKeys: string[]) {
  try {
    const keep = new Set(liveKeys);
    const stale: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (!key) continue;
      const suffix = key.startsWith(bufKey('')) ? key.slice(bufKey('').length)
        : key.startsWith(sidKey('')) ? key.slice(sidKey('').length)
        : null;
      // `dock_*` belongs to the docked terminal panel, which has no window row.
      if (suffix === null || suffix.startsWith('dock_') || keep.has(suffix)) continue;
      stale.push(key);
    }
    stale.forEach((key) => localStorage.removeItem(key));
  } catch { /* ignore */ }
}

/** The session id a pane last attached to, if any. */
export function storedSessionId(storageKey: string): string {
  return readStored(sidKey(storageKey));
}
