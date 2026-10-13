/**
 * The SFTP side of a saved server, as the browser sees it.
 *
 * Small requests go through the shared API helpers. Transfers use
 * XMLHttpRequest instead of fetch because only it reports upload progress,
 * and a progress bar is most of what makes a large upload bearable.
 */
import { apiRequest } from '../../../hooks/useApi';

export interface SftpEntry {
  name: string;
  kind: 'dir' | 'file' | 'link' | 'other';
  /** A link that leads to a folder. */
  to_dir: boolean;
  size: number;
  mtime: number | null;
  mode: number | null;
}

export interface SftpListing {
  path: string;
  parent: string | null;
  entries: SftpEntry[];
  truncated: boolean;
}

const API_BASE = process.env.REACT_APP_API_URL || '';
const token = () => sessionStorage.getItem('nabiman_token') || '';

/**
 * Where a panel's files come from, and how it signs its requests: a signed-in
 * account (a saved server or the local shell), or the visitor of a share link,
 * who has no account and carries the link's file key instead.
 */
export interface SftpSource {
  /** Route prefix: `/api/terminal/sftp/<id>` or `/api/share/<token>/files`. */
  base: string;
  /** Names what the panel remembers (the last folder) for this source. */
  key: string;
  /** False when a share link allows browsing and downloading only. */
  writable: boolean;
  /** A share visitor's file key; absent for an account. */
  shareKey?: string;
}

export const accountSource = (scope: string): SftpSource =>
  ({ base: `/api/terminal/sftp/${encodeURIComponent(scope)}`, key: scope, writable: true });

export const shareSource = (shareToken: string, shareKey: string, writable: boolean): SftpSource =>
  ({ base: `/api/share/${encodeURIComponent(shareToken)}/files`, key: `share_${shareToken.slice(0, 16)}`, writable, shareKey });

interface Reply<T> { success: boolean; data: T | null; message: string }

/** Accounts go through the shared helper (token refresh); visitors send their key only. */
async function request<T>(src: SftpSource, path: string, init: RequestInit = {}): Promise<Reply<T>> {
  if (!src.shareKey) return apiRequest<T>(`${src.base}${path}`, init) as Promise<Reply<T>>;
  const res = await fetch(`${API_BASE}${src.base}${path}`, {
    ...init, credentials: 'omit', cache: 'no-store',
    headers: { 'Content-Type': 'application/json', 'X-Share-Files': src.shareKey },
  });
  try {
    return await res.json();
  } catch {
    return { success: false, data: null, message: `HTTP ${res.status}` };
  }
}

const authHeader = (src: SftpSource): [string, string] =>
  (src.shareKey ? ['X-Share-Files', src.shareKey] : ['Authorization', `Bearer ${token()}`]);

export const joinPath = (dir: string, name: string) => (dir.endsWith('/') ? `${dir}${name}` : `${dir}/${name}`);
export const baseName = (path: string) => path.replace(/\/+$/, '').split('/').pop() || path;
export const isFolder = (e: SftpEntry) => e.kind === 'dir' || e.to_dir;

