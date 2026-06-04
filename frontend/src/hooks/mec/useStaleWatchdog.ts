import { useEffect, useState } from 'react';

/**
 * Returns true when no fresh data has arrived within `thresholdMs` of
 * `lastUpdated`. Self-ticks every second so a silently dead poll/stream
 * visibly degrades (dim + badge) instead of showing stale values as live.
 */
export function useStaleWatchdog(lastUpdated: number | null, thresholdMs: number): boolean {
  const [stale, setStale] = useState(false);
  useEffect(() => {
    const check = () => {
      setStale(lastUpdated != null && Date.now() - lastUpdated > thresholdMs);
    };
    check();
    const id = setInterval(check, 1000);
    return () => clearInterval(id);
  }, [lastUpdated, thresholdMs]);
  return stale;
}
