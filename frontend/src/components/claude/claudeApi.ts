import { apiRequest, fetchWithRefresh } from '../../hooks/useApi';

const BASE = '/api/terminal/claude';
const API = process.env.REACT_APP_API_URL || '';

export interface VoiceSettings {
  speaker: string;
  language: string;
  instruct: string;
  speed: number;
  continuous: boolean;
  tools: 'none' | 'read';
  model: string;
  run_as: string;
  claude_path: string;
  extra_prompt: string;
}

export interface VoiceStatus {
  claude: string | null;
  run_as: string | null;
  problem: string | null;
  voice_ready: boolean;
  speakers: { id: string; name: string; desc: string }[];
}

export interface ChatMessage { role: 'user' | 'assistant'; text: string; at: number; error?: string }
export interface Chat { id: string; title: string; created: number; updated: number; messages: ChatMessage[] }
export interface ChatSummary { id: string; title: string; created: number; updated: number; count: number }

export type AskEvent =
  | { type: 'start' }
  | { type: 'delta'; text: string }
  | { type: 'tool'; name: string }
  | { type: 'note'; message: string }
  | { type: 'done'; text: string }
  | { type: 'error'; message: string; text?: string };

export const getSettings = () => apiRequest<{ settings: VoiceSettings; status: VoiceStatus }>(`${BASE}/settings`);
export const putSettings = (s: VoiceSettings) =>
  apiRequest<{ settings: VoiceSettings; status: VoiceStatus }>(`${BASE}/settings`, { method: 'PUT', body: JSON.stringify(s) });
export const testClaude = () => apiRequest<{ version: string; run_as: string }>(`${BASE}/test`, { method: 'POST', body: '{}' });
export const listChats = () => apiRequest<ChatSummary[]>(`${BASE}/chats`);
export const newChat = () => apiRequest<Chat>(`${BASE}/chats`, { method: 'POST', body: '{}' });
export const getChat = (id: string) => apiRequest<Chat>(`${BASE}/chats/${id}`);
export const deleteChat = (id: string) => apiRequest<null>(`${BASE}/chats/${id}`, { method: 'DELETE' });
export const stopAnswer = (id: string) => apiRequest<{ stopped: boolean }>(`${BASE}/chats/${id}/stop`, { method: 'POST', body: '{}' });

/** Asks, and hands over each event of the answer as it comes. */
export async function ask(id: string, text: string, onEvent: (e: AskEvent) => void, signal?: AbortSignal): Promise<void> {
  const res = await fetchWithRefresh(`${API}${BASE}/chats/${id}/ask`, {
    method: 'POST', body: JSON.stringify({ text }), headers: { 'Content-Type': 'application/json' }, signal,
  });
  if (!res.ok || !res.body) {
    const body = await res.json().catch(() => null);
    throw new Error(body?.message || `HTTP ${res.status}`);
  }
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = '';
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    let nl = buf.indexOf('\n');
    while (nl >= 0) {
      const raw = buf.slice(0, nl).trim();
      buf = buf.slice(nl + 1);
      if (raw) { try { onEvent(JSON.parse(raw)); } catch { /* a broken line */ } }
      nl = buf.indexOf('\n');
    }
  }
}

/** `text` read aloud as WAV, in the saved voice or one to try. */
export async function speak(text: string, signal?: AbortSignal, voice?: { speaker?: string; instruct?: string }): Promise<Blob> {
  const res = await fetchWithRefresh(`${API}${BASE}/speak`, {
    method: 'POST', body: JSON.stringify({ text, ...voice }), headers: { 'Content-Type': 'application/json' }, signal,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    throw new Error(body?.message || `HTTP ${res.status}`);
  }
  return res.blob();
}
