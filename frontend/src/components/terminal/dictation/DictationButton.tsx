import React from 'react';
import { useT } from '../../../i18n';
import { useDictation } from './useDictation';
import './dictation.css';

interface Props {
  open: () => Promise<WebSocket>;
  onText: (text: string) => void;
  /** Presses keys: Enter for a spoken "엔터", DEL to take back a correction. */
  onKeys: (keys: string) => void;
  /** `bar` is the compact form for a floating window's title bar. */
  variant?: 'head' | 'bar';
}

const KNOWN = ['denied', 'nomic', 'connection'];

/**
 * The microphone beside a terminal: press to dictate, press again to stop.
 * While it listens, the words still being worked out show under it; what is
 * final is typed at the prompt, and a spoken "엔터" presses Enter.
 */
export default function DictationButton({ open, onText, onKeys, variant = 'head' }: Props) {
  const { t } = useT();
  const { state, partial, error, start, stop } = useDictation({ open, onText, onKeys });
  const on = state === 'listening' || state === 'starting';
  const message = error ? (KNOWN.includes(error) ? t(`dictation.${error}`) : error) : '';
  return (
    <span className={`dictation dictation-${variant}`}>
      <button type="button" className={`btn btn-secondary btn-sm dictation-btn ${on ? 'is-on' : ''} is-${state}`}
        aria-pressed={on} title={t(on ? 'dictation.stopHint' : 'dictation.startHint')}
        aria-label={t(on ? 'dictation.stop' : 'dictation.start')}
        onMouseDown={(e) => e.preventDefault()}
        onClick={(e) => { e.stopPropagation(); if (on) stop(); else if (state === 'idle') start(); }}>
        <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8">
          <rect x="9" y="3" width="6" height="11" rx="3" />
          <path d="M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6" />
        </svg>
        {variant === 'head' && <span>{t(on ? 'dictation.listening' : 'dictation.label')}</span>}
      </button>
      {(on || state === 'stopping') && (partial || state !== 'listening') && (
        <span className="dictation-bubble" role="status" aria-live="polite">
          {state === 'starting' ? t('dictation.starting') : state === 'stopping' ? t('dictation.finishing') : partial}
        </span>
      )}
      {message && !on && <span className="dictation-bubble is-error" role="alert">{message}</span>}
    </span>
  );
}
