/** Anonymous share transport never sends or refreshes the control-plane JWT. */
export type ShareFilesMode = 'none' | 'read' | 'write';
export interface ShareInfo {
  label: string; needs_password: boolean; read_only: boolean; expires_unix: number;
  alive: boolean; cols: number; rows: number;
  /** File panel access the link grants; older servers do not say, meaning none. */
  files: ShareFilesMode;
  /** `files` for a link to a server's files alone, with no terminal. */
  kind: 'terminal' | 'files';
  /** The visitor may dictate into the terminal (it is set up, the link is not read-only). */
  dictation?: boolean;
  /** How dictation ends what was said there (the server's setting): the pause, and Enter at it. */
  dictation_pause_ms?: number;
  dictation_auto_enter?: boolean;
}
/** The key a visitor's file panel signs its requests with. Memory only. */
export interface ShareFilesGrant { key: string; mode: 'read' | 'write' }
export interface ShareTicket { ticket: string; expires_in_seconds: number; files?: ShareFilesGrant }
export class ShareAdmissionError extends Error {
  constructor(public readonly translationKey: string) { super(translationKey); }
}
const API_BASE = process.env.REACT_APP_API_URL || '';
async function publicRequest<T>(path: string, init: RequestInit): Promise<T> {
  try {
    const response = await fetch(`${API_BASE}${path}`, { ...init, credentials: 'omit', cache: 'no-store', headers: { 'Content-Type': 'application/json' } });
    if (response.status === 429) throw new ShareAdmissionError('share.authLimited');
    if (response.status === 403 || response.status === 401) throw new ShareAdmissionError('share.authDenied');
    if (!response.ok) throw new ShareAdmissionError('share.authUnavailable');
    const body = await response.json();
    if (!body || body.success !== true) throw new ShareAdmissionError('share.authDenied');
    if (!body.data || typeof body.data !== 'object') throw new ShareAdmissionError('share.authUnavailable');
    if (init.signal?.aborted) throw new DOMException('Aborted', 'AbortError');
    return body.data as T;
  } catch (cause) {
    if (cause instanceof ShareAdmissionError || (cause instanceof Error && cause.name === 'AbortError')) throw cause;
    throw new ShareAdmissionError('share.authUnavailable');
  }
}
export async function readShareInfo(token: string, signal?: AbortSignal): Promise<ShareInfo> {
  const data = await publicRequest<ShareInfo>(`/api/share/${encodeURIComponent(token)}`, { signal });
  if (typeof data.label !== 'string' || typeof data.needs_password !== 'boolean' || typeof data.read_only !== 'boolean' || typeof data.alive !== 'boolean'
      || !Number.isSafeInteger(data.expires_unix) || data.expires_unix <= 0
      || ![data.cols, data.rows].every(n => Number.isInteger(n) && n >= 0 && n <= 4096)) throw new ShareAdmissionError('share.authUnavailable');
  const files = (data as { files?: unknown }).files;
  const kind = (data as { kind?: unknown }).kind === 'files' ? 'files' : 'terminal';
  const dictation = (data as { dictation?: unknown }).dictation === true;
  const pause = (data as { dictation_pause_ms?: unknown }).dictation_pause_ms;
  const dictation_pause_ms = Number.isInteger(pause) && (pause as number) >= 0 && (pause as number) <= 10000 ? pause as number : 1500;
  const dictation_auto_enter = (data as { dictation_auto_enter?: unknown }).dictation_auto_enter === true;
  return { ...data, kind, dictation, dictation_pause_ms, dictation_auto_enter, files: files === 'read' || files === 'write' ? files : 'none' };
}
export async function issueShareTicket(token: string, password: string, query: string, signal?: AbortSignal): Promise<ShareTicket> {
  const data = await publicRequest<ShareTicket>(`/api/share/${encodeURIComponent(token)}/auth`, {
    method: 'POST', body: JSON.stringify({ password, query }), signal,
  });
  if (typeof data.ticket !== 'string' || !/^[a-f0-9]{64}$/.test(data.ticket)
      || !Number.isInteger(data.expires_in_seconds) || data.expires_in_seconds <= 0 || data.expires_in_seconds > 30) throw new ShareAdmissionError('share.authUnavailable');
  const files = data.files;
  const grant = files && typeof files.key === 'string' && /^[a-f0-9]{64}$/.test(files.key)
    && (files.mode === 'read' || files.mode === 'write') ? { key: files.key, mode: files.mode } : undefined;
  return { ticket: data.ticket, expires_in_seconds: data.expires_in_seconds, files: grant };
}
