import { useEffect, useRef, useState } from 'react';

const MAX_SAMPLES = 30;

export type MetricHistory = Record<string, number[]>;

/**
 * Keeps a rolling, localStorage-persisted history of named numeric metrics.
 * A new sample is appended whenever the supplied values change, so sparklines
 * survive page reloads and reflect recent trend direction.
 */
export function useMetricHistory(
  storageKey: string,
  current: Record<string, number> | null,
): MetricHistory {
  const [history, setHistory] = useState<MetricHistory>(() => {
    try {
      return JSON.parse(localStorage.getItem(storageKey) || '{}');
    } catch {
      return {};
    }
  });
  // Record one sample per distinct `current` object. Callers pass a fresh
  // object per fetch (even when values are unchanged), so the series advances
  // over time and a flat-but-live line is shown rather than nothing.
  const lastRef = useRef<Record<string, number> | null>(null);

  useEffect(() => {
    if (!current || current === lastRef.current) return;
    lastRef.current = current;
    setHistory((prev) => {
      const next: MetricHistory = { ...prev };
      for (const [k, v] of Object.entries(current)) {
        next[k] = [...(prev[k] || []), v].slice(-MAX_SAMPLES);
      }
      try {
        localStorage.setItem(storageKey, JSON.stringify(next));
      } catch {
        /* quota / privacy mode — ignore */
      }
      return next;
    });
  }, [current, storageKey]);

  return history;
}

/** 'up' | 'down' | 'flat' from the last two samples of a series. */
export function trendOf(series?: number[]): 'up' | 'down' | 'flat' {
  if (!series || series.length < 2) return 'flat';
  const a = series[series.length - 2];
  const b = series[series.length - 1];
  if (b > a) return 'up';
  if (b < a) return 'down';
  return 'flat';
}
