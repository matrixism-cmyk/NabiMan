import { useEffect } from 'react';

// Minimal Screen Wake Lock typing (lib.dom may lack it on older TS targets).
interface WakeLockSentinelLike { release: () => Promise<void>; }
interface WakeLockNav { wakeLock?: { request: (type: 'screen') => Promise<WakeLockSentinelLike> }; }

/**
 * Keeps the display awake while `enabled` (for an unattended NOC wall) and
 * re-acquires the lock after the tab is hidden/restored — the documented
 * footgun where the sentinel is silently released on visibility change.
 * Requires a secure context (HTTPS); failures degrade to a sleeping display.
 */
export function useWakeLock(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    const nav = navigator as unknown as WakeLockNav;
    if (!nav.wakeLock) return;
    let sentinel: WakeLockSentinelLike | null = null;
    let cancelled = false;

    const acquire = async () => {
      try {
        sentinel = await nav.wakeLock!.request('screen');
      } catch { /* denied / not focused — ignore */ }
    };
    const onVisible = () => {
      if (!cancelled && document.visibilityState === 'visible') acquire();
    };

    acquire();
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      cancelled = true;
      document.removeEventListener('visibilitychange', onVisible);
      sentinel?.release().catch(() => {});
    };
  }, [enabled]);
}
