import React, { forwardRef, useCallback, useEffect, useId, useRef, useState } from 'react';
import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { apiRequest } from '../../hooks/useApi';
import { attachClipboard, attachTerminalIo, createTerminal } from './terminalChrome';
import TerminalStatusPill from './TerminalStatusPill';
import { HistoryPosition } from './TerminalHistoryBar';
import TerminalHistoryControls from './TerminalHistoryControls';
import { useScreenBuffer } from './useScreenBuffer';
import { buildTerminalUrl, ticketedTerminalUrl, ticketedShareUrl } from './terminalUrl';
import { bufKey, markEnded, paneEnded, readStored, sidKey, storedSessionId, writeStored } from './terminalStorage';
import { useTerminalZoom } from './useTerminalZoom';
import { useTerminalHandle } from './useTerminalHandle';
import { useT } from '../../i18n';

import type { TerminalStatus, TerminalViewHandle, TerminalViewProps } from './terminalTypes';
export type { TerminalTarget, TerminalStatus, TerminalViewHandle } from './terminalTypes';

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

const TerminalView = forwardRef<TerminalViewHandle, TerminalViewProps>(function TerminalView(
  { target, storageKey, settings, joinSessionId, active, share, grid, onStatus, onSessionId }, ref,
) {
  const screenId = useId();
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const connectGeneration = useRef(0);
  const ticketController = useRef<AbortController | null>(null);
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
  const [historyPosition, setHistoryPosition] = useState<HistoryPosition | null>(null);
  const [status, setStatus] = useState<TerminalStatus>('connecting');
  const { t } = useT();
  const { chunksRef, record, saveBuffer, clearBuffer } = useScreenBuffer(storageKey, settingsRef);

  settingsRef.current = settings;
  targetRef.current = target;
  joinRef.current = joinSessionId;
  shareRef.current = share;

  // How big the text is drawn, and how a watched grid is followed: see
  // ./useTerminalZoom. Zoom belongs to the session, not to this pane.
  const isShare = useCallback(() => Boolean(shareRef.current), []);
  const zoom = useTerminalZoom({
    storageKey,
    containerRef,
    termRef,
    fitRef,
    isShare,
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
    // A pane going back to its session must not get a new one in its place
    // when that session is gone: it hears that it ended instead.
    join: true,
    cols: termRef.current?.cols ?? 0,
    rows: termRef.current?.rows ?? 0,
  }), []);

  const connect = useCallback(async () => {
    const term = termRef.current;
    if (!term || closedRef.current) return;
    const generation = ++connectGeneration.current;
    ticketController.current?.abort();
    const controller = new AbortController();
    ticketController.current = controller;
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

    const sessionId = shareRef.current
      ? ''
      : joinRef.current || sessionIdRef.current || storedSessionId(storageKey);

    // Reopening a window with an empty screen: pull the history tmux kept
    // while nothing was attached, so the previous output is there again.
    if (sessionId && settingsRef.current.restore_scrollback && chunksRef.current.length === 0) {
      try {
        const res = await apiRequest<string>(
          `/api/terminal/sessions/${encodeURIComponent(sessionId)}/scrollback?lines=${settingsRef.current.scrollback_lines}`,
          { signal: controller.signal },
        );
        if (closedRef.current || generation !== connectGeneration.current) return;
        if (res.success && res.data) {
          const text = res.data.replace(/\r?\n/g, '\r\n');
          term.write(text);
          record(text);
        }
      } catch { /* A missing history snapshot must not prevent a fresh authenticated connection. */ }
      if (closedRef.current || generation !== connectGeneration.current || controller.signal.aborted) return;
    }

    setHistoryPosition(null);
    const url = buildUrl(sessionId);
    let connectionUrl: string;
    try {
      connectionUrl = shareRef.current ? await ticketedShareUrl(url, shareRef.current, controller.signal) : await ticketedTerminalUrl(`${url}&history=1`, controller.signal);
    } catch (cause) {
      if (controller.signal.aborted || closedRef.current || generation !== connectGeneration.current) return;
      const attempt = attemptsRef.current++;
      if (attempt === 0) {
        const reason = cause instanceof Error ? cause.message : t('common.error');
        term.write(`\r\n\x1b[31m[${reason}]\x1b[0m\r\n`);
      }
      // A share admission failure requires an explicit retry; never brute-force
      // the password endpoint or retry a rejected/limited proof automatically.
      if (shareRef.current || attempt >= MAX_COLD_ATTEMPTS) { report('disconnected'); return; }
      report('reconnecting');
      retryTimerRef.current = window.setTimeout(() => { connect(); }, Math.min(MAX_RECONNECT_DELAY_MS, 1000 * Math.pow(2, attempt)));
      return;
    }
    if (closedRef.current || generation !== connectGeneration.current || controller.signal.aborted) return;
    const ws = new WebSocket(connectionUrl);
    ws.binaryType = 'arraybuffer';
    wsRef.current = ws;

    ws.onopen = () => {
      if (closedRef.current || generation !== connectGeneration.current || wsRef.current !== ws) { ws.close(); return; }
      attemptsRef.current = 0;
      everConnectedRef.current = true;
      report('connected');
      window.setTimeout(() => {
        if (ws.readyState === WebSocket.OPEN) ws.send(`\x01RESIZE:${term.cols}:${term.rows}`);
      }, 150);
    };

    ws.onmessage = (event) => {
      if (closedRef.current || generation !== connectGeneration.current || wsRef.current !== ws) return;
      if (typeof event.data === 'string') {
        if (event.data.startsWith('\x02HISTORY:')) {
          try {
            const position: HistoryPosition = JSON.parse(event.data.slice(9));
            if ([position.total, position.offset, position.rows].every(n => Number.isFinite(n) && n >= 0)) {
              setHistoryPosition(position);
            }
          } catch { /* Ignore malformed metadata without printing it into the shell. */ }
          return;
        }
        if (event.data.startsWith(ENDED_MSG)) {
          // The shell exited or the session was killed: stop retrying and let
          // the operator start a new one deliberately.
          endedRef.current = true;
          sessionIdRef.current = '';
          joinRef.current = undefined;
          markEnded(storageKey);
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

    ws.onclose = (event) => {
      if (wsRef.current === ws) wsRef.current = null;
      saveBuffer();
      if (closedRef.current) return;
      if (endedRef.current) { report('ended'); return; }
      if (event.code === 1008) {
        term.write(`\r\n\x1b[33m[${t('terminal.accessRevoked')}]\x1b[0m\r\n`);
        report('disconnected');
        return;
      }
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
      // Canvas may render before the bundled symbols font finishes loading.
      // Refresh the atlas so early output uses the one-cell glyphs too.
      if (document.fonts) {
        void document.fonts.load(`${term.options.fontSize}px "NabiMan Circled Digits"`, '①').then(() => {
          if (!closedRef.current && termRef.current === term) {
            term.clearTextureAtlas();
            term.refresh(0, term.rows - 1);
          }
        }).catch(() => {});
      }
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

    // A pane whose session ended stays ended until asked for a new one.
    if (!shareRef.current && !joinRef.current && paneEnded(storageKey)) {
      endedRef.current = true;
      markEnded(storageKey);
      term.write(`\r\n\x1b[33m[${t('terminal.sessionEnded')}]\x1b[0m\r\n`);
      report('ended');
    } else connect();

    const saveTimer = window.setInterval(saveBuffer, SAVE_INTERVAL_MS);
    return () => {
      closedRef.current = true;
      ticketController.current?.abort();
      detachClipboard();
      window.clearInterval(saveTimer);
      if (retryTimerRef.current) window.clearTimeout(retryTimerRef.current);
      saveBuffer();
      const socket = wsRef.current;
      if (socket) {
        socket.onopen = null; socket.onmessage = null; socket.onclose = null; socket.onerror = null;
        socket.close();
      }
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

  useTerminalHandle(ref, {
    termRef, wsRef, endedRef, everConnectedRef, attemptsRef, sessionIdRef,
    joinRef, settingsRef, zoom, storageKey, clearBuffer, connect,
  });

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
    <div className={`terminal-view ${share ? '' : 'terminal-view-with-history'}`}>
      <div id={screenId} ref={containerRef} className="terminal-view-screen" />
      {!share && <TerminalHistoryControls screenId={screenId} position={historyPosition}
        connected={status === 'connected'} termRef={termRef} wsRef={wsRef} />}
      <TerminalStatusPill status={status} onRetry={retry} />
    </div>
  );
});

export default TerminalView;
