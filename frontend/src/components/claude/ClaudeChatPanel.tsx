import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { confirmDialog } from '../common/ConfirmDialog';
import { getSettings, VoiceSettings, VoiceStatus } from './claudeApi';
import { useVoiceChat } from './useVoiceChat';
import './claude.css';

/**
 * On a phone, the conversation takes exactly the rest of the screen below
 * where it starts, keyboard included, so the composer is always in view.
 */
function useFitHeight(ref: React.RefObject<HTMLElement | null>) {
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || typeof window.matchMedia !== 'function') return undefined;
    const phone = window.matchMedia('(max-width: 760px)');
    const fit = () => {
      if (!phone.matches) { el.style.height = ''; return; }
      const visible = window.visualViewport?.height ?? window.innerHeight;
      el.style.height = `${Math.max(360, Math.floor(visible - el.getBoundingClientRect().top - 12))}px`;
    };
    fit();
    window.visualViewport?.addEventListener('resize', fit);
    window.addEventListener('resize', fit);
    phone.addEventListener('change', fit);
    return () => {
      window.visualViewport?.removeEventListener('resize', fit);
      window.removeEventListener('resize', fit);
      phone.removeEventListener('change', fit);
    };
  }, [ref]);
}

const when = (sec: number) => new Date(sec * 1000).toLocaleString(undefined, { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false });

/**
 * Talking with Claude: "새 채팅" starts a conversation and listens; what is
 * said gathers below and goes to Claude when "엔터" is said at the end (or
 * the send button is pressed). The answer appears as it is written and is
 * read aloud; the microphone rests until it has been read.
 */
export default function ClaudeChatPanel() {
  const { t } = useT();
  const [settings, setSettings] = useState<VoiceSettings | null>(null);
  const [status, setStatus] = useState<VoiceStatus | null>(null);
  const [showList, setShowList] = useState(false);
  const v = useVoiceChat(settings);
  const endRef = useRef<HTMLDivElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  useFitHeight(rootRef);

  useEffect(() => {
    getSettings().then((res) => { if (res.success && res.data) { setSettings(res.data.settings); setStatus(res.data.status); } });
  }, []);
  useEffect(() => { endRef.current?.scrollIntoView({ block: 'end' }); }, [v.chat?.messages.length, v.streaming]);

  const listening = v.dictation.state === 'listening' || v.dictation.state === 'starting';
  const stateText = v.phase === 'thinking' ? (v.tool ? t('claude.usingTool', { name: v.tool }) : t('claude.thinking'))
    : v.phase === 'speaking' ? t('claude.speaking')
    : listening ? t('claude.listening') : t('claude.micOff');
  const problem = status?.problem || (status && !status.voice_ready ? t('claude.noVoice') : '');

  const remove = async (id: string, title: string) => {
    if (await confirmDialog({ message: t('claude.deleteChat', { title: title || t('claude.untitled') }), danger: true, confirmLabel: t('common.delete') })) v.remove(id);
  };

  return (
    <div ref={rootRef} className={`cv ${showList ? 'is-list' : ''}`}>
      <aside className="cv-list" aria-label={t('claude.chats')}>
        <button type="button" className="btn btn-primary cv-new" onClick={() => { setShowList(false); v.create(); }}>＋ {t('claude.newChat')}</button>
        <div className="cv-chats">
          {v.chats.map((c) => (
            <div key={c.id} className={`cv-chat ${v.chat?.id === c.id ? 'is-active' : ''}`}>
              <button type="button" className="cv-chat-open" onClick={() => { setShowList(false); v.select(c.id); }}>
                <span className="cv-chat-title">{c.title || t('claude.untitled')}</span>
                <span className="cv-chat-when">{when(c.updated)} · {t('claude.messages', { count: c.count })}</span>
              </button>
              <button type="button" className="cv-chat-delete" aria-label={t('common.delete')} onClick={() => remove(c.id, c.title)}>✕</button>
            </div>
          ))}
          {!v.chats.length && <p className="cv-empty">{t('claude.noChats')}</p>}
        </div>
      </aside>

      <section className="cv-main">
        <header className="cv-head">
          <button type="button" className="cv-list-toggle" onClick={() => setShowList((s) => !s)} aria-label={t('claude.chats')}>☰</button>
          <h2>{v.chat ? (v.chat.title || t('claude.newChat')) : 'Claude'}</h2>
        </header>
        {problem && <div className="message" role="alert">{problem}</div>}

        <div className="cv-messages" aria-live="polite">
          {!v.chat && (
            <div className="cv-welcome">
              <p>{t('claude.welcome')}</p>
              <button type="button" className="btn btn-primary" onClick={v.create} disabled={Boolean(status?.problem)}>🎙 {t('claude.newChat')}</button>
            </div>
          )}
          {v.chat?.messages.map((m, i) => (
            <div key={i} className={`cv-msg is-${m.role} ${m.error ? 'is-failed' : ''}`}>
              <div className="cv-bubble">{m.text || (m.error ? '' : '…')}</div>
              {m.error && m.error !== 'Stopped' && <div className="cv-msg-error">{m.error}</div>}
              {m.role === 'assistant' && m.text && (
                <button type="button" className="cv-replay" onClick={() => v.replay(m.text)} aria-label={t('claude.replay')}>🔊</button>
              )}
            </div>
          ))}
          {v.phase === 'thinking' && (
            <div className="cv-msg is-assistant is-live"><div className="cv-bubble">{v.streaming || '…'}</div></div>
          )}
          <div ref={endRef} />
        </div>

        {v.error && <div className="message cv-error" role="alert">{v.error}</div>}
        {v.dictation.error && (
          <div className="message cv-error" role="alert">
            {['denied', 'nomic', 'connection'].includes(v.dictation.error) ? t(`dictation.${v.dictation.error}`) : v.dictation.error}
          </div>
        )}

        <footer className="cv-composer">
          <div className={`cv-state is-${v.phase} ${listening ? 'is-listening' : ''}`} role="status">
            <span className="cv-state-dot" />{stateText}
            {(v.phase !== 'idle') && (
              <button type="button" className="btn btn-secondary btn-sm cv-stop" onClick={v.stop}>■ {t('claude.stop')}</button>
            )}
          </div>
          <div className="cv-input">
            <button type="button" className={`cv-mic ${listening ? 'is-on' : ''} ${v.phase !== 'idle' ? 'is-resting' : ''}`}
              aria-pressed={listening} aria-label={listening ? t('dictation.stop') : t('dictation.start')}
              onClick={() => (listening ? v.dictation.stop() : v.listen())} disabled={Boolean(status && !status.voice_ready)}>🎙</button>
            <textarea className="cv-draft" value={v.draft} rows={2} placeholder={t('claude.draftPlaceholder')}
              aria-label={t('claude.draft')}
              onChange={(e) => v.setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); v.send(); }
              }} />
            <button type="button" className="btn btn-primary cv-send" onClick={() => v.send()} disabled={!v.draft.trim() || v.phase === 'thinking'}>
              {t('claude.send')}
            </button>
          </div>
          {listening && v.dictation.partial && v.phase === 'idle' && <div className="cv-partial">{v.dictation.partial}</div>}
        </footer>
      </section>
    </div>
  );
}
