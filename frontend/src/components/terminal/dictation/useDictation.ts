import { useCallback, useEffect, useRef, useState } from 'react';
import { Action, finalText, Resampler, Typist } from './speech';

export type DictationState = 'idle' | 'starting' | 'listening' | 'stopping';

interface Options {
  /** Opens the WebSocket to dictate over: an account's or a share link's. */
  open: () => Promise<WebSocket>;
  /** Types text at the prompt. */
  onText: (text: string) => void;
  /** Presses keys: Enter ("\r") for a spoken "엔터", DEL to take back a correction. */
  onKeys: (keys: string) => void;
  /** While true the microphone sends silence: nothing heard is typed (an
   *  answer being read aloud must not be taken for the next question). */
  muted?: boolean;
}

/** Hands the microphone's samples over as they come, off the audio thread. */
const TAP = `class NabimanTap extends AudioWorkletProcessor {
  process(inputs) { const ch = inputs[0] && inputs[0][0]; if (ch) this.port.postMessage(ch.slice(0)); return true; }
}
registerProcessor('nabiman-tap', NabimanTap);`;

/** How long to wait for the last words after stopping. */
const LAST_WORDS_MS = 6000;
/** Unchanged this long, what is heard so far counts as said: the speaker paused. */
const PAUSE_MS = 1200;

/**
 * Live dictation into a terminal. The microphone streams to the speech
 * service, whose text for a short utterance stays tentative until a long
 * pause, so a phrase is typed once the speaker pauses — the text stops
 * changing — or once the service makes it final; never twice. "엔터" presses
 * Enter. What is heard but not typed yet shows as `partial`.
 */
export function useDictation({ open, onText, onKeys, muted = false }: Options) {
  const [state, setState] = useState<DictationState>('idle');
  const [partial, setPartial] = useState('');
  const [error, setError] = useState('');
  const live = useRef<{ ws: WebSocket; media: MediaStream; audio: AudioContext } | null>(null);
  const typist = useRef(new Typist());
  const heard = useRef('');
  const pause = useRef<number | undefined>(undefined);
  const handlers = useRef({ onText, onKeys });
  handlers.current = { onText, onKeys };
  const mutedRef = useRef(muted);
  mutedRef.current = muted;

  /** Brings the prompt up to what has been said. */
  const typeUpTo = useCallback((said: string) => {
    for (const action of typist.current.to(said) as Action[]) {
      if (action.type === 'text') handlers.current.onText(action.text);
      else if (action.type === 'enter') handlers.current.onKeys('\r');
      else handlers.current.onKeys('\x7f'.repeat(action.count));
    }
  }, []);

  const release = useCallback(() => {
    window.clearTimeout(pause.current);
    const now = live.current;
    live.current = null;
    if (!now) return;
    now.media.getTracks().forEach((t) => t.stop());
    now.audio.close().catch(() => undefined);
    if (now.ws.readyState <= WebSocket.OPEN) now.ws.close();
  }, []);

  const start = useCallback(async () => {
    if (live.current) return;
    setError('');
    setPartial('');
    setState('starting');
    typist.current = new Typist();
    heard.current = '';
    let media: MediaStream | null = null;
    try {
      media = await navigator.mediaDevices.getUserMedia({
        audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      });
      const ws = await open();
      ws.binaryType = 'arraybuffer';
      await new Promise<void>((resolve, reject) => {
        ws.onopen = () => resolve();
        ws.onerror = () => reject(new Error('dictation'));
        ws.onclose = () => reject(new Error('dictation'));
      });
      const audio = new AudioContext();
      const resampler = new Resampler(audio.sampleRate);
      const source = audio.createMediaStreamSource(media);
      const feed = (samples: Float32Array) => {
        if (ws.readyState !== WebSocket.OPEN) return;
        // Muted, the stream goes on (the service keeps listening) but hears silence.
        for (const frame of resampler.push(samples)) ws.send(mutedRef.current ? new ArrayBuffer(frame.byteLength) : frame);
      };
      const url = URL.createObjectURL(new Blob([TAP], { type: 'application/javascript' }));
      await audio.audioWorklet.addModule(url);
      URL.revokeObjectURL(url);
      const tap = new AudioWorkletNode(audio, 'nabiman-tap');
      tap.port.onmessage = (e) => feed(e.data as Float32Array);
      source.connect(tap);
      live.current = { ws, media, audio };
      ws.onmessage = (e) => {
        if (typeof e.data !== 'string') return;
        let msg: { type?: string; committed?: string; partial?: string; message?: string };
        try { msg = JSON.parse(e.data); } catch { return; }
        if (msg.type === 'partial') {
          const all = finalText(msg.committed || '', msg.partial || '');
          if (all === heard.current && all) {
            // Heard the same twice: the speaker has paused.
            window.clearTimeout(pause.current);
            typeUpTo(all);
          } else {
            heard.current = all;
            window.clearTimeout(pause.current);
            pause.current = window.setTimeout(() => { typeUpTo(heard.current); setPartial(''); }, PAUSE_MS);
          }
          setPartial(typist.current.pending(all));
        } else if (msg.type === 'final') {
          typeUpTo(finalText(msg.committed || '', msg.partial || ''));
          setPartial('');
        } else if (msg.type === 'error') {
          setError(msg.message || 'error');
        }
      };
      ws.onclose = () => { release(); setPartial(''); setState('idle'); };
      ws.onerror = () => setError('connection');
      ws.send(JSON.stringify({}));
      setState('listening');
    } catch (e) {
      media?.getTracks().forEach((t) => t.stop());
      release();
      setState('idle');
      const name = e instanceof DOMException ? e.name : '';
      setError(name === 'NotAllowedError' ? 'denied' : name === 'NotFoundError' ? 'nomic' : e instanceof Error && e.message !== 'dictation' ? e.message : 'connection');
    }
  }, [open, release, typeUpTo]);

  /** Stops listening, and types what the last words turn out to be. */
  const stop = useCallback(() => {
    const now = live.current;
    if (!now) return;
    setState('stopping');
    now.media.getTracks().forEach((t) => t.stop());
    now.audio.close().catch(() => undefined);
    if (now.ws.readyState === WebSocket.OPEN) now.ws.send(JSON.stringify({ type: 'stop' }));
    window.setTimeout(() => { if (live.current === now) release(); setState('idle'); }, LAST_WORDS_MS);
  }, [release]);

  useEffect(() => release, [release]);

  return { state, partial, error, start, stop };
}
