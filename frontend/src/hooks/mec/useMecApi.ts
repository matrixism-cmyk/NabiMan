import { useState, useEffect, useCallback } from 'react';
import { MecListResponse, MecResponse } from '../../types/mec';
import { fetchWithRefresh } from '../useApi';

const API_BASE = process.env.REACT_APP_API_URL || '';

async function fetchMec<T>(endpoint: string, init?: RequestInit): Promise<T> {
  // Shared with the main API hook: on an expired access token this refreshes
  // and retries once, and only redirects to login if the refresh token is also
  // gone — so MEC pages no longer surface the raw 401 body on session timeout.
  const res = await fetchWithRefresh(`${API_BASE}${endpoint}`, init);
  if (!res.ok && res.status !== 404) {
    const body = await res.text();
    throw new Error(body || `HTTP ${res.status}`);
  }
  return res.json() as Promise<T>;
}

export async function mecGet<T>(endpoint: string): Promise<MecResponse<T>> {
  return fetchMec<MecResponse<T>>(endpoint);
}

export async function mecList<T>(endpoint: string): Promise<MecListResponse<T>> {
  return fetchMec<MecListResponse<T>>(endpoint);
}

export async function mecPost<T>(endpoint: string, body: object): Promise<MecResponse<T>> {
  return fetchMec<MecResponse<T>>(endpoint, {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export async function mecDelete(endpoint: string, confirmName?: string): Promise<void> {
  await fetchMec(endpoint, {
    method: 'DELETE',
    headers: confirmName ? { 'X-Confirm-Name': confirmName } : undefined,
  });
}

export async function mecPatch<T>(endpoint: string, body: object): Promise<MecResponse<T>> {
  return fetchMec<MecResponse<T>>(endpoint, {
    method: 'PATCH',
    body: JSON.stringify(body),
  });
}

interface UseMecApiResult<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

export function useMecApi<T>(endpoint: string, interval?: number): UseMecApiResult<T> {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!endpoint) {
      setLoading(false);
      return;
    }
    try {
      const res = await mecGet<T>(endpoint);
      if (res.error) {
        setError(res.error.message);
      } else if (res.data !== undefined) {
        setData(res.data);
        setError(null);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Network error');
    } finally {
      setLoading(false);
    }
  }, [endpoint]);

  useEffect(() => {
    fetchData();
    if (interval && endpoint) {
      const id = setInterval(fetchData, interval);
      return () => clearInterval(id);
    }
  }, [fetchData, interval, endpoint]);

  return { data, loading, error, refetch: fetchData };
}

export function useMecList<T>(endpoint: string, interval?: number): UseMecApiResult<T[]> {
  const [data, setData] = useState<T[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!endpoint) {
      setLoading(false);
      return;
    }
    try {
      const res = await mecList<T>(endpoint);
      setData(res.data || []);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Network error');
    } finally {
      setLoading(false);
    }
  }, [endpoint]);

  useEffect(() => {
    fetchData();
    if (interval && endpoint) {
      const id = setInterval(fetchData, interval);
      return () => clearInterval(id);
    }
  }, [fetchData, interval, endpoint]);

  return { data, loading, error, refetch: fetchData };
}
