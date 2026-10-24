import { apiRequest } from '../../shared/api/useApi';

const API = process.env.REACT_APP_API_URL || '';

export interface RecordingSettings {
  enabled: boolean;
  keystrokes: boolean;
  retention_days: number;
  max_mb: number;
}

export interface Recording {
  sid: string;
  owner: string;
  label: string;
  started_ms: number;
  bytes: number;
  live: boolean;
}

/** One replay step: milliseconds since the start, and the bytes to write. */
export interface Frame { t: number; data: Uint8Array }
export interface Cast { cols: number; rows: number; frames: Frame[]; duration: number }

export const getSettings = () => apiRequest<RecordingSettings>('/api/terminal/recording/settings');
export const saveSettings = (s: RecordingSettings) =>
  apiRequest<string>('/api/terminal/recording/settings', { method: 'PUT', body: JSON.stringify(s) });
export const listRecordings = () => apiRequest<Recording[]>('/api/terminal/recordings');
export const deleteRecording = (sid: string) =>
  apiRequest(`/api/terminal/recordings/${encodeURIComponent(sid)}`, { method: 'DELETE' });

function fromBase64(s: string): Uint8Array {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) out[i] = bin.charCodeAt(i);
  return out;
}

/** Fetches and parses a recording (header line + `[ms, base64]` lines). */
export async function loadCast(sid: string, token: string | null): Promise<Cast> {
  const res = await fetch(`${API}/api/terminal/recordings/${encodeURIComponent(sid)}`, {
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const text = await res.text();
  const lines = text.split('\n').filter((l) => l.trim());
  let cols = 80; let rows = 24;
  const frames: Frame[] = [];
  lines.forEach((line, i) => {
    if (i === 0) {
      try { const h = JSON.parse(line); cols = h.cols || cols; rows = h.rows || rows; } catch { /* no header */ }
      return;
    }
    try {
      const ev = JSON.parse(line) as [number, string];
      frames.push({ t: ev[0], data: fromBase64(ev[1]) });
    } catch { /* skip a torn line */ }
  });
  const duration = frames.length ? frames[frames.length - 1].t : 0;
  return { cols, rows, frames, duration };
}
