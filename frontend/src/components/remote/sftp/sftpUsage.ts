import { fetchWithRefresh } from '../../../shared/api/useApi';
import { API_BASE, SftpEntry, SftpSource } from './sftpApi';

/** What the server counted under the chosen entries. */
export interface UsageCount {
  dirs: number;
  files: number;
  links: number;
  other: number;
  /** The files' sizes added up. */
  bytes: number;
  /** Folders that could not be read. */
  unreadable: number;
}

export interface UsageItem extends SftpEntry {
  path: string;
  /** Where a link points. */
  target?: string;
}

/** What is directly in one chosen folder. */
export interface UsageInside { dirs: number; files: number; links: number; other: number; hidden: number }

/** A line of the answer: first the items, then running counts, then the last count with `done`. */
export interface UsageLine {
  items?: UsageItem[];
  inside?: UsageInside | null;
  count?: UsageCount;
  done?: boolean;
  /** What stopped the count early: `time`, `entries`, or `lost` (the connection). */
  capped?: 'time' | 'entries' | 'lost' | null;
  /** `local`, `find`, `walk` or `none`. */
  how?: string;
  /** A checksum's progress and answer. */
  read?: number;
  size?: number;
  sha256?: string;
  error?: string;
}

/**
 * POSTs `body` and hands each JSON line of the answer to `onLine` as it
 * arrives. Resolves with an error message, or '' when the answer ended.
 */
export async function streamLines(src: SftpSource, path: string, body: unknown,
  onLine: (line: UsageLine) => void, signal: AbortSignal): Promise<string> {
  const init: RequestInit = { method: 'POST', cache: 'no-store', signal, body: JSON.stringify(body) };
  const url = `${API_BASE}${src.base}${path}`;
  // An account's token is renewed as for any request; a visitor sends the link's key.
  const res = src.shareKey
    ? await fetch(url, { ...init, credentials: 'omit', headers: { 'Content-Type': 'application/json', 'X-Share-Files': src.shareKey } })
    : await fetchWithRefresh(url, { ...init, headers: { 'Content-Type': 'application/json' } });
  if (!res.ok || !res.body) {
    try { return (await res.json()).message || `HTTP ${res.status}`; } catch { return `HTTP ${res.status}`; }
  }
  const reader = res.body.getReader();
  const dec = new TextDecoder();
  let rest = '';
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    rest += dec.decode(value, { stream: true });
    const lines = rest.split('\n');
    rest = lines.pop() ?? '';
    for (const l of lines) if (l.trim()) onLine(JSON.parse(l));
  }
  if (rest.trim()) onLine(JSON.parse(rest));
  return '';
}

export const usage = (src: SftpSource, paths: string[], onLine: (l: UsageLine) => void, signal: AbortSignal) =>
  streamLines(src, '/usage', { paths }, onLine, signal);

export const checksum = (src: SftpSource, path: string, onLine: (l: UsageLine) => void, signal: AbortSignal) =>
  streamLines(src, '/checksum', { path }, onLine, signal);
