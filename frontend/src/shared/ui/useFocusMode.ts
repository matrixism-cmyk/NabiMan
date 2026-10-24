import { useCallback, useEffect, useState } from 'react';

/**
 * "Fullscreen (content only)": hide the chrome and drive the browser Fullscreen
 * API for an edge-to-edge view. Shared by the full app and the remote shell so
 * both behave identically. Esc leaves (but is left to the terminal when one has
 * focus), and leaving browser fullscreen turns it off.
 */
export function useFocusMode(): { focusMode: boolean; enterFocus: () => void; exitFocus: () => void } {
  const [focusMode, setFocusMode] = useState(false);

  const enterFocus = useCallback(() => {
    setFocusMode(true);
    const el = document.documentElement;
    const lockEscape = () => (navigator as Navigator & { keyboard?: { lock?: (keys: string[]) => Promise<void> } })
      .keyboard?.lock?.(['Escape']).catch(() => undefined);
    if (el.requestFullscreen) el.requestFullscreen().then(lockEscape).catch(() => { /* denied/unsupported */ });
  }, []);

  const exitFocus = useCallback(() => {
    setFocusMode(false);
    if (document.fullscreenElement && document.exitFullscreen) {
      document.exitFullscreen().catch(() => { /* ignore */ });
    }
  }, []);

  useEffect(() => {
    const onFsChange = () => { if (!document.fullscreenElement) setFocusMode(false); };
    document.addEventListener('fullscreenchange', onFsChange);
    return () => document.removeEventListener('fullscreenchange', onFsChange);
  }, []);

  useEffect(() => {
    if (!focusMode) return undefined;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !(e.target instanceof Element && e.target.closest('.xterm'))) exitFocus();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [focusMode, exitFocus]);

  return { focusMode, enterFocus, exitFocus };
}
