import { useState, useEffect, useCallback } from 'react';
import { useVisibleInterval } from '../ui/useVisibleInterval';

/** Every NabiMan API answers in this shape. */
export interface ApiResponse<T> {
  code?: string;
  success: boolean;
  data: T | null;
  message: string;
}

const API_BASE = process.env.REACT_APP_API_URL || '';

function getToken(): string | null { return sessionStorage.getItem('nabiman_token'); }
function getRefreshToken(): string | null { return sessionStorage.getItem('nabiman_refresh'); }

// Login/logout change this generation; an access-token renewal keeps it.
let sessionGeneration = 0;
let refreshRevision = 0;
let currentRefresh = getRefreshToken();
export function setToken(token: string) {
  sessionGeneration += 1;
  refreshRevision = 0;
  sessionStorage.removeItem('nabiman_refresh');
  currentRefresh = null;
  sessionStorage.setItem('nabiman_token', token);
  sessionStorage.removeItem('nabiman_reauth_required');
  sessionStorage.removeItem('nabiman_idle_logout');
  sessionStorage.removeItem('nabiman_security_changed');
  window.dispatchEvent(new Event('nabiman_auth_changed'));
}
export function setRefreshToken(token: string) {
  sessionGeneration += 1;
  refreshRevision = 0;
  sessionStorage.setItem('nabiman_refresh', token);
  currentRefresh = token;
}
export function clearToken() {
  sessionGeneration += 1;
  refreshRevision = 0;
  sessionStorage.removeItem('nabiman_token');
  sessionStorage.removeItem('nabiman_refresh');
  currentRefresh = null;
  window.dispatchEvent(new Event('nabiman_auth_changed'));
}
function snapshotSession() {
  return { generation: sessionGeneration, revision: refreshRevision, refresh: getRefreshToken(), access: getToken() };
}
type SessionSnapshot = ReturnType<typeof snapshotSession>;
function isCurrent(session: SessionSnapshot) {
  return session.generation === sessionGeneration && currentRefresh === getRefreshToken()
    && (session.refresh !== null || session.access === getToken());
}
function assertCurrent(session: SessionSnapshot) {
  if (!isCurrent(session)) throw new DOMException('Login session changed; request discarded', 'AbortError');
}
/** A body the browser types itself (multipart boundary, a file's bytes). */
function typesItself(body: RequestInit['body']): boolean {
  return (typeof FormData !== 'undefined' && body instanceof FormData)
    || (typeof Blob !== 'undefined' && body instanceof Blob)
    || (typeof URLSearchParams !== 'undefined' && body instanceof URLSearchParams);
}
function authHeaders(body?: RequestInit['body']): Record<string, string> {
  const token = getToken();
  const headers: Record<string, string> = typesItself(body) ? {} : { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  return headers;
}
function headerRecord(headers: HeadersInit | undefined): Record<string, string> {
  if (!headers) return {};
  if (typeof Headers !== 'undefined' && headers instanceof Headers) return Object.fromEntries(headers.entries());
  if (Array.isArray(headers)) return Object.fromEntries(headers);
  return headers as Record<string, string>;
}
/** Whether this tab holds a login at all. */
export function hasSession(): boolean { return Boolean(getToken()); }
/** The signed-in account's id (the token's subject), or null. Not a credential. */
export function sessionSubject(): string | null {
  try {
    const part = getToken()?.split('.')[1];
    if (!part) return null;
    const json = atob(part.replace(/-/g, '+').replace(/_/g, '/').padEnd(Math.ceil(part.length / 4) * 4, '='));
    const sub = JSON.parse(json).sub;
    return typeof sub === 'string' && sub ? sub : null;
  } catch { return null; }
}
/**
 * For the one request fetch cannot make (an XMLHttpRequest, for its upload
 * progress): the header to sign it with, taken fresh for every send.
 */
export function authorizationHeader(): string {
  const token = getToken();
  return token ? `Bearer ${token}` : '';
}
let refreshJob: { session: SessionSnapshot; promise: Promise<boolean> } | null = null;
async function tryRefresh(session: SessionSnapshot): Promise<boolean> {
  assertCurrent(session);
  if (!session.refresh) return false;
  if (refreshJob && refreshJob.session.generation === session.generation) return refreshJob.promise;
  const requestRefresh = getRefreshToken();
  const promise = (async () => {
    try {
      const res = await fetch(`${API_BASE}/api/auth/refresh`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ refresh_token: requestRefresh }),
      });
      assertCurrent(session);
      if (!res.ok) return false;
      const json = await res.json();
      assertCurrent(session);
      if (json.success && typeof json.data?.token === 'string' && json.data.token
        && typeof json.data.refresh_token === 'string' && json.data.refresh_token
        && json.data.refresh_token !== requestRefresh) {
        // Rotation changes credentials, not the identity of the active login.
        sessionStorage.setItem('nabiman_token', json.data.token);
        sessionStorage.setItem('nabiman_refresh', json.data.refresh_token);
        currentRefresh = json.data.refresh_token;
        refreshRevision += 1;
        return true;
      }
    } catch (error) {
      assertCurrent(session);
      if ((error as Error).name === 'AbortError') throw error;
    }
    return false;
  })();
  const job = { session, promise };
  refreshJob = job;
  try { return await promise; }
  finally { if (refreshJob === job) refreshJob = null; }
}
/** What a request remembers of the login it was sent under. */
export type SessionMark = SessionSnapshot;
export const markSession = (): SessionMark => snapshotSession();
/**
 * After a 401 to a request sent under `session`: true when it may be sent
 * again, because the access token was (or has just been) renewed. When the
 * login cannot be renewed the tab signs out and asks for a new login.
 */
