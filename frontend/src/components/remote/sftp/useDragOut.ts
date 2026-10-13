import { useCallback, useEffect, useRef } from 'react';
import { downloadLink, SftpSource } from './sftpApi';

/** Links live five minutes on the server; one this old is not offered. */
const FRESH_MS = 4 * 60 * 1000;
/** How long the pointer rests on a row before its link is fetched. */
const DWELL_MS = 250;

/**
 * Dragging a file out of the browser onto the desktop saves it — in Chromium
 * browsers, through a `DownloadURL` set when the drag starts. That has to
 * happen synchronously and the browser fetches the URL without our sign-in
 * header, so a single-use link is fetched ahead of time: when the pointer
 * rests on a file row, or presses it.
 */
export function useDragOut(source: SftpSource) {
  const cache = useRef(new Map<string, { url: string; at: number }>());
  const pending = useRef(new Set<string>());
  const timer = useRef<number | undefined>(undefined);

  const fetchLink = useCallback(async (path: string) => {
    const hit = cache.current.get(path);
    if ((hit && Date.now() - hit.at < FRESH_MS) || pending.current.has(path)) return;
    pending.current.add(path);
    try {
      const url = await downloadLink(source, path);
      if (url) cache.current.set(path, { url, at: Date.now() });
    } catch {
      /* no link: the drag still works inside the page */
    } finally {
      pending.current.delete(path);
    }
  }, [source]);

  const prefetch = useCallback((path: string, now: boolean) => {
    window.clearTimeout(timer.current);
    if (now) fetchLink(path);
    else timer.current = window.setTimeout(() => fetchLink(path), DWELL_MS);
  }, [fetchLink]);

  /** Adds the desktop download to a starting drag, when a link is ready. */
  const attach = useCallback((dt: DataTransfer, path: string, name: string) => {
    const hit = cache.current.get(path);
    if (!hit || Date.now() - hit.at >= FRESH_MS) return false;
    // The format is type:name:url, so a colon in the name would cut it short.
    dt.setData('DownloadURL', `application/octet-stream:${name.replace(/:/g, '_')}:${hit.url}`);
    return true;
  }, []);

  /** The drop may have used the link; the next drag gets a fresh one. */
  const spent = useCallback((path: string) => { cache.current.delete(path); }, []);

  useEffect(() => () => window.clearTimeout(timer.current), []);
  useEffect(() => { cache.current.clear(); }, [source]);

  return { prefetch, attach, spent };
}
