import { useEffect, useRef } from 'react';

/**
 * Polling that rests while nobody can see it. `tick` runs every `ms` while the
 * tab is visible; hiding the tab stops the timer, and showing it again runs
 * `tick` at once (the screen is likely stale) and restarts the timer. The
 * first run is left to the caller, who usually fetches on mount anyway.
 * Returns the function that stops it.
 */
export function startVisibleInterval(tick: () => void, ms: number): () => void {
  let id: ReturnType<typeof setInterval> | null = null;
  const start = () => { if (id === null) id = setInterval(tick, ms); };
  const stop = () => { if (id !== null) { clearInterval(id); id = null; } };
  const onVisibility = () => {
    if (document.hidden) { stop(); return; }
    if (id === null) { tick(); start(); }
  };
  if (!document.hidden) start();
  document.addEventListener('visibilitychange', onVisibility);
  return () => { stop(); document.removeEventListener('visibilitychange', onVisibility); };
}

/**
 * The hook form: `ms` falsy (0, null, undefined) means no polling. The latest
 * `tick` is always the one called, so callers need not memoise it.
 */
export function useVisibleInterval(tick: () => void, ms: number | null | undefined | false) {
  const tickRef = useRef(tick);
  tickRef.current = tick;
  useEffect(() => {
    if (!ms) return undefined;
    return startVisibleInterval(() => tickRef.current(), ms);
  }, [ms]);
}
