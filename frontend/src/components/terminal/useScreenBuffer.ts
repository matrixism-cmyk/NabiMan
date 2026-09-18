import { useCallback, useRef } from 'react';
import { bufKey, MAX_BUFFER_CHARS, writeStored } from './terminalStorage';
import { TerminalSettings } from './settings';

/**
 * Mirrors what a pane printed into localStorage, capped at the configured
 * scrollback, so the screen survives a reload and a closed window.
 */
export function useScreenBuffer(storageKey: string, settingsRef: { current: TerminalSettings }) {
  const chunksRef = useRef<string[]>([]);
  const dirtyRef = useRef(false);

  const record = useCallback((text: string) => {
    chunksRef.current.push(text);
    dirtyRef.current = true;
  }, []);

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
  }, [storageKey, settingsRef]);

  /** Forget the screen — used when a pane starts a brand new session. */
  const clearBuffer = useCallback(() => {
    chunksRef.current = [];
    dirtyRef.current = true;
    saveBuffer();
  }, [saveBuffer]);

  return { chunksRef, record, saveBuffer, clearBuffer };
}
