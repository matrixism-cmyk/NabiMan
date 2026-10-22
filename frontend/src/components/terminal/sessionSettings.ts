/**
 * A session's own settings over the account's. What a session leaves unset
 * follows the terminal settings (and dictation's) as they are now. Kept on
 * the server, so a session looks the same in the pane, a floating window and
 * another browser; every view of one session shares what is loaded here.
 */
import { useEffect, useMemo, useState } from 'react';
import { apiRequest } from '../../shared/api/useApi';
import { TerminalSettings, useTerminalSettings } from './settings';

/** What a session may set for itself; null or absent follows the account. */
export interface SessionOverrides {
  font_family?: string | null;
  font_size?: number | null;
  scrollback_lines?: number | null;
  restore_scrollback?: boolean | null;
  dictation_pause_ms?: number | null;
  dictation_auto_enter?: boolean | null;
  dictation_voice_lock?: boolean | null;
  dictation_voice_reply?: boolean | null;
}

/** What the session itself holds: kept alive, and when it closes if not. */
export interface SessionState { keepalive: boolean; timeout_secs: number }

export interface SessionEntry { overrides: SessionOverrides; session: SessionState | null }

const EMPTY: SessionEntry = { overrides: {}, session: null };
const cache = new Map<string, SessionEntry>();
const pending = new Map<string, Promise<void>>();
const listeners = new Set<() => void>();
const notify = () => listeners.forEach((l) => l());
const url = (sid: string) => `/api/terminal/sessions/${encodeURIComponent(sid)}/settings`;

function load(sid: string): Promise<void> {
  const already = pending.get(sid);
  if (already) return already;
  const p = apiRequest<SessionEntry>(url(sid))
    .then((res) => { cache.set(sid, res.success && res.data ? res.data : EMPTY); })
    .catch(() => { cache.set(sid, EMPTY); })
    .finally(() => { pending.delete(sid); notify(); });
  pending.set(sid, p);
  return p;
}

/** The session's settings, loaded once and shared; null until known. */
export function useSessionEntry(sid?: string): SessionEntry | null {
  const [, changed] = useState(0);
  useEffect(() => {
    const listener = () => changed((n) => n + 1);
    listeners.add(listener);
    return () => { listeners.delete(listener); };
  }, []);
  useEffect(() => { if (sid && !cache.has(sid)) load(sid); }, [sid]);
  return sid ? cache.get(sid) || null : null;
}

/** Saves what the session sets for itself; every view of it follows at once. */
export async function saveSessionOverrides(sid: string, overrides: SessionOverrides): Promise<string> {
  const res = await apiRequest<SessionEntry>(url(sid), { method: 'PUT', body: JSON.stringify(overrides) });
  if (!res.success || !res.data) return res.message || 'error';
  cache.set(sid, res.data);
  notify();
  return '';
}

/** Asks again (after the session's keep-alive or closing time changed). */
export function reloadSession(sid: string) {
  cache.delete(sid);
  return load(sid);
}

/** The account's terminal settings with the session's own over them. */
export function withOverrides(base: TerminalSettings, o?: SessionOverrides | null): TerminalSettings {
  if (!o) return base;
  return {
    ...base,
    font_family: o.font_family ?? base.font_family,
    font_size: o.font_size ?? base.font_size,
    scrollback_lines: o.scrollback_lines ?? base.scrollback_lines,
    restore_scrollback: o.restore_scrollback ?? base.restore_scrollback,
  };
}

/** The settings a session's terminal is drawn with. */
export function useSessionTerminalSettings(sid?: string): TerminalSettings {
  const { settings } = useTerminalSettings();
  const entry = useSessionEntry(sid);
  return useMemo(() => withOverrides(settings, entry?.overrides), [settings, entry]);
}