/** A path quoted for a POSIX shell, ready to paste as one argument. */
export function shellQuote(path: string): string {
  return /^[\w@%+=:,./-]+$/.test(path) ? path : `'${path.replace(/'/g, `'\\''`)}'`;
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

export function listDir(src: SftpSource, path: string) {
  return request<SftpListing>(src, `/list?path=${encodeURIComponent(path)}`);
}

export function makeDir(src: SftpSource, path: string) {
  return request<string>(src, '/mkdir', { method: 'POST', body: JSON.stringify({ path }) });
}

export function movePath(src: SftpSource, from: string, to: string) {
  return request<string>(src, '/rename', { method: 'POST', body: JSON.stringify({ from, to }) });
}

export function removePath(src: SftpSource, path: string) {
  return request<number>(src, '/delete', { method: 'POST', body: JSON.stringify({ path }) });
}

/** A single-use URL the browser can fetch on its own, for a drag onto the desktop. */
export async function downloadLink(src: SftpSource, path: string): Promise<string | null> {
  const res = await request<{ url: string }>(src, '/link', { method: 'POST', body: JSON.stringify({ path }) });
  return res.success && res.data?.url ? new URL(`${API_BASE}${res.data.url}`, window.location.href).href : null;
}

export interface TransferResult {
  ok: boolean;
  /** The upload stopped because something by that name is already there. */
  exists?: boolean;
  aborted?: boolean;
  message?: string;
  blob?: Blob;
  status?: number;
  /** Where the server's part file ends, when a piece arrived out of step. */
  size?: number;
}

export interface Transfer {
  promise: Promise<TransferResult>;
  abort: () => void;
}

function send(
  src: SftpSource, method: 'GET' | 'POST', url: string, body: Blob | null,
  onProgress: (loaded: number, total: number) => void,
): Transfer {
  const xhr = new XMLHttpRequest();
  const promise = new Promise<TransferResult>((resolve) => {
    xhr.open(method, `${API_BASE}${src.base}${url}`);
    xhr.setRequestHeader(...authHeader(src));
    if (method === 'GET') xhr.responseType = 'blob';
    else xhr.setRequestHeader('Content-Type', 'application/octet-stream');
    const progress = (e: ProgressEvent) => onProgress(e.loaded, e.lengthComputable ? e.total : 0);
    if (method === 'GET') xhr.onprogress = progress;
    else xhr.upload.onprogress = progress;
    xhr.onload = async () => {
      if (method === 'GET' && xhr.status === 200) {
        resolve({ ok: true, blob: xhr.response as Blob });
        return;
      }
      let json: { success?: boolean; message?: string; data?: { exists?: boolean; size?: number } } = {};
      try {
        const text = method === 'GET' ? await (xhr.response as Blob).text() : xhr.responseText;
        json = JSON.parse(text);
      } catch { /* not JSON: the status says enough */ }
      resolve({
        ok: xhr.status === 200 && json.success !== false,
        exists: xhr.status === 409 && Boolean(json.data?.exists),
        status: xhr.status,
        size: typeof json.data?.size === 'number' ? json.data.size : undefined,
        message: json.message || (xhr.status === 401 ? 'Sign in again' : `HTTP ${xhr.status}`),
      });
    };
    xhr.onerror = () => resolve({ ok: false, message: 'Network error' });
    xhr.onabort = () => resolve({ ok: false, aborted: true });
    xhr.send(body);
  });
  return { promise, abort: () => xhr.abort() };
}

/** Files above this go up in pieces of this size. */
export const PART_SIZE = 32 * 1024 * 1024;
/** Tries per piece before the upload gives up. */
const PART_TRIES = 4;

export function uploadFile(
  src: SftpSource, dir: string, name: string, file: Blob, overwrite: boolean,
  onProgress: (loaded: number, total: number) => void,
): Transfer {
  if (file.size > PART_SIZE) return uploadInParts(src, dir, name, file, overwrite, onProgress);
  const query = `dir=${encodeURIComponent(dir)}&name=${encodeURIComponent(name)}&overwrite=${overwrite}`;
  return send(src, 'POST', `/upload?${query}`, file, onProgress);
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function uploadId(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/**
 * A large file in pieces, so one dropped connection costs one piece rather
 * than gigabytes, and no proxy has to hold the whole file before passing it
 * on. A piece that fails is retried; if the server's part file ends somewhere
 * else (a piece arrived but its answer was lost), the upload continues from
 * there. Only the last step gives the file its name.
 */
export function uploadInParts(
  src: SftpSource, dir: string, name: string, file: Blob, overwrite: boolean,
  onProgress: (loaded: number, total: number) => void,
): Transfer {
  const id = uploadId();
  const query = (extra: string) =>
    `dir=${encodeURIComponent(dir)}&name=${encodeURIComponent(name)}&id=${id}&overwrite=${overwrite}&${extra}`;
  let current: Transfer | null = null;
  let stopped = false;
  const discard = () => request(src, `/upload-abort?${query('offset=0')}`, { method: 'POST' }).catch(() => undefined);

  const promise = (async (): Promise<TransferResult> => {
    let offset = 0;
    let tries = 0;
    while (offset < file.size) {
      if (stopped) { discard(); return { ok: false, aborted: true }; }
      const end = Math.min(file.size, offset + PART_SIZE);
      const start = offset;
      current = send(src, 'POST', `/upload-part?${query(`offset=${start}`)}`, file.slice(start, end),
        (loaded) => onProgress(start + loaded, file.size));
      const result = await current.promise;
      if (result.aborted) { discard(); return result; }
      if (result.exists) return result;
      if (result.ok) { offset = end; tries = 0; continue; }
      if (result.status === 409 && result.size !== undefined) { offset = result.size; continue; }
      const final = result.status !== undefined && [400, 401, 403, 404].includes(result.status);
      if (final || ++tries >= PART_TRIES) { discard(); return result; }
      await sleep(1000 * tries);
    }
    const done = await request<{ size: number }>(src, `/upload-finish?${query(`size=${file.size}`)}`, { method: 'POST' });
    if (done.success) return { ok: true };
    const exists = Boolean((done.data as { exists?: boolean } | null)?.exists);
    discard();
    return { ok: false, exists, message: done.message };
  })();
  return { promise, abort: () => { stopped = true; current?.abort(); } };
}

export function downloadFile(
  src: SftpSource, path: string, onProgress: (loaded: number, total: number) => void,
): Transfer {
  return send(src, 'GET', `/download?path=${encodeURIComponent(path)}`, null, onProgress);
}

/**
 * Saves a file through the browser's own downloader, from a single-use link:
 * it streams to disk with the browser's progress and pause, whatever the size,
 * instead of being held in the page's memory first.
 */
export async function saveFile(src: SftpSource, path: string): Promise<boolean> {
  const url = await downloadLink(src, path);
  if (!url) return false;
  const link = document.createElement('a');
  link.href = url;
  link.download = baseName(path);
  link.rel = 'noopener';
  document.body.appendChild(link);
  link.click();
  link.remove();
  return true;
}
