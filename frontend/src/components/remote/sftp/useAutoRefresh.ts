import { useCallback, useEffect, useRef, useState } from 'react';

const read = (key: string) => { try { return localStorage.getItem(key); } catch { return null; } };
const write = (key: string, v: string) => { try { localStorage.setItem(key, v); } catch { /* private mode */ } };

/**
 * Background monitoring: while the tab is visible and nothing blocks it, run
 * `tick` on an interval and also the moment the tab or window regains focus,
 * so changes made elsewhere show up without a manual refresh. `blocked`
 * returns true to skip a run (e.g. mid-edit). On/off is remembered per
 * browser under `key`, defaulting to on.
 */
export function useAutoRefresh(tick: () => void, blocked: () => boolean, key: string, ms = 7000) {
  const [on, setOn] = useState(() => read(key) !== '0');
  const tickRef = useRef(tick); tickRef.current = tick;
  const blockedRef = useRef(blocked); blockedRef.current = blocked;

  const run = useCallback(() => {
    if (document.visibilityState === 'visible' && !blockedRef.current()) tickRef.current();
  }, []);

  useEffect(() => {
    if (!on) return undefined;
    const id = window.setInterval(run, ms);
    const onVisible = () => { if (document.visibilityState === 'visible') run(); };
    document.addEventListener('visibilitychange', onVisible);
    window.addEventListener('focus', run);
    return () => {
      window.clearInterval(id);
      document.removeEventListener('visibilitychange', onVisible);
      window.removeEventListener('focus', run);
    };
  }, [on, run, ms]);

  const toggle = useCallback(() => setOn((v) => { write(key, v ? '0' : '1'); return !v; }), [key]);
  return { on, toggle };
}
