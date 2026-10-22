import React, { useEffect, useState } from 'react';
import { useT } from '../../../shared/i18n';
import { useDictation } from './useDictation';
import Waveform from './Waveform';
import type { useVoiceReply } from './useVoiceReply';
import './dictation.css';

interface Props {
  open: () => Promise<WebSocket>;
  onText: (text: string) => void;
  /** Presses keys: Enter for a spoken "엔터", DEL to take back a correction. */
  onKeys: (keys: string) => void;
  /** `bar` is the compact form for a floating window's title bar. */
  variant?: 'head' | 'bar';
  /** The service's end-of-speech detection: the pause that ends a sentence, and whether it presses Enter. */
  endpointMs?: number;
  enterOnPause?: boolean;
  /** Only the enrolled voice is taken ("내 목소리만"). */
  voiceLock?: boolean;
  /** Reads the answer aloud when "음성으로 대답해줘" ends a line; none where it cannot. */
  reply?: ReturnType<typeof useVoiceReply>;
  /** Every dictated line's answer is read aloud, asked for or not ("음성으로 대답하기"). */
  replyAlways?: boolean;
}

const KNOWN = ['denied', 'nomic', 'connection'];

/**
 * The microphone beside a terminal: press to dictate, press again to stop.
 * While it listens, the microphone's loudness runs through it as a waveform
 * and the words still being worked out show under it; what is final is typed
 * at the prompt, and a spoken "엔터" presses Enter.
 */
export default function DictationButton({ open, onText, onKeys, variant = 'head', endpointMs, enterOnPause, voiceLock, reply, replyAlways }: Props) {
  const { t } = useT();
  const { state, partial, error, blocked, meter, start, stop } = useDictation({
    open, onText, onKeys, endpointMs, enterOnPause, voiceLock, onVoiceAsk: reply?.ask, replyAlways: Boolean(reply && replyAlways),
    // The answer read aloud must not be heard as the next thing said.
    muted: reply?.state === 'reading',
  });
  // "내 목소리만" says at the end how much of other voices it kept out; shown for a moment.
  const [keptOut, setKeptOut] = useState(0);
  useEffect(() => {
    if (!blocked) return undefined;
    setKeptOut(blocked);
    const timer = window.setTimeout(() => setKeptOut(0), 6000);
    return () => window.clearTimeout(timer);
  }, [blocked]);
  const on = state === 'listening' || state === 'starting';
  const message = error ? (KNOWN.includes(error) ? t(`dictation.${error}`) : error) : '';
  // What a press will do, as this server is set up: Enter at a pause, a pause only, or "엔터" alone.
  const seconds = String((endpointMs || 0) / 1000);
  const startHint = !endpointMs ? t('dictation.startHint')
    : enterOnPause ? t('dictation.startHintPauseEnter', { seconds }) : t('dictation.startHintPause', { seconds });
  const locked = voiceLock ? `${startHint} ${t('dictation.voiceLockHint')}` : startHint;
  const hint = reply && replyAlways ? `${locked} ${t('dictation.voiceReplyOnHint')}` : locked;
  return (
    <span className={`dictation dictation-${variant}`}>
      <button type="button" className={`btn btn-secondary btn-sm dictation-btn ${on ? 'is-on' : ''} is-${state}`}
        aria-pressed={on} title={on ? t('dictation.stopHint') : hint}
        aria-label={t(on ? 'dictation.stop' : 'dictation.start')}
        onMouseDown={(e) => e.preventDefault()}
        onClick={(e) => { e.stopPropagation(); if (on) stop(); else if (state === 'idle') { reply?.prime(); start(); } }}>
        <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8">
          <rect x="9" y="3" width="6" height="11" rx="3" />
          <path d="M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6" />
        </svg>
        {variant === 'head' && <span>{t(on ? 'dictation.listening' : 'dictation.label')}</span>}
        {state === 'listening' && <Waveform meter={meter} />}
      </button>
      {(on || state === 'stopping') && (partial || state !== 'listening') && (
        <span className="dictation-bubble" role="status" aria-live="polite">
          {state === 'starting' ? t('dictation.starting') : state === 'stopping' ? t('dictation.finishing') : partial}
        </span>
      )}
      {message && !on && <span className="dictation-bubble is-error" role="alert">{message}</span>}
      {reply && reply.state !== 'idle' && !partial && (
        <span className="dictation-bubble dictation-reply" role="status">
          {t(reply.state === 'waiting' ? 'dictation.replyWaiting' : 'dictation.replyReading')}
          <button type="button" className="dictation-reply-stop" onMouseDown={(e) => e.preventDefault()}
            onClick={(e) => { e.stopPropagation(); reply.stop(); }} aria-label={t('dictation.replyStop')} title={t('dictation.replyStop')}>■</button>
        </span>
      )}
      {reply?.note && reply.state === 'idle' && !on && <span className="dictation-bubble" role="status">{reply.note}</span>}
      {keptOut > 0 && !on && !message && (
        <span className="dictation-bubble" role="status">{t('dictation.keptOut', { seconds: keptOut })}</span>
      )}
    </span>
  );
}
