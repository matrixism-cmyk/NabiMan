import { useEffect, useRef, useState } from 'react';

export type JobStreamEvent =
  | { event: 'snapshot'; job_id: string; status?: unknown }
  | { event: 'started'; job_id: string }
  | { event: 'step_started'; job_id: string; step: string }
  | {
      event: 'step_completed';
      job_id: string;
      step: string;
      duration_ms: number;
      message?: string | null;
    }
  | { event: 'step_failed'; job_id: string; step: string; reason: string }
  | { event: 'completed'; job_id: string; result?: unknown }
  | { event: 'failed'; job_id: string; error: string };

interface Options {
  jobId: string | null;
  onEvent?: (ev: JobStreamEvent) => void;
}

export function useJobStream({ jobId, onEvent }: Options) {
  const [events, setEvents] = useState<JobStreamEvent[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const handlerRef = useRef(onEvent);
  handlerRef.current = onEvent;

  useEffect(() => {
    if (!jobId) return;
    const token = sessionStorage.getItem('nabiman_token');
    // EventSource does not support custom headers; we embed token as query string
    // and the backend middleware accepts it. If not supported, we fall back to
    // authenticated fetch + manual parsing.
    const url = `/api/mec/v1/jobs/${encodeURIComponent(jobId)}/stream${
      token ? `?token=${encodeURIComponent(token)}` : ''
    }`;
    const controller = new AbortController();

    (async () => {
      try {
        const res = await fetch(url, {
          headers: token ? { Authorization: `Bearer ${token}` } : {},
          signal: controller.signal,
        });
        if (!res.ok || !res.body) {
          throw new Error(`HTTP ${res.status}`);
        }
        setIsOpen(true);
        const reader = res.body.getReader();
        const decoder = new TextDecoder();
        let buffer = '';
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          let idx;
          while ((idx = buffer.indexOf('\n\n')) !== -1) {
            const chunk = buffer.slice(0, idx);
            buffer = buffer.slice(idx + 2);
            const ev = parseSseChunk(chunk);
            if (ev) {
              setEvents((prev) => [...prev, ev]);
              handlerRef.current?.(ev);
              if (ev.event === 'completed' || ev.event === 'failed') {
                controller.abort();
                setIsOpen(false);
                return;
              }
            }
          }
        }
      } catch (e) {
        if ((e as Error).name !== 'AbortError') {
          setError((e as Error).message);
        }
      } finally {
        setIsOpen(false);
      }
    })();

    return () => controller.abort();
  }, [jobId]);

  return { events, isOpen, error };
}

function parseSseChunk(chunk: string): JobStreamEvent | null {
  const lines = chunk.split('\n');
  let eventName = 'message';
  const dataLines: string[] = [];
  for (const line of lines) {
    if (line.startsWith(':')) continue;
    if (line.startsWith('event: ')) {
      eventName = line.slice(7).trim();
    } else if (line.startsWith('data: ')) {
      dataLines.push(line.slice(6));
    }
  }
  if (dataLines.length === 0) return null;
  try {
    const parsed = JSON.parse(dataLines.join('\n'));
    return { ...(parsed as object), event: eventName } as JobStreamEvent;
  } catch {
    return null;
  }
}