export async function recoverFromUnauthorized(session: SessionMark): Promise<boolean> {
  assertCurrent(session);
  if (!session.refresh) return false;
  // Another request may already have renewed this same session.
  const refreshed = refreshRevision !== session.revision || getToken() !== session.access || await tryRefresh(session);
  assertCurrent(session);
  if (refreshed) return true;
  clearToken();
  sessionStorage.setItem('nabiman_reauth_required', '1');
  window.location.reload();
  return false;
}
/**
 * fetch for every signed-in request: the bearer token, one renewal and retry
 * on a 401, and the MFA-policy signal. Bodies the browser types itself
 * (FormData, Blob, URLSearchParams) keep their own Content-Type; an abort
 * signal, a blob or a streamed body pass through unchanged.
 */
export async function fetchWithRefresh(url: string, options: RequestInit = {}): Promise<Response> {
  const session = snapshotSession();
  const send = () => {
    assertCurrent(session);
    return fetch(url, { ...options, headers: { ...authHeaders(options.body), ...headerRecord(options.headers) } });
  };
  const res = await send();
  assertCurrent(session);
  if (res.status === 401 && session.refresh) {
    if (options.signal?.aborted) throw new DOMException('Request aborted', 'AbortError');
    if (await recoverFromUnauthorized(session)) {
      if (options.signal?.aborted) throw new DOMException('Request aborted', 'AbortError');
      const retried = await send();
      assertCurrent(session);
      notifyMfaPolicy(retried);
      return retried;
    }
  }
  notifyMfaPolicy(res);
  return res;
}
function notifyMfaPolicy(res: Response) {
  if (res.status === 403 && res.headers?.get("X-Nabiman-MFA-Required") === "1") {
    window.dispatchEvent(new Event("nabiman_mfa_required"));
  }
}

export function useApi<T>(endpoint: string, interval?: number) {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!endpoint) { setData(null); setLoading(false); return; }
    const session = snapshotSession();
    try {
      const res = await fetchWithRefresh(`${API_BASE}${endpoint}`);
      assertCurrent(session);
      if (res.status === 401) { clearToken(); window.location.reload(); return; }
      const json: ApiResponse<T> = await res.json();
      assertCurrent(session);
      if (json.success) { setData(json.data); setError(null); }
      else { setError(json.message); }
    } catch (e) {
      if (isCurrent(session)) setError(e instanceof Error ? e.message : 'Network error');
    } finally { if (isCurrent(session)) setLoading(false); }
  }, [endpoint]);

  useEffect(() => { fetchData(); }, [fetchData]);
  // A hidden tab stops polling and catches up the moment it is shown again.
  useVisibleInterval(fetchData, interval);

  return { data, loading, error, refetch: fetchData };
}

export async function apiPost<T>(endpoint: string, body: object): Promise<ApiResponse<T>> {
  return apiRequest<T>(endpoint, { method: 'POST', body: JSON.stringify(body) });
}

export async function apiDelete<T>(endpoint: string, body: object): Promise<ApiResponse<T>> {
  return apiRequest<T>(endpoint, { method: 'DELETE', body: JSON.stringify(body) });
}

export async function apiRequest<T>(endpoint: string, options: RequestInit = {}): Promise<ApiResponse<T>> {
  const session = snapshotSession();
  const res = await fetchWithRefresh(`${API_BASE}${endpoint}`, options);
  assertCurrent(session);
  const json = await res.json();
  assertCurrent(session);
  return json;
}
