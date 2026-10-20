import { useCallback, useEffect, useRef, useState } from 'react';
import { useDictation } from '../terminal/dictation/useDictation';
import { openAccountDictation } from '../terminal/dictation/sockets';
import {
  ask, Chat, ChatSummary, deleteChat, getChat, listChats, newChat, speak, stopAnswer, VoiceSettings,
} from './claudeApi';
import { Sentences } from './sentences';
import { Speaker } from './speaker';

/** Where a spoken conversation is: waiting, Claude writing, or the answer being read. */
export type Phase = 'idle' | 'thinking' | 'speaking';

/**
 * A conversation with Claude by voice. Listening, what is heard collects in
 * the draft; saying "엔터" at the end sends it. The answer is read aloud as it
 * is written, and meanwhile the microphone hears nothing, so the answer is
 * never taken for the next question. When it has been read, listening goes
 * on (if the settings say so) for the next question.
 */
export function useVoiceChat(settings: VoiceSettings | null) {
  const [chats, setChats] = useState<ChatSummary[]>([]);
  const [chat, setChat] = useState<Chat | null>(null);
  const [draft, setDraftState] = useState('');
  // Heard text and a spoken "엔터" can come together: the send must see the text first.
  const draftRef = useRef('');
  const setDraft = useCallback((next: string | ((d: string) => string)) => {
    draftRef.current = typeof next === 'function' ? next(draftRef.current) : next;
    setDraftState(draftRef.current);
  }, []);
  const [writing, setWriting] = useState(false);
  const [speaking, setSpeaking] = useState(false);
  const [streaming, setStreaming] = useState('');
  const [tool, setTool] = useState('');
  const [error, setError] = useState('');
  const abort = useRef<AbortController | null>(null);
  const sendRef = useRef<(text?: string) => void>(() => undefined);
  const speaker = useRef<Speaker | null>(null);
  if (!speaker.current) {
    speaker.current = new Speaker((text, signal) => speak(text, signal), setSpeaking, (m) => setError(m));
  }
  useEffect(() => { if (settings && speaker.current) speaker.current.rate = settings.speed; }, [settings]);

  const phase: Phase = writing ? 'thinking' : speaking ? 'speaking' : 'idle';
  const dictation = useDictation({
    open: openAccountDictation,
    muted: phase !== 'idle',
    onText: (text) => setDraft((d) => d + text),
    onKeys: (keys) => {
      for (const key of keys) {
        if (key === '\r') sendRef.current();
        else if (key === '\x7f') setDraft((d) => Array.from(d).slice(0, -1).join(''));
      }
    },
  });

  const refreshList = useCallback(async () => {
    const res = await listChats();
    if (res.success && res.data) setChats(res.data);
  }, []);
  useEffect(() => { refreshList(); }, [refreshList]);

  const select = useCallback(async (id: string) => {
    speaker.current?.stop();
    const res = await getChat(id);
    if (res.success && res.data) { setChat(res.data); setStreaming(''); setError(''); }
  }, []);

  /** A new conversation, listening at once (this runs in the tap that asked for it). */
  const create = useCallback(async () => {
    speaker.current?.stop();
    speaker.current?.unlock();
    const res = await newChat();
    if (!res.success || !res.data) { setError(res.message || 'error'); return; }
    setChat(res.data);
    setDraft('');
    setStreaming('');
    setError('');
    refreshList();
    if (dictation.state === 'idle') dictation.start();
  }, [dictation, refreshList, setDraft]);

  const remove = useCallback(async (id: string) => {
    await deleteChat(id);
    if (chat?.id === id) setChat(null);
    refreshList();
  }, [chat, refreshList]);

  const send = useCallback(async (given?: string) => {
    const text = (given ?? draftRef.current).trim();
    if (!text || writing) return;
    let current = chat;
    if (!current) {
      const res = await newChat();
      if (!res.success || !res.data) { setError(res.message || 'error'); return; }
      current = res.data;
    }
    const id = current.id;
    setDraft('');
    setError('');
    setTool('');
    setStreaming('');
    setWriting(true);
    setChat({ ...current, messages: [...current.messages, { role: 'user', text, at: Date.now() / 1000 }] });
    const sentences = new Sentences();
    const say = (parts: string[]) => parts.forEach((p) => speaker.current?.say(p));
    abort.current = new AbortController();
    try {
      await ask(id, text, (e) => {
        if (e.type === 'delta') { setStreaming((s) => s + e.text); setTool(''); say(sentences.push(e.text)); }
        else if (e.type === 'tool') setTool(e.name);
        else if (e.type === 'error' && e.message !== 'Stopped') setError(e.message);
      }, abort.current.signal);
      say(sentences.flush());
    } catch (e) {
      if (!abort.current?.signal.aborted) setError(e instanceof Error ? e.message : String(e));
    } finally {
      setWriting(false);
      setTool('');
      const res = await getChat(id);
      if (res.success && res.data) { setChat(res.data); setStreaming(''); }
      refreshList();
    }
  }, [chat, refreshList, setDraft, writing]);
  sendRef.current = send;

  /** Stops Claude writing and the reading, and listens again. */
  const stop = useCallback(() => {
    speaker.current?.stop();
    if (writing && chat) { stopAnswer(chat.id); abort.current?.abort(); }
  }, [chat, writing]);

  /** Reads an earlier answer again. */
  const replay = useCallback((text: string) => {
    speaker.current?.stop();
    speaker.current?.unlock();
    const s = new Sentences();
    [...s.push(text), ...s.flush()].forEach((p) => speaker.current?.say(p));
  }, []);

  const listen = useCallback(() => { speaker.current?.unlock(); dictation.start(); }, [dictation]);

  // Not set to go on listening: once an answer has been read, the microphone closes.
  const was = useRef<Phase>('idle');
  useEffect(() => {
    if (was.current !== 'idle' && phase === 'idle' && settings && !settings.continuous && dictation.state === 'listening') dictation.stop();
    was.current = phase;
  }, [phase, settings, dictation]);

  useEffect(() => () => { speaker.current?.stop(); abort.current?.abort(); }, []);

  return {
    chats, chat, draft, setDraft, phase, streaming, tool, error, setError,
    dictation, listen, select, create, remove, send, stop, replay,
  };
}
