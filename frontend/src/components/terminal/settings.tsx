import React, { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { apiRequest } from '../../hooks/useApi';

/** Terminal preferences. Mirrors `TerminalSettings` in the Rust backend. */
export interface TerminalSettings {
  /** Lines of history kept per session (xterm buffer + tmux history-limit). */
  scrollback_lines: number;
  /** Keep the session running server-side; never auto-close it. */
  keepalive: boolean;
  /** SSH ServerAliveInterval for remote sessions. */
  keepalive_interval_secs: number;
  /** Seconds a detached session survives when keep-alive is off. 0 = forever. */
  idle_timeout_secs: number;
  font_size: number;
  /** Replay server-side history when a window reopens with an empty screen. */
  restore_scrollback: boolean;
}

export const DEFAULT_TERMINAL_SETTINGS: TerminalSettings = {
  scrollback_lines: 5000,
  keepalive: true,
  keepalive_interval_secs: 30,
  idle_timeout_secs: 0,
  font_size: 14,
  restore_scrollback: true,
};

const STORAGE_KEY = 'nabiman_terminal_settings';

function readLocal(): TerminalSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULT_TERMINAL_SETTINGS, ...JSON.parse(raw) };
  } catch { /* corrupted or blocked storage — fall through to defaults */ }
  return DEFAULT_TERMINAL_SETTINGS;
}

interface Ctx {
  settings: TerminalSettings;
  save: (next: TerminalSettings) => Promise<string>;
  reload: () => void;
}

const TerminalSettingsContext = createContext<Ctx>({
  settings: DEFAULT_TERMINAL_SETTINGS,
  save: async () => '',
  reload: () => {},
});

/**
 * Settings live on the server (per user) so they follow the operator across
 * browsers, with a localStorage mirror so the first paint never waits on a
 * round trip.
 */
export function TerminalSettingsProvider({ children }: { children: React.ReactNode }) {
  const [settings, setSettings] = useState<TerminalSettings>(readLocal);

  const reload = useCallback(async () => {
    const res = await apiRequest<TerminalSettings>('/api/terminal/settings');
    if (res.success && res.data) {
      const merged = { ...DEFAULT_TERMINAL_SETTINGS, ...res.data };
      setSettings(merged);
      try { localStorage.setItem(STORAGE_KEY, JSON.stringify(merged)); } catch { /* ignore */ }
    }
  }, []);

  useEffect(() => { reload(); }, [reload]);

  const save = useCallback(async (next: TerminalSettings): Promise<string> => {
    const res = await apiRequest<TerminalSettings>('/api/terminal/settings', {
      method: 'PUT', body: JSON.stringify(next),
    });
    if (res.success) {
      const merged = { ...DEFAULT_TERMINAL_SETTINGS, ...(res.data || next) };
      setSettings(merged);
      try { localStorage.setItem(STORAGE_KEY, JSON.stringify(merged)); } catch { /* ignore */ }
      return '';
    }
    return res.message || 'save failed';
  }, []);

  return (
    <TerminalSettingsContext.Provider value={{ settings, save, reload }}>
      {children}
    </TerminalSettingsContext.Provider>
  );
}

export function useTerminalSettings() {
  return useContext(TerminalSettingsContext);
}
