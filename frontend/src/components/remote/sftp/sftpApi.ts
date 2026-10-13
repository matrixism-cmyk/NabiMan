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
      let json: { success?: boolean; message?: string; data?: { exists?: boolean } } = {};
      try {
        const text = method === 'GET' ? await (xhr.response as Blob).text() : xhr.responseText;
        json = JSON.parse(text);
      } catch { /* not JSON: the status says enough */ }
      resolve({
        ok: xhr.status === 200 && json.success !== false,
        exists: xhr.status === 409 && Boolean(json.data?.exists),
        message: json.message || (xhr.status === 401 ? 'Sign in again' : `HTTP ${xhr.status}`),
      });
    };
    xhr.onerror = () => resolve({ ok: false, message: 'Network error' });
    xhr.onabort = () => resolve({ ok: false, aborted: true });
    xhr.send(body);
  });
  return { promise, abort: () => xhr.abort() };
}

export function uploadFile(
  src: SftpSource, dir: string, name: string, file: Blob, overwrite: boolean,
  onProgress: (loaded: number, total: number) => void,
): Transfer {
  const query = `dir=${encodeURIComponent(dir)}&name=${encodeURIComponent(name)}&overwrite=${overwrite}`;
  return send(src, 'POST', `/upload?${query}`, file, onProgress);
}

export function downloadFile(
  src: SftpSource, path: string, onProgress: (loaded: number, total: number) => void,
): Transfer {
  return send(src, 'GET', `/download?path=${encodeURIComponent(path)}`, null, onProgress);
}

/** Hands a downloaded file to the browser's own save flow. */
export function saveBlob(name: string, blob: Blob) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  document.body.appendChild(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}
