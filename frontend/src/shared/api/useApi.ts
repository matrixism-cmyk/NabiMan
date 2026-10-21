import { useState, useEffect, useCallback } from 'react';

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
function authHeaders(): Record<string, string> {
  const token = getToken();
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  return headers;
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
export async function fetchWithRefresh(url: string, options: RequestInit = {}): Promise<Response> {
  const session = snapshotSession();
  const send = () => {
    assertCurrent(session);
    return fetch(url, { ...options, headers: { ...authHeaders(), ...(options.headers as Record<string, string> || {}) } });
  };
  const res = await send();
  assertCurrent(session);
  if (res.status === 401 && session.refresh) {
    if (options.signal?.aborted) throw new DOMException('Request aborted', 'AbortError');
    // Another request may already have renewed this same session.
    const refreshed = refreshRevision !== session.revision || getToken() !== session.access || await tryRefresh(session);
    assertCurrent(session);
    if (refreshed) {
      if (options.signal?.aborted) throw new DOMException('Request aborted', 'AbortError');
      const retried = await send();
      assertCurrent(session);
      notifyMfaPolicy(retried);
      return retried;
    }
    clearToken();
    sessionStorage.setItem('nabiman_reauth_required', '1');
    window.location.reload();
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

  useEffect(() => {
    fetchData();
    if (interval) {
      const id = setInterval(fetchData, interval);
      return () => clearInterval(id);
    }
  }, [fetchData, interval]);

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
