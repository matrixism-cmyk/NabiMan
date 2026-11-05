import { useCallback, useEffect, useState } from 'react';
import { apiRequest } from '../../shared/api/useApi';
import type { SessionInfo } from './SessionsView';

export type OrderMode = 'custom' | 'newest' | 'oldest' | 'name' | 'active';
export const ORDER_MODES: OrderMode[] = ['custom', 'newest', 'oldest', 'name', 'active'];

export interface SessionOrder { mode: OrderMode; ids: string[] }

const name = (s: SessionInfo) => s.title || s.label;
const byNewest = (a: SessionInfo, b: SessionInfo) => a.age_secs - b.age_secs || a.id.localeCompare(b.id);
const weight = (s: SessionInfo) => (s.orchestrator ? 0 : s.busy ? 1 : (s.claude?.length ? 2 : 3));

/**
 * The sessions in the order the account chose. In its own order, sessions it
 * has not placed yet (new ones) come first, newest first, so they are seen.
 */
export function arrange(sessions: SessionInfo[], order: SessionOrder): SessionInfo[] {
  const list = [...sessions];
  switch (order.mode) {
    case 'oldest': return list.sort((a, b) => byNewest(b, a));
    case 'name': return list.sort((a, b) => name(a).localeCompare(name(b), 'ko') || byNewest(a, b));
    case 'active': return list.sort((a, b) => weight(a) - weight(b) || byNewest(a, b));
    case 'custom': {
      const at = new Map(order.ids.map((id, i) => [id, i]));
      return list.sort((a, b) => {
        const x = at.get(a.id);
        const y = at.get(b.id);
        if (x === undefined || y === undefined) return x === undefined && y === undefined ? byNewest(a, b) : x === undefined ? -1 : 1;
        return x - y;
      });
    }
    default: return list.sort(byNewest);
  }
}

/**
 * `shown` with `id` moved to just before `before` (to the end when null),
 * folded back into the whole order: sessions this list does not show (other
 * servers') keep their places around it.
 */
export function moved(all: string[], shown: string[], id: string, before: string | null): string[] {
  const rest = shown.filter((x) => x !== id);
  const at = before === null ? rest.length : Math.max(0, rest.indexOf(before));
  const next = [...rest.slice(0, at), id, ...rest.slice(at)];
  const visible = new Set(shown);
  const firstAt = all.findIndex((x) => visible.has(x));
  const others = all.filter((x) => !visible.has(x));
  const splice = firstAt < 0 ? 0 : all.slice(0, firstAt).filter((x) => !visible.has(x)).length;
  return [...others.slice(0, splice), ...next, ...others.slice(splice)];
}

/** The account's order, read once and saved on every change. */
export function useSessionOrder() {
  const [order, setOrder] = useState<SessionOrder>({ mode: 'newest', ids: [] });
  const [error, setError] = useState('');
  useEffect(() => {
    let live = true;
    Promise.resolve(apiRequest<SessionOrder>('/api/terminal/session-order'))
      .then((res) => { if (live && res?.success && res.data && ORDER_MODES.includes(res.data.mode)) setOrder(res.data); })
      .catch(() => { /* the default order stands */ });
    return () => { live = false; };
  }, []);
  const save = useCallback((next: SessionOrder) => {
    setOrder(next);
    Promise.resolve(apiRequest<SessionOrder>('/api/terminal/session-order', { method: 'PUT', body: JSON.stringify(next) }))
      .then((res) => { setError(res && !res.success ? res.message || 'error' : ''); })
      .catch(() => setError('error'));
  }, []);
  return { order, save, error };
}
