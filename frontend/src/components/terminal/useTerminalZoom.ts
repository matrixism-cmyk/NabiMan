import { MutableRefObject, RefObject, useCallback, useEffect, useMemo, useRef } from 'react';
import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { adoptGrid, attachWheelZoom, mirrorGrid } from './terminalChrome';
import { attachTouchGestures } from './touchGestures';
import {
  forgetFontSize, readFontSize, sessionZoomKey, storedSessionId, writeFontSize,
} from './terminalStorage';

interface Options {
  /** The pane's identity, used until its session is known. */
  storageKey: string;
  containerRef: RefObject<HTMLElement | null>;
  termRef: MutableRefObject<Terminal | null>;
  fitRef: MutableRefObject<FitAddon | null>;
  /** True while this pane is watching someone else's session. */
  isShare: () => boolean;
  /** The grid a share viewer starts out mirroring. */
  initialGrid?: { cols: number; rows: number };
}

export interface TerminalZoom {
  /** Re-lay out the pane after the window, the settings or the owner moved. */
  relayout: () => void;
  /** Follow the owner's grid, keeping the viewer's own text size. */
  followOwnerGrid: (cols: number, rows: number) => void;
  /** Take on the zoom stored for a session the pane just attached to. */
  adoptSessionZoom: (sessionId: string) => void;
  /** The size this pane should open at, if it was left zoomed. */
  storedSize: () => number | null;
  /** Lay out a freshly created terminal before it connects. */
  applyInitialLayout: (term: Terminal, fit: FitAddon) => void;
  /** Re-read whether the pane is zoomed, after a reconnect. */
  refresh: () => void;
  /** Drop the zoom and go back to the size from settings. */
  forget: (settingsFontSize: number) => void;
  /** Make the text a step larger (or smaller, below 0), kept like a Ctrl+wheel zoom. */
  step: (delta: number) => void;
}

/**
 * The size a terminal is drawn at, which is nobody else's business.
 *
 * Zoom is kept per session rather than per pane, so a shell looks the way it
 * was left wherever it is opened next, and a viewer watching someone else's
 * session scales their own window without touching the session's grid — two
 * people can watch the same shell at different sizes.
 */
export function useTerminalZoom({
  storageKey, containerRef, termRef, fitRef, isShare, initialGrid,
}: Options): TerminalZoom {
  const keyRef = useRef(
    storedSessionId(storageKey) ? sessionZoomKey(storedSessionId(storageKey)) : storageKey,
  );
  /** A pane that was left zoomed keeps that size instead of auto-scaling. */
  const manualRef = useRef(readFontSize(keyRef.current) !== null);
  /** The grid of the session being watched, which a viewer always follows. */
  const ownerGridRef = useRef(initialGrid && initialGrid.cols > 0 ? { ...initialGrid } : null);

  const relayout = useCallback(() => {
    const term = termRef.current;
    if (!term) return;
    const owner = ownerGridRef.current;
    if (isShare() && owner) {
      if (manualRef.current || !fitRef.current) adoptGrid(term, owner.cols, owner.rows);
      else mirrorGrid(term, fitRef.current, owner.cols, owner.rows);
      return;
    }
    try { fitRef.current?.fit(); } catch { /* hidden container */ }
  }, [fitRef, isShare, termRef]);

  const followOwnerGrid = useCallback((cols: number, rows: number) => {
    const term = termRef.current;
    if (!term || cols <= 0 || rows <= 0) return;
    ownerGridRef.current = { cols, rows };
    if (manualRef.current || !fitRef.current) adoptGrid(term, cols, rows);
    else mirrorGrid(term, fitRef.current, cols, rows);
  }, [fitRef, termRef]);

  const adoptSessionZoom = useCallback((sessionId: string) => {
    const key = sessionZoomKey(sessionId);
    if (!sessionId || keyRef.current === key) return;
    keyRef.current = key;
    const term = termRef.current;
    const stored = readFontSize(key);
    if (stored && term) {
      // The session was left at a size of its own: go back to it.
      manualRef.current = true;
      term.options.fontSize = stored;
      relayout();
    } else if (manualRef.current && term?.options.fontSize) {
      // A session with no size yet keeps what is on screen, and remembers it.
      writeFontSize(key, Math.round(term.options.fontSize));
    }
  }, [relayout, termRef]);

  const storedSize = useCallback(
    () => readFontSize(keyRef.current) ?? readFontSize(storageKey),
    [storageKey],
  );

  const applyInitialLayout = useCallback((term: Terminal, fit: FitAddon) => {
    const owner = ownerGridRef.current;
    if (owner) {
      // Match the watched session before connecting: resizing after the attach
      // redraw would clear the screen until tmux painted it again.
      if (manualRef.current) adoptGrid(term, owner.cols, owner.rows);
      else mirrorGrid(term, fit, owner.cols, owner.rows);
      return;
    }
    try { fit.fit(); } catch { /* container not laid out yet */ }
  }, []);

  const refresh = useCallback(() => {
    manualRef.current = readFontSize(keyRef.current) !== null;
  }, []);

  const forget = useCallback((settingsFontSize: number) => {
    manualRef.current = false;
    forgetFontSize(keyRef.current);
    forgetFontSize(storageKey);
    const term = termRef.current;
    if (term) {
      term.options.fontSize = settingsFontSize;
      term.options.lineHeight = 1.15;
    }
    relayout();
  }, [relayout, storageKey, termRef]);

  const zoomTo = useCallback((size: number) => {
    const term = termRef.current;
    if (!term) return;
    const next = Math.min(28, Math.max(8, Math.round(size)));
    term.options.fontSize = next;
    manualRef.current = true;
    writeFontSize(keyRef.current, next);
    relayout();
  }, [relayout, termRef]);

  const step = useCallback((delta: number) => {
    zoomTo((termRef.current?.options.fontSize || 14) + delta);
  }, [termRef, zoomTo]);

  // Ctrl + wheel resizes the text of this window alone; a plain wheel goes to
  // tmux (mouse mode) as scrollback. On a touch screen, a pinch resizes and a
  // drag scrolls.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const unWheel = attachWheelZoom(el, () => termRef.current, relayout, (size) => {
      manualRef.current = true;
      writeFontSize(keyRef.current, size);
    });
    const unTouch = attachTouchGestures(el, () => termRef.current, zoomTo);
    return () => { unWheel(); unTouch(); };
  }, [containerRef, relayout, termRef, zoomTo]);

  // One stable object, so effects that depend on it do not re-run every render.
  return useMemo(() => ({
    relayout, followOwnerGrid, adoptSessionZoom, storedSize, applyInitialLayout, refresh, forget, step,
  }), [adoptSessionZoom, applyInitialLayout, followOwnerGrid, forget, refresh, relayout, storedSize, step]);
}
