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
  // A pane closing after a sign-out must not write its screen back: an
  // account's screens are kept only while it is signed in (a share link's
  // visitor has no account, and keeps theirs).
  if (key.startsWith(bufKey('')) && !key.startsWith(bufKey('share_')) && !signedIn()) return;
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

/**
 * Kept instead of a session id once that session has ended (exited, or
 * ended from the session list). The pane then stays ended across reloads and
 * sign-ins, and opens a new shell only when asked to ("새 세션"): restoring it
 * used to start a fresh shell every time, and a few forgotten windows added
 * a few unwanted shells at every sign-in.
 */
export const ENDED_SID = '!ended';

/** The session id a pane last attached to, if any. */
export function storedSessionId(storageKey: string): string {
  const sid = readStored(sidKey(storageKey));
  return sid === ENDED_SID ? '' : sid;
}

export function markEnded(storageKey: string) {
  writeStored(sidKey(storageKey), ENDED_SID);
}

/**
 * Whether a pane being restored should stay ended: its session ended, or
 * (from before the marker existed) it shows an old screen but no session.
 */
export function paneEnded(storageKey: string): boolean {
  const sid = readStored(sidKey(storageKey));
  return sid === ENDED_SID || (sid === '' && readStored(bufKey(storageKey)) !== '');
}

/** The pane's next connection starts a new shell, with a clean screen. */
export function startFresh(storageKey: string) {
  try { localStorage.removeItem(bufKey(storageKey)); } catch { /* ignore */ }
  writeStored(sidKey(storageKey), '');
}

const OWNER_KEY = 'nabiman_term_owner';

/** The account signed in in this tab, by its token's subject; null when none. */
function signedIn(): string | null {
  try {
    const token = sessionStorage.getItem('nabiman_token');
    const part = token?.split('.')[1];
    if (!part) return null;
    const json = atob(part.replace(/-/g, '+').replace(/_/g, '/').padEnd(Math.ceil(part.length / 4) * 4, '='));
    const sub = JSON.parse(json).sub;
    return typeof sub === 'string' && sub ? sub : null;
  } catch { return null; }
}

/** Every pane's stored screen and session, of whoever used this browser. */
export function forgetAllScreens() {
  try {
    const gone: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k && (k.startsWith(bufKey('')) || k.startsWith(sidKey('')))) gone.push(k);
    }
    gone.forEach((k) => localStorage.removeItem(k));
  } catch { /* storage blocked: nothing was kept either */ }
}

/**
 * Stored screens are an account's own: what a terminal printed (passwords
 * and keys included) must not greet the next person at this browser. On
 * signing out — by hand, for being idle or on expiry — they go; on signing
 * in as someone else than whose they are, they go too. The same account
 * signing in again (or in a new tab) finds its screens as it left them.
 */
export function guardScreens(event?: Event) {
  const who = signedIn();
  if (!who) {
    // Only a sign-out clears; a page that has not signed in yet knows nobody.
    if (event) forgetAllScreens();
    return;
  }
  try {
    if (localStorage.getItem(OWNER_KEY) !== who) {
      forgetAllScreens();
      localStorage.setItem(OWNER_KEY, who);
    }
  } catch { /* storage blocked */ }
}

if (typeof window !== 'undefined') {
  window.addEventListener('nabiman_auth_changed', guardScreens);
  guardScreens();
}
