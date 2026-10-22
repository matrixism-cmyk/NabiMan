import { RefObject, useCallback, useEffect, useRef, useState } from 'react';
import { apiRequest, fetchWithRefresh } from '../../../shared/api/useApi';
import { Speaker } from '../../claude/speaker';
import type { TerminalViewHandle } from '../terminalTypes';
import { lastAnswer, sentences } from './voiceReply';

export type ReplyState = 'idle' | 'waiting' | 'reading';

/** Claude Code (and Codex) at work: the answer is not over while this shows. */
const BUSY = /esc to (?:interrupt|cancel)/i;
/** The screen unchanged this long, with nothing at work, the answer is over. */
const QUIET_MS = 2000;
const POLL_MS = 600;
/** Waiting for an answer gives up after this long. */
const MAX_WAIT_MS = 15 * 60 * 1000;
/** How far back the session's history is read for the answer. */
const HISTORY_LINES = 400;

const API = process.env.REACT_APP_API_URL || '';

async function speak(text: string, signal: AbortSignal): Promise<Blob> {
  const res = await fetchWithRefresh(`${API}/api/terminal/dictation/speak`, {
    method: 'POST', body: JSON.stringify({ text }), headers: { 'Content-Type': 'application/json' }, signal,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    throw new Error(body?.message || `HTTP ${res.status}`);
  }
  return res.blob();
}

/**
 * Reading a terminal's answer aloud when it is asked for ("음성으로
 * 대답해줘"): wait for the line that just ran to finish — nothing at work and
 * the screen still — then read its last answer from the session's history.
 */
export function useVoiceReply(handle: RefObject<TerminalViewHandle | null>, sessionId?: string) {
  const [state, setState] = useState<ReplyState>('idle');
  const [note, setNote] = useState('');
  const speaker = useRef<Speaker | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const session = useRef(sessionId);
  session.current = sessionId;

  const voice = useCallback(() => {
    if (!speaker.current) {
      speaker.current = new Speaker(speak, (speaking) => setState((s) => (speaking ? 'reading' : s === 'reading' ? 'idle' : s)),
        (message) => setNote(message));
    }
    return speaker.current;
  }, []);

  const stop = useCallback(() => {
    window.clearInterval(timer.current);
    timer.current = undefined;
    speaker.current?.stop();
    setState('idle');
  }, []);

  /** Call from the tap that starts dictation: a phone plays sound only after one. */
  const prime = useCallback(() => voice().unlock(), [voice]);

  const read = useCallback(async () => {
    let text = '';
    if (session.current) {
      const res = await apiRequest<string>(`/api/terminal/sessions/${encodeURIComponent(session.current)}/scrollback?lines=${HISTORY_LINES}`)
        .catch(() => null);
      if (res?.success && res.data) text = lastAnswer(res.data);
    }
    if (!text) text = lastAnswer(handle.current?.screenText() ?? '');
    if (!text) { setState('idle'); setNote('읽어 줄 답을 찾지 못했습니다'); return; }
    setNote('');
    const say = voice();
    sentences(text).forEach((s) => say.say(s));
  }, [handle, voice]);

  const ask = useCallback(() => {
    stop();
    setNote('');
    setState('waiting');
    const started = Date.now();
    let last = handle.current?.screenText() ?? '';
    let changedAt = started;
    let moved = false;
    timer.current = window.setInterval(() => {
      const screen = handle.current?.screenText() ?? '';
      const now = Date.now();
      if (screen !== last) { last = screen; changedAt = now; moved = true; }
      const busy = BUSY.test(screen);
      if (busy) moved = true;
      if (now - started > MAX_WAIT_MS) { stop(); setNote('답을 너무 오래 기다려 읽기를 그만두었습니다'); return; }
      if (!moved || busy || now - changedAt < QUIET_MS) return;
      window.clearInterval(timer.current);
      timer.current = undefined;
      read();
    }, POLL_MS);
  }, [handle, read, stop]);

  useEffect(() => () => { window.clearInterval(timer.current); speaker.current?.stop(); }, []);

  return { state, note, ask, prime, stop };
}
