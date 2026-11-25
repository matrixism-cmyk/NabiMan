/**
 * A link to one file on a server: `/remote#file/<server>/<path>`, where
 * `<server>` is a saved server's id or `local` and the path keeps its
 * slashes. It names the file and nothing else — no key, no token — so opening
 * it means signing in, and the file is read through the same requests, with
 * the same checks, as the file panel.
 */
export interface FileLink {
  /** A saved server's id, or `local`. */
  scope: string;
  /** The file's absolute path. */
  path: string;
}

const PREFIX = '#file/';
/** A link opened before signing in, kept for the tab until the sign-in is done. */
const PENDING_KEY = 'nabiman_file_link';

export function fileLinkUrl(link: FileLink, origin = window.location.origin): string {
  const path = link.path.split('/').map(encodeURIComponent).join('/');
  return `${origin}/remote${PREFIX}${encodeURIComponent(link.scope)}${path}`;
}

export function parseFileLink(pathname: string, hash: string): FileLink | null {
  if (!/^\/remote\/?$/.test(pathname) || !hash.startsWith(PREFIX)) return null;
  const rest = hash.slice(PREFIX.length);
  const slash = rest.indexOf('/');
  if (slash <= 0) return null;
  try {
    const scope = decodeURIComponent(rest.slice(0, slash));
    const path = rest.slice(slash).split('/').map(decodeURIComponent).join('/');
    // The path is handed to the server as it is; it only has to look like one here.
    if (!/^[\w.-]{1,128}$/.test(scope) || path.length < 2 || path.length > 4096 || path.includes('\0')) return null;
    return { scope, path };
  } catch {
    return null; // not percent-encoding
  }
}

const session = () => { try { return window.sessionStorage; } catch { return null; } };

/**
 * The file this page was opened for, if any. A sign-in that leaves the page
 * and comes back (single sign-on) loses the address, so a link seen before
 * signing in is remembered and the address put back.
 */
export function openedFileLink(signedIn: boolean): FileLink | null {
  const here = parseFileLink(window.location.pathname, window.location.hash);
  if (!signedIn) {
    if (here) session()?.setItem(PENDING_KEY, window.location.hash);
    return null;
  }
  const pending = session()?.getItem(PENDING_KEY) || '';
  session()?.removeItem(PENDING_KEY);
  if (here) return here;
  const kept = parseFileLink('/remote', pending);
  if (kept) window.history.replaceState(null, '', `/remote${pending}`);
  return kept;
}
