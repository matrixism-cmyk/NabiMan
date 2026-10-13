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
const base = (serverId: string) => `/api/terminal/sftp/${encodeURIComponent(serverId)}`;
const token = () => sessionStorage.getItem('nabiman_token') || '';

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

export function listDir(serverId: string, path: string) {
  return apiRequest<SftpListing>(`${base(serverId)}/list?path=${encodeURIComponent(path)}`);
}

export function makeDir(serverId: string, path: string) {
  return apiRequest<string>(`${base(serverId)}/mkdir`, { method: 'POST', body: JSON.stringify({ path }) });
}

export function movePath(serverId: string, from: string, to: string) {
  return apiRequest<string>(`${base(serverId)}/rename`, { method: 'POST', body: JSON.stringify({ from, to }) });
}

export function removePath(serverId: string, path: string) {
  return apiRequest<number>(`${base(serverId)}/delete`, { method: 'POST', body: JSON.stringify({ path }) });
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
  method: 'GET' | 'POST', url: string, body: Blob | null,
  onProgress: (loaded: number, total: number) => void,
): Transfer {
  const xhr = new XMLHttpRequest();
  const promise = new Promise<TransferResult>((resolve) => {
    xhr.open(method, `${API_BASE}${url}`);
    xhr.setRequestHeader('Authorization', `Bearer ${token()}`);
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
  serverId: string, dir: string, name: string, file: Blob, overwrite: boolean,
  onProgress: (loaded: number, total: number) => void,
): Transfer {
  const query = `dir=${encodeURIComponent(dir)}&name=${encodeURIComponent(name)}&overwrite=${overwrite}`;
  return send('POST', `${base(serverId)}/upload?${query}`, file, onProgress);
}

export function downloadFile(
  serverId: string, path: string, onProgress: (loaded: number, total: number) => void,
): Transfer {
  return send('GET', `${base(serverId)}/download?path=${encodeURIComponent(path)}`, null, onProgress);
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
