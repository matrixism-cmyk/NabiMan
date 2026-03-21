import { useState, useEffect, useCallback } from 'react';
import { ApiResponse } from '../types';

const API_BASE = process.env.REACT_APP_API_URL || '';

function getToken(): string | null {
  return sessionStorage.getItem('nabiman_token');
}

export function setToken(token: string) {
  sessionStorage.setItem('nabiman_token', token);
}

export function clearToken() {
  sessionStorage.removeItem('nabiman_token');
}

function authHeaders(): Record<string, string> {
  const token = getToken();
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['X-Auth-Token'] = token;
  return headers;
}

export function useApi<T>(endpoint: string, interval?: number) {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}${endpoint}`, { headers: authHeaders() });
      if (res.status === 401) {
        clearToken();
        window.location.reload();
        return;
      }
      const json: ApiResponse<T> = await res.json();
      if (json.success) {
        setData(json.data);
        setError(null);
      } else {
        setError(json.message);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Network error');
    } finally {
      setLoading(false);
    }
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
  const res = await fetch(`${API_BASE}${endpoint}`, {
    method: 'POST',
    headers: authHeaders(),
    body: JSON.stringify(body),
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
  }
  return res.json();
}

export async function apiDelete<T>(endpoint: string, body: object): Promise<ApiResponse<T>> {
  const res = await fetch(`${API_BASE}${endpoint}`, {
    method: 'DELETE',
    headers: authHeaders(),
    body: JSON.stringify(body),
  });
  if (res.status === 401) {
    clearToken();
    window.location.reload();
  }
  return res.json();
}
