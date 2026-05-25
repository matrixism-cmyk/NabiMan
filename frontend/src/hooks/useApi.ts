import { useState, useEffect, useCallback } from 'react';
import { ApiResponse } from '../types';

const API_BASE = process.env.REACT_APP_API_URL || '';

function getToken(): string | null { return sessionStorage.getItem('nabiman_token'); }
function getRefreshToken(): string | null { return sessionStorage.getItem('nabiman_refresh'); }

export function setToken(token: string) { sessionStorage.setItem('nabiman_token', token); }
export function setRefreshToken(token: string) { sessionStorage.setItem('nabiman_refresh', token); }

export function clearToken() {
  sessionStorage.removeItem('nabiman_token');
  sessionStorage.removeItem('nabiman_refresh');
}

function authHeaders(): Record<string, string> {
  const token = getToken();
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;
  return headers;
}

let refreshPromise: Promise<boolean> | null = null;

async function tryRefresh(): Promise<boolean> {
  if (refreshPromise) return refreshPromise;
  refreshPromise = (async () => {
    const rt = getRefreshToken();
    if (!rt) return false;
    try {
      const res = await fetch(`${API_BASE}/api/auth/refresh`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ refresh_token: rt }),
      });
      if (!res.ok) return false;
      const json = await res.json();
      if (json.success && json.data?.token) {
        setToken(json.data.token);
        return true;
      }
    } catch { /* ignore */ }
    return false;
  })();
  const result = await refreshPromise;
  refreshPromise = null;
  return result;
}

export async function fetchWithRefresh(url: string, options: RequestInit = {}): Promise<Response> {
  const res = await fetch(url, { ...options, headers: { ...authHeaders(), ...(options.headers as Record<string, string> || {}) } });
  if (res.status === 401 && getRefreshToken()) {
    const refreshed = await tryRefresh();
    if (refreshed) {
      return fetch(url, { ...options, headers: { ...authHeaders(), ...(options.headers as Record<string, string> || {}) } });
    }
    clearToken();
    window.location.reload();
  }
  return res;
}

export function useApi<T>(endpoint: string, interval?: number) {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!endpoint) { setLoading(false); return; }
    try {
      const res = await fetchWithRefresh(`${API_BASE}${endpoint}`);
      if (res.status === 401) { clearToken(); window.location.reload(); return; }
      const json: ApiResponse<T> = await res.json();
      if (json.success) { setData(json.data); setError(null); }
      else { setError(json.message); }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Network error');
    } finally { setLoading(false); }
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
  const res = await fetchWithRefresh(`${API_BASE}${endpoint}`, {
    method: 'POST', body: JSON.stringify(body),
  });
  return res.json();
}

export async function apiDelete<T>(endpoint: string, body: object): Promise<ApiResponse<T>> {
  const res = await fetchWithRefresh(`${API_BASE}${endpoint}`, {
    method: 'DELETE', body: JSON.stringify(body),
  });
  return res.json();
}

export async function apiRequest<T>(endpoint: string, options: RequestInit = {}): Promise<ApiResponse<T>> {
  const res = await fetchWithRefresh(`${API_BASE}${endpoint}`, options);
  return res.json();
}
