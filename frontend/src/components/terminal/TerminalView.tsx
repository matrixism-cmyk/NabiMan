import React, { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { apiRequest } from '../../hooks/useApi';
import { attachClipboard, attachWheelZoom, createTerminal } from './terminalChrome';
import { buildTerminalUrl } from './terminalUrl';
import {
  bufKey, forgetFontSize, MAX_BUFFER_CHARS, readFontSize, readStored, sidKey, writeFontSize,
  writeStored,
} from './terminalStorage';
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
const SAVE_INTERVAL_MS = 3000;
const MAX_RECONNECT_DELAY_MS = 15000;
/** Retries for a socket that has never opened — a broken target, not a blip. */
const MAX_COLD_ATTEMPTS = 8;
/** The server says the session is gone for good; do not start a new one. */
const ENDED_MSG = '\x02ENDED';

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
  /** The settings size this pane last applied, so a zoom is not undone by it. */
  const appliedFontRef = useRef(settings.font_size);
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

  const buildUrl = useCallback((sessionId: string) => buildTerminalUrl({
    settings: settingsRef.current,
    target: targetRef.current,
    sessionId,
    join: Boolean(joinRef.current),
    cols: termRef.current?.cols ?? 0,
    rows: termRef.current?.rows ?? 0,
  }), []);

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
    // A pane reopens at the size it was zoomed to, not at the settings size.
    const zoom = readFontSize(storageKey);
    const { term, fit: fitAddon } = createTerminal(
      zoom ? { ...settingsRef.current, font_size: zoom } : settingsRef.current,
    );
    appliedFontRef.current = settingsRef.current.font_size;
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

    const detachClipboard = attachClipboard(term, containerRef.current, (text) => {
      if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(text);
    });

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
    return () => {
      closedRef.current = true;
      detachClipboard();
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
    term.options.scrollback = settings.scrollback_lines;
    // Changing the size in settings is deliberate, so it clears a pane's zoom.
    if (settings.font_size !== appliedFontRef.current) {
      appliedFontRef.current = settings.font_size;
      term.options.fontSize = settings.font_size;
      forgetFontSize(storageKey);
    }
    try { fitRef.current?.fit(); } catch { /* ignore */ }
  }, [settings.font_size, settings.scrollback_lines, storageKey]);

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
    return attachWheelZoom(
      el,
      () => termRef.current,
      () => { try { fitRef.current?.fit(); } catch { /* ignore */ } },
      (size) => writeFontSize(storageKey, size),
    );
  }, [storageKey]);

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
