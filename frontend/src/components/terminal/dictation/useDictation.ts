import { useCallback, useEffect, useRef, useState } from 'react';
import { Action, finalText, Resampler, Typist, utteranceActions } from './speech';
import { opusUplink, pcmUplink, Uplink } from './uplink';
import { LevelMeter } from './meter';
import { markActivity } from '../../../shared/auth/useIdleLogout';

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
  /** How long a pause ends what was said (ms), for the service's end-of-speech
   *  detection; 0 or none for the older guess on the page. */
  endpointMs?: number;
  /** Press Enter (send) at each pause, not only when "엔터" is said. */
  enterOnPause?: boolean;
  /** Ask the service to take only the enrolled voice ("내 목소리만"). */
  voiceLock?: boolean;
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
export function useDictation({ open, onText, onKeys, muted = false, endpointMs = 0, enterOnPause = false, voiceLock = false }: Options) {
  const [state, setState] = useState<DictationState>('idle');
  const [partial, setPartial] = useState('');
  const [error, setError] = useState('');
  const live = useRef<{ ws: WebSocket; media: MediaStream; audio: AudioContext; uplink: Uplink } | null>(null);
  /** Seconds of other voices "내 목소리만" kept out, as the service reports at the end. */
  const [blocked, setBlocked] = useState(0);
  /** How loud the microphone has been lately: drawn, never rendered by React. */
  const meter = useRef(new LevelMeter()).current;
  const typist = useRef(new Typist());
  const heard = useRef('');
  const pause = useRef<number | undefined>(undefined);
  const handlers = useRef({ onText, onKeys });
  handlers.current = { onText, onKeys };
  const mutedRef = useRef(muted);
  mutedRef.current = muted;
  const pauseRef = useRef({ endpointMs, enterOnPause, voiceLock });
  pauseRef.current = { endpointMs, enterOnPause, voiceLock };
  /** The service tells where speech ends (newer ones): its word wins over the guess. */
  const ends = useRef(false);
  const lineHasText = useRef(false);

  const perform = useCallback((actions: Action[]) => {
    // Speaking is using NabiMan, though no key is pressed.
    markActivity();
    for (const action of actions) {
      if (action.type === 'text') { handlers.current.onText(action.text); lineHasText.current = true; }
      else if (action.type === 'enter') { handlers.current.onKeys('\r'); lineHasText.current = false; }
      else handlers.current.onKeys('\x7f'.repeat(action.count));
    }
  }, []);

  /** Brings the prompt up to what has been said (the page's own guess at pauses). */
  const typeUpTo = useCallback((said: string) => {
    if (!ends.current) perform(typist.current.to(said));
  }, [perform]);

  const release = useCallback(() => {
    window.clearTimeout(pause.current);
    const now = live.current;
    live.current = null;
    if (!now) return;
    now.media.getTracks().forEach((t) => t.stop());
    now.audio.close().catch(() => undefined);
    now.uplink.close();
    if (now.ws.readyState <= WebSocket.OPEN) now.ws.close();
  }, []);

  const start = useCallback(async () => {
    if (live.current) return;
    setError('');
    setPartial('');
    setBlocked(0);
    setState('starting');
    typist.current = new Typist();
    heard.current = '';
    ends.current = false;
    lineHasText.current = false;
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
      const send = (data: ArrayBuffer) => { if (ws.readyState === WebSocket.OPEN) ws.send(data); };
      // Opus where the browser can make it (an eighth of the bytes), PCM where not.
      const uplink = (await opusUplink(send)) ?? pcmUplink(send);
      // The opening message comes before any sound: how it is sent, and how to listen.
      const { endpointMs: pauseMs, voiceLock: onlyMe } = pauseRef.current;
      ws.send(JSON.stringify({ ...uplink.config, ...(pauseMs > 0 ? { endpoint_ms: pauseMs } : {}), ...(onlyMe ? { voice_lock: true } : {}) }));
      const audio = new AudioContext();
      const resampler = new Resampler(audio.sampleRate);
      meter.reset(audio.sampleRate);
      const source = audio.createMediaStreamSource(media);
      const feed = (samples: Float32Array) => {
        if (ws.readyState !== WebSocket.OPEN) return;
        meter.push(samples);
        // Muted, the stream goes on (the service keeps listening) but hears silence.
        for (const frame of resampler.push(samples)) uplink.push(frame, mutedRef.current);
      };
      const url = URL.createObjectURL(new Blob([TAP], { type: 'application/javascript' }));
      await audio.audioWorklet.addModule(url);
      URL.revokeObjectURL(url);
      const tap = new AudioWorkletNode(audio, 'nabiman-tap');
      tap.port.onmessage = (e) => feed(e.data as Float32Array);
      source.connect(tap);
      live.current = { ws, media, audio, uplink };
      ws.onmessage = (e) => {
        if (typeof e.data !== 'string') return;
        let msg: { type?: string; committed?: string; partial?: string; message?: string; utterance?: string; text?: string };
        try { msg = JSON.parse(e.data); } catch { return; }
        if (msg.type === 'stats') { uplink.stats(msg as { received_ms?: unknown }); return; }
        // With end-of-speech detection the service says when something has been said:
        // the words are shown until then, and typed when it does.
        if (msg.type === 'endpoint') {
          ends.current = true;
          window.clearTimeout(pause.current);
          setPartial('');
          perform(utteranceActions(msg.text || '', pauseRef.current.enterOnPause, lineHasText.current));
          return;
        }
        if (msg.type === 'partial' && pauseRef.current.endpointMs > 0 && typeof msg.utterance === 'string') {
          ends.current = true;
          setPartial(msg.utterance);
          return;
        }
        if (msg.type === 'final') {
          const kept = (msg as { uplink?: { voice_blocked_sec?: number } }).uplink?.voice_blocked_sec;
          if (typeof kept === 'number' && kept >= 0.5) setBlocked(Math.round(kept));
        }
        if (msg.type === 'final' && ends.current) { setPartial(''); return; }
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
      setState('listening');
    } catch (e) {
      media?.getTracks().forEach((t) => t.stop());
      release();
      setState('idle');
      const name = e instanceof DOMException ? e.name : '';
      setError(name === 'NotAllowedError' ? 'denied' : name === 'NotFoundError' ? 'nomic' : e instanceof Error && e.message !== 'dictation' ? e.message : 'connection');
    }
  }, [open, release, typeUpTo, perform, meter]);

  /** Stops listening, and types what the last words turn out to be. */
  const stop = useCallback(() => {
    const now = live.current;
    if (!now) return;
    setState('stopping');
    now.media.getTracks().forEach((t) => t.stop());
    now.audio.close().catch(() => undefined);
    // What is still in the encoder goes first, or the last words are cut off.
    now.uplink.finish().catch(() => undefined).then(() => {
      if (now.ws.readyState === WebSocket.OPEN) now.ws.send(JSON.stringify({ type: 'stop' }));
    });
    window.setTimeout(() => { if (live.current === now) release(); setState('idle'); }, LAST_WORDS_MS);
  }, [release]);

  useEffect(() => release, [release]);

  return { state, partial, error, blocked, meter, start, stop };
}
