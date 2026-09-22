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
const fontKey = (key: string) => `nabiman_term_font_${key}`;
/** The zoom's storage key, for tests that check what survives a prune. */
export const fontKeyForTest = fontKey;

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
    localStorage.removeItem(fontKey(storageKey));
  } catch { /* ignore */ }
}

/**
 * The zoom level a terminal was left at, kept under the session's key once one
 * is known (see sessionZoomKey). Ctrl+wheel therefore outlives switching to
 * another session and coming back, a reload, and reopening the same session
 * somewhere else.
 */
export function readFontSize(storageKey: string): number | null {
  const raw = readStored(fontKey(storageKey));
  const size = parseInt(raw, 10);
  return size >= 8 && size <= 28 ? size : null;
}

export function writeFontSize(storageKey: string, size: number) {
  writeStored(fontKey(storageKey), String(size));
}

/** Forget the zoom, so the size from settings applies again. */
export function forgetFontSize(storageKey: string) {
  try { localStorage.removeItem(fontKey(storageKey)); } catch { /* ignore */ }
}

/** The prefix every floating window's id carries (see TerminalWindows). */
const WINDOW_PREFIX = 'tw_';

/** Everything stored about one pane, whatever kind of entry it is. */
function suffixOf(key: string): string | null {
  for (const prefix of [bufKey(''), sidKey(''), fontKey('')]) {
    if (key.startsWith(prefix)) return key.slice(prefix.length);
  }
  return null;
}

/**
 * Drop stored screens, session ids and zooms left behind by floating windows
 * that are gone — one closed in another tab, say.
 *
 * Only window panes are pruned. The workspace panes (`ws_*`), the docked
 * panel (`dock_*`) and share viewers (`share_*`) have no window row to match
 * against, and used to be wiped on every page load: a reload lost the zoom,
 * the restored screen and the session id, so the pane started a fresh shell
 * instead of re-attaching to the one still running.
 */
export function pruneTerminalStorage(liveKeys: string[]) {
  try {
    const keep = new Set(liveKeys);
    const stale: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (!key) continue;
      const suffix = suffixOf(key);
      if (suffix === null || !suffix.startsWith(WINDOW_PREFIX) || keep.has(suffix)) continue;
      stale.push(key);
    }
    stale.forEach((key) => localStorage.removeItem(key));
  } catch { /* ignore */ }
}

/**
 * Where a session's zoom is kept. Zoom follows the shell rather than the pane
 * showing it, so a session looks the same whether it is joined in the
 * workspace or opened in a window of its own.
 */
export const sessionZoomKey = (sessionId: string) => `sess_${sessionId}`;

/** Forget the zoom of sessions that are no longer running. */
export function pruneSessionZooms(liveSessionIds: string[]) {
  try {
    const keep = new Set(liveSessionIds.map(sessionZoomKey));
    const stale: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (!key || !key.startsWith(fontKey(''))) continue;
      const suffix = key.slice(fontKey('').length);
      if (!suffix.startsWith('sess_') || keep.has(suffix)) continue;
      stale.push(key);
    }
    stale.forEach((key) => localStorage.removeItem(key));
  } catch { /* ignore */ }
}

/** The session id a pane last attached to, if any. */
export function storedSessionId(storageKey: string): string {
  return readStored(sidKey(storageKey));
}
