import { useEffect, useState } from 'react';

interface DashboardStreamResult<T> {
  data: T | null;
  connected: boolean;
  /** True once the stream has failed to (re)connect repeatedly — the caller
   *  should fall back to interval polling until it recovers. */
  degraded: boolean;
  error: string | null;
  lastUpdated: number | null;
}

const MAX_BACKOFF = 30_000;
const DEGRADE_AFTER = 3; // consecutive failed connects before advising fallback

/**
 * Consume the server's single multiplexed dashboard SSE stream
 * (snapshot-then-stream) as a long-lived channel with backoff reconnect.
 *
 * Modeled on useJobStream but for an endless channel: no terminal event, and it
 * auto-reconnects. Uses fetch + ReadableStream (not EventSource) so the JWT can
 * ride in the Authorization header. `degraded` lets callers drop to polling.
 */
export function useDashboardStream<T>(path: string, enabled = true): DashboardStreamResult<T> {
  const [data, setData] = useState<T | null>(null);
  const [connected, setConnected] = useState(false);
  const [degraded, setDegraded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<number | null>(null);

  useEffect(() => {
    if (!enabled) return;
    let stopped = false;
    let controller: AbortController | null = null;
    let failures = 0;
    let timer: ReturnType<typeof setTimeout>;

    const connect = async () => {
      if (stopped) return;
      const token = sessionStorage.getItem('nabiman_token');
      controller = new AbortController();
      try {
        const res = await fetch(`${path}${token ? `?token=${encodeURIComponent(token)}` : ''}`, {
          headers: token ? { Authorization: `Bearer ${token}` } : {},
          signal: controller.signal,
        });
        if (!res.ok || !res.body) throw new Error(`HTTP ${res.status}`);
        failures = 0;
        setConnected(true);
        setDegraded(false);
        setError(null);
        const reader = res.body.getReader();
        const decoder = new TextDecoder();
        let buffer = '';
        while (!stopped) {
          const { done, value } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          let idx;
          while ((idx = buffer.indexOf('\n\n')) !== -1) {
            const snap = parseSnapshot<T>(buffer.slice(0, idx));
            buffer = buffer.slice(idx + 2);
            if (snap !== null) { setData(snap); setLastUpdated(Date.now()); }
          }
        }
      } catch (e) {
        if ((e as Error).name === 'AbortError') return;
        setError((e as Error).message);
      }
      setConnected(false);
      if (stopped) return;
      failures += 1;
      if (failures >= DEGRADE_AFTER) setDegraded(true);
      const backoff = Math.min(1000 * 2 ** (failures - 1), MAX_BACKOFF);
      timer = setTimeout(connect, backoff);
    };

    connect();
    return () => { stopped = true; controller?.abort(); clearTimeout(timer); };
  }, [path, enabled]);

  return { data, connected, degraded, error, lastUpdated };
}

function parseSnapshot<T>(chunk: string): T | null {
  const dataLines: string[] = [];
  let event = 'message';
  for (const line of chunk.split('\n')) {
    if (line.startsWith(':')) continue;
    if (line.startsWith('event: ')) event = line.slice(7).trim();
    else if (line.startsWith('data: ')) dataLines.push(line.slice(6));
  }
  if (event !== 'snapshot' || dataLines.length === 0) return null;
  try { return JSON.parse(dataLines.join('\n')) as T; } catch { return null; }
}
