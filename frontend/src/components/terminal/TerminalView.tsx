import React, { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { apiRequest } from '../../hooks/useApi';
import { attachClipboard, attachTerminalIo, createTerminal } from './terminalChrome';
import TerminalStatusPill from './TerminalStatusPill';
import { useScreenBuffer } from './useScreenBuffer';
import { buildTerminalUrl } from './terminalUrl';
import { bufKey, readStored, sidKey, writeStored } from './terminalStorage';
import { useTerminalZoom } from './useTerminalZoom';
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
  /** Drop a hand-made zoom and go back to filling the window. */
  resetZoom: () => void;
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
  /** Opened through a share link: no account, no session bookkeeping. */
  share?: { token: string; ticket: string };
  /** Mirror this grid instead of fitting the window (share viewers). */
  grid?: { cols: number; rows: number };
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
/** The server telling a share viewer how big the owner's pane is. */
const SIZE_MSG = '\x02SIZE:';
/** Clipboard text on its way out, for the server to hand to tmux. */
const PASTE_MSG = '\x01PASTE:';

const TerminalView = forwardRef<TerminalViewHandle, Props>(function TerminalView(
  { target, storageKey, settings, joinSessionId, active, share, grid, onStatus, onSessionId }, ref,
) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
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
  const shareRef = useRef(share);
  const [status, setStatus] = useState<TerminalStatus>('connecting');
  const { t } = useT();
  const { chunksRef, record, saveBuffer, clearBuffer } = useScreenBuffer(storageKey, settingsRef);

  settingsRef.current = settings;
  targetRef.current = target;
  joinRef.current = joinSessionId;
  shareRef.current = share;

  // How big the text is drawn, and how a watched grid is followed: see
  // ./useTerminalZoom. Zoom belongs to the session, not to this pane.
  const zoom = useTerminalZoom({
    storageKey,
    containerRef,
    termRef,
    fitRef,
    isShare: () => Boolean(shareRef.current),
    initialGrid: grid,
  });
  /** The pane is created once, so the mount effect reads the hook through a ref. */
  const zoomRef = useRef(zoom);
  zoomRef.current = zoom;

  const report = useCallback((s: TerminalStatus) => {
    setStatus(s);
    onStatus?.(s);
  }, [onStatus]);

  const buildUrl = useCallback((sessionId: string) => buildTerminalUrl({
    share: shareRef.current,
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

    // A reconnect keeps the text size the viewer chose; only the grid is
    // taken from the owner again once the socket reports it.
    zoom.refresh();
    if (attemptsRef.current > 0 && !shareRef.current) {
      // A long-lived pane can outlive its access token. One authenticated REST
      // call refreshes it (see fetchWithRefresh) before the socket carries it.
      await apiRequest('/api/terminal/settings');
      if (closedRef.current) return;
    }

    const sessionId = shareRef.current
      ? ''
      : joinRef.current || sessionIdRef.current || readStored(sidKey(storageKey));

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
        if (event.data.startsWith(SIZE_MSG)) {
          // Follow the owner's grid without resizing their pane. The text size
          // stays local: a viewer who has zoomed keeps their own.
          const [cols, rows] = event.data.slice(SIZE_MSG.length).split(':').map(Number);
          zoom.followOwnerGrid(cols, rows);
          return;
        }
        if (event.data.startsWith('\x02SESSION:')) {
          const sid = event.data.slice(9);
          sessionIdRef.current = sid;
          writeStored(sidKey(storageKey), sid);
          zoom.adoptSessionZoom(sid);
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
  }, [buildUrl, chunksRef, onSessionId, record, report, saveBuffer, storageKey, t, zoom]);

  // Create the terminal once, restore the saved screen, then connect.
  useEffect(() => {
    // A pane reopens at the size its session was zoomed to, not the settings size.
    const size = zoomRef.current.storedSize();
    const { term, fit: fitAddon } = createTerminal(
      size ? { ...settingsRef.current, font_size: size } : settingsRef.current,
    );
    appliedFontRef.current = settingsRef.current.font_size;
    termRef.current = term;
    fitRef.current = fitAddon;
    closedRef.current = false;
    attemptsRef.current = 0;

    if (containerRef.current) {
      term.open(containerRef.current);
      zoomRef.current.applyInitialLayout(term, fitAddon);
    }

    const saved = readStored(bufKey(storageKey));
    if (saved) {
      chunksRef.current = [saved];
      term.write(saved);
      term.write('\r\n\x1b[90m──────── 이전 화면 복원됨 / restored ────────\x1b[0m\r\n');
    }

    const detachClipboard = attachClipboard(term, containerRef.current, (text) => {
      if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(PASTE_MSG + text);
    });

    attachTerminalIo(
      term,
      (data) => {
        if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(data as string);
      },
      // A share viewer mirrors the pane; only the owner's panes may resize it.
      shareRef.current ? null : (cols, rows) => {
        if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(`\x01RESIZE:${cols}:${rows}`);
      },
    );

    connect();

    const saveTimer = window.setInterval(saveBuffer, SAVE_INTERVAL_MS);
    return () => {
      closedRef.current = true;
      detachClipboard();
      window.clearInterval(saveTimer);
      if (retryTimerRef.current) window.clearTimeout(retryTimerRef.current);
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
      frame = requestAnimationFrame(zoom.relayout);
    });
    observer.observe(el);
    return () => { cancelAnimationFrame(frame); observer.disconnect(); };
  }, [zoom.relayout]);

  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    term.options.scrollback = settings.scrollback_lines;
    // Changing the size in settings is deliberate, so it clears a pane's zoom.
    if (settings.font_size !== appliedFontRef.current) {
      appliedFontRef.current = settings.font_size;
      zoom.forget(settings.font_size);
      return;
    }
    zoom.relayout();
  }, [settings.font_size, settings.scrollback_lines, zoom]);

  useEffect(() => {
    if (!active) return;
    const id = window.setTimeout(() => {
      zoom.relayout();
      termRef.current?.focus();
    }, 50);
    return () => window.clearTimeout(id);
  }, [active, zoom]);

  useImperativeHandle(ref, () => ({
    focus: () => termRef.current?.focus(),
    fit: zoom.relayout,
    clear: () => {
      termRef.current?.clear();
      clearBuffer();
    },
    reconnect: () => { attemptsRef.current = 0; endedRef.current = false; connect(); },
    newSession: () => {
      endedRef.current = false;
      everConnectedRef.current = false;
      sessionIdRef.current = '';
      joinRef.current = undefined;
      writeStored(sidKey(storageKey), '');
      clearBuffer();
      termRef.current?.reset();
      attemptsRef.current = 0;
      connect();
    },
    resetZoom: () => zoom.forget(settingsRef.current.font_size),
    sessionId: () => sessionIdRef.current,
  }), [clearBuffer, connect, storageKey, zoom]);

  const retry = () => {
    attemptsRef.current = 0;
    if (status === 'ended') {
      // A finished session cannot be re-attached; start a fresh one.
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
      <TerminalStatusPill status={status} onRetry={retry} />
    </div>
  );
});

export default TerminalView;
