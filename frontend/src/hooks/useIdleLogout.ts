import { useCallback, useEffect, useRef, useState } from 'react';
import { apiRequest } from './useApi';

/** When someone last did something here, shared by every tab of this browser. */
const LAST = 'nabiman_last_activity';
/** What counts as use. The page's own polling does not; speech does (see markActivity). */
const EVENTS = ['pointerdown', 'keydown', 'wheel', 'touchstart'] as const;
/** The server hears of use at most this often (it allows more slack than this). */
const REPORT_EVERY_MS = 60_000;
/** Use is written down at most this often (keys in a terminal come fast). */
const WRITE_EVERY_MS = 5_000;
const WARN_MS = 60_000;

/** Something a person did that no click or key shows: dictating, listening to an answer. */
export function markActivity() {
  window.dispatchEvent(new Event('nabiman_activity'));
}

const readLast = () => { try { return Number(localStorage.getItem(LAST)) || 0; } catch { return 0; } };

/**
 * Signs out after the idle time set in 환경설정 (none by default). A minute
 * before, it warns; any use puts it off. Use in one tab counts for the
 * others, which tell the server too, so none of them is signed out while
 * NabiMan is in use somewhere in this browser.
 */
export function useIdleLogout(active: boolean, signOut: () => void) {
  const [minutes, setMinutes] = useState(0);
  const [secondsLeft, setSecondsLeft] = useState<number | null>(null);
  const signOutRef = useRef(signOut);
  signOutRef.current = signOut;
  const reported = useRef(0);
  const written = useRef(0);

  useEffect(() => {
    if (!active) { setMinutes(0); return undefined; }
    const load = () => apiRequest<{ idle_logout_minutes: number }>('/api/app-settings')
      .then((r) => { if (r.success && r.data) setMinutes(r.data.idle_logout_minutes); })
      .catch(() => undefined);
    load();
    window.addEventListener('nabiman_app_settings_changed', load);
    return () => window.removeEventListener('nabiman_app_settings_changed', load);
  }, [active]);

  const report = useCallback(() => {
    const now = Date.now();
    if (now - reported.current < REPORT_EVERY_MS) return;
    reported.current = now;
    apiRequest('/api/auth/activity', { method: 'POST', body: '{}' }).catch(() => undefined);
  }, []);

  const used = useCallback((force = false) => {
    const now = Date.now();
    setSecondsLeft(null);
    if (force || now - written.current >= WRITE_EVERY_MS) {
      written.current = now;
      try { localStorage.setItem(LAST, String(now)); } catch { /* private mode: this tab alone */ }
    }
    report();
  }, [report]);

  useEffect(() => {
    if (!active || !minutes) { setSecondsLeft(null); return undefined; }
    // Signing in, or the limit being turned on, counts as use.
    used(true);
    const onUse = () => used();
    const onOtherTab = (e: StorageEvent) => { if (e.key === LAST) { setSecondsLeft(null); report(); } };
    EVENTS.forEach((name) => window.addEventListener(name, onUse, { capture: true, passive: true }));
    window.addEventListener('nabiman_activity', onUse);
    window.addEventListener('storage', onOtherTab);
    const tick = window.setInterval(() => {
      const left = minutes * 60_000 - (Date.now() - Math.max(readLast(), written.current));
      if (left <= 0) {
        try { sessionStorage.setItem('nabiman_idle_logout', '1'); } catch { /* shown or not */ }
        signOutRef.current();
      } else setSecondsLeft(left <= WARN_MS ? Math.ceil(left / 1000) : null);
    }, 1000);
    return () => {
      EVENTS.forEach((name) => window.removeEventListener(name, onUse, { capture: true }));
      window.removeEventListener('nabiman_activity', onUse);
      window.removeEventListener('storage', onOtherTab);
      window.clearInterval(tick);
    };
  }, [active, minutes, used, report]);

  return { minutes, secondsLeft, stay: () => used(true) };
}
