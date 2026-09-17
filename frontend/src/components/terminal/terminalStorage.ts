/**
 * Where a terminal pane keeps its screen between visits.
 *
 * Each pane mirrors its output into localStorage, capped at the configured
 * scrollback, so a reload shows what was on screen even before the socket is
 * back. The data never leaves the browser it was written in.
 */

/** Raw output kept per window in localStorage so a reload still shows history.
 *  512 KB comfortably holds the 5,000-line default without crowding the
 *  browser's storage quota when many windows are open. */
export const MAX_BUFFER_CHARS = 512 * 1024;

export const bufKey = (key: string) => `nabiman_term_buf_${key}`;
export const sidKey = (key: string) => `nabiman_term_sid_${key}`;

export function readStored(key: string): string {
  try { return localStorage.getItem(key) || ''; } catch { return ''; }
}

export function writeStored(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Out of quota (many windows open): drop every other saved screen and try
    // once more, so at least the pane being written keeps its history.
    try {
      const stale: string[] = [];
      for (let i = 0; i < localStorage.length; i++) {
        const k = localStorage.key(i);
        if (k && k.startsWith(bufKey('')) && k !== key) stale.push(k);
      }
      stale.forEach((k) => localStorage.removeItem(k));
      localStorage.setItem(key, value);
    } catch { /* private mode or still too large — history is best effort */ }
  }
}

/** Remove the persisted screen + session id of a pane that is going away. */
export function forgetTerminalStorage(storageKey: string) {
  try {
    localStorage.removeItem(bufKey(storageKey));
    localStorage.removeItem(sidKey(storageKey));
  } catch { /* ignore */ }
}

/** Drop stored screens/session ids for panes that no longer exist. */
export function pruneTerminalStorage(liveKeys: string[]) {
  try {
    const keep = new Set(liveKeys);
    const stale: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (!key) continue;
      const suffix = key.startsWith(bufKey('')) ? key.slice(bufKey('').length)
        : key.startsWith(sidKey('')) ? key.slice(sidKey('').length)
        : null;
      // `dock_*` belongs to the docked terminal panel, which has no window row.
      if (suffix === null || suffix.startsWith('dock_') || keep.has(suffix)) continue;
      stale.push(key);
    }
    stale.forEach((key) => localStorage.removeItem(key));
  } catch { /* ignore */ }
}

/** The session id a pane last attached to, if any. */
export function storedSessionId(storageKey: string): string {
  return readStored(sidKey(storageKey));
}
