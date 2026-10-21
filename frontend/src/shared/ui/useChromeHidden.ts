import { useCallback, useEffect, useState } from 'react';

const KEY = 'nabiman_chrome_hidden';
const EVENT = 'nabiman_chrome_changed';

const read = () => { try { return localStorage.getItem(KEY) === '1'; } catch { return false; } };

/** Sets whether the logo and the menu bar are folded away, from anywhere (remote settings too). */
export function setChromeHidden(hidden: boolean) {
  try { localStorage.setItem(KEY, hidden ? '1' : '0'); } catch { /* private mode: this page only */ }
  window.dispatchEvent(new Event(EVENT));
}

/**
 * Whether the top of NabiMan — the logo and the menu bar — is folded away,
 * so the page below has the whole height. Remembered in this browser.
 */
export function useChromeHidden(): [boolean, (hidden: boolean) => void] {
  const [hidden, setHidden] = useState(read);
  useEffect(() => {
    const sync = () => setHidden(read());
    window.addEventListener(EVENT, sync);
    window.addEventListener('storage', sync);
    return () => { window.removeEventListener(EVENT, sync); window.removeEventListener('storage', sync); };
  }, []);
  return [hidden, useCallback((next: boolean) => setChromeHidden(next), [])];
}
