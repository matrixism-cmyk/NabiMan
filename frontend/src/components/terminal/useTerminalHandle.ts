import { useImperativeHandle } from 'react';
import type { MutableRefObject, Ref } from 'react';
import type { Terminal } from 'xterm';
import type { TerminalViewHandle } from './terminalTypes';
import type { TerminalSettings } from './settings';
import { useTerminalZoom } from './useTerminalZoom';
import { sidKey, writeStored } from './terminalStorage';
import { armModifiers, isBarKey, keySequence, NO_MODS, withMods } from './stickyKeys';

interface Controls {
  termRef: MutableRefObject<Terminal | null>;
  wsRef: MutableRefObject<WebSocket | null>;
  endedRef: MutableRefObject<boolean>;
  everConnectedRef: MutableRefObject<boolean>;
  attemptsRef: MutableRefObject<number>;
  sessionIdRef: MutableRefObject<string>;
  joinRef: MutableRefObject<string | undefined>;
  settingsRef: MutableRefObject<TerminalSettings>;
  zoom: ReturnType<typeof useTerminalZoom>;
  storageKey: string;
  clearBuffer: () => void;
  connect: () => Promise<void>;
}

export function useTerminalHandle(ref: Ref<TerminalViewHandle>, {
  termRef, wsRef, endedRef, everConnectedRef, attemptsRef, sessionIdRef,
  joinRef, settingsRef, zoom, storageKey, clearBuffer, connect,
}: Controls) {
  useImperativeHandle(ref, () => ({
    focus: () => termRef.current?.focus(),
    fit: zoom.relayout,
    redraw: () => {
      const term = termRef.current;
      const socket = wsRef.current;
      if (!term || !socket || socket.readyState !== WebSocket.OPEN) return;
      term.clearTextureAtlas();
      term.refresh(0, term.rows - 1);
      // The server refreshes this tmux client even when its size is unchanged.
      // Preserve the terminal buffer, connection, and application input.
      socket.send(`\x01RESIZE:${term.cols}:${term.rows}`);
      term.focus();
    },
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
    insertText: (text: string) => {
      termRef.current?.paste(text);
      termRef.current?.focus();
    },
    sendKeys: (keys: string) => {
      if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(keys);
      termRef.current?.focus();
    },
    pressKey: (key, mods = NO_MODS) => {
      const appCursor = termRef.current?.modes.applicationCursorKeysMode ?? false;
      const data = isBarKey(key) ? keySequence(key, appCursor, mods) : withMods(key, mods);
      if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(data);
    },
    armModifiers: (mods, used) => { if (termRef.current) armModifiers(termRef.current, mods, used); },
    zoomBy: (delta) => zoom.step(delta),
  }), [
    clearBuffer, connect, storageKey, zoom, attemptsRef, endedRef, everConnectedRef,
    joinRef, sessionIdRef, settingsRef, termRef, wsRef,
  ]);
}
