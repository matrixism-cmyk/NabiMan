import React, { useCallback, useEffect, useRef, useState } from 'react';
import TerminalView, { TerminalStatus, TerminalViewHandle } from './TerminalView';
import TerminalWithFiles from '../remote/TerminalWithFiles';
import { useSessionTerminalSettings } from './sessionSettings';
import TerminalSettingsModal from './TerminalSettingsModal';
import { useT } from '../../shared/i18n';
import ShareDialog from './ShareDialog';
import SessionNameEdit from './SessionNameEdit';
import ClaudeAddress, { useClaudePeers } from './ClaudeAddress';
import AiGuideButton from './AiGuideButton';
import { apiRequest } from '../../shared/api/useApi';
import type { SessionInfo } from '../remote/SessionsView';
import { TerminalWin } from './windowGeometry';
import WindowShell, { ShellHandlers } from './WindowShell';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import AccountDictation from './dictation/AccountDictation';
import { useVoiceReply } from './dictation/useVoiceReply';

interface FrameProps extends ShellHandlers {
  win: TerminalWin;
  onClose: (id: string) => void;
  onEndSession: (id: string) => void;
}

export default function TerminalWindowFrame({ win, onUpdate, onFocus, onClose, onEndSession }: FrameProps) {
  const { t } = useT();
  const viewRef = useRef<TerminalViewHandle>(null);
  const [status, setStatus] = useState<TerminalStatus>('connecting');
  const [sessionId, setSessionId] = useState('');
  // "음성으로 대답해줘": this window's answer, read aloud.
  const reply = useVoiceReply(viewRef, sessionId || undefined);
  const [showShare, setShowShare] = useState(false);
  const [sessionName, setName] = useState('');
  // The taskbar tells windows apart by it, so the window manager keeps a copy.
  const setSessionName = useCallback((name: string) => {
    setName(name);
    onUpdate(win.id, { sessionName: name });
  }, [onUpdate, win.id]);
  const [showSettings, setShowSettings] = useState(false);
  const settings = useSessionTerminalSettings(sessionId || undefined);
  const claude = useClaudePeers(sessionId || undefined);

  // The window knows its session only by id; look its name up once attached.
  useEffect(() => {
    setName('');
    if (!sessionId) return;
    let live = true;
    apiRequest<SessionInfo[]>('/api/terminal/sessions')
      .then(res => { if (live && res.success) setSessionName(res.data?.find(s => s.id === sessionId)?.title || ''); })
      .catch(() => { /* the name stays blank; renaming still works */ });
    return () => { live = false; };
  }, [sessionId, setSessionName]);

  return (
    <WindowShell
      win={win}
      onUpdate={onUpdate}
      onFocus={onFocus}
      lead={<>
        <span className={`term-window-dot term-window-dot-${status}`} />
        <span className="term-window-title">{win.title}</span>
        {sessionId && (
          <SessionNameEdit variant="bar" sessionId={sessionId} name={sessionName} onRenamed={setSessionName} />
        )}
        <ClaudeAddress variant="bar" peers={claude} />
        <span className="term-window-sub">{win.subtitle}</span>
      </>}
      actions={<>
          <AccountDictation variant="bar" sessionId={sessionId || undefined} onText={(text) => viewRef.current?.insertText(text)}
            onKeys={(keys) => viewRef.current?.sendKeys(keys)} reply={reply} />
          <button
            title={t('terminal.redraw')}
            aria-label={t('terminal.redraw')}
            disabled={status !== 'connected'}
            onClick={(e) => { e.stopPropagation(); onFocus(win.id); viewRef.current?.redraw(); }}
          ><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
            <rect x="2" y="3" width="20" height="14" rx="2" />
            <path d="M8 21h8M12 17v4M15 7a4 4 0 1 0 1 5M15 5v4h-4" />
          </svg></button>
          <button title={t('terminal.reconnect')} onClick={(e) => { e.stopPropagation(); viewRef.current?.reconnect(); }}>↻</button>
          <button title={t('terminal.newSession')} onClick={(e) => { e.stopPropagation(); viewRef.current?.newSession(); }}>+</button>
          {sessionId && (<>
            <button title={t('share.hint')} onClick={(e) => { e.stopPropagation(); setShowShare(true); }}>🔗</button>
            <AiGuideButton variant="bar" sessionId={sessionId} handle={viewRef} connected={status === 'connected'} />
            <button title={t('terminal.sessionSettings')} aria-label={t('terminal.sessionSettings')}
              onClick={(e) => { e.stopPropagation(); setShowSettings(true); }}>⚙</button>
          </>)}
      </>}
      closing={<>
          <button
            title={t('terminal.endSessionHint')}
            onClick={async (e) => {
              e.stopPropagation();
              if ((await confirmDialog({ message: t('terminal.endSessionConfirm', { name: win.title }), danger: true }))) {
                onEndSession(win.id);
              }
            }}
          >⏻</button>
          <button
            className="term-window-close"
            title={t('terminal.closeWindowHint')}
            onClick={(e) => { e.stopPropagation(); onClose(win.id); }}
          >✕</button>
      </>}
      extras={<>
      {showSettings && sessionId && (
        <TerminalSettingsModal sessionId={sessionId} sessionName={sessionName || win.title} onClose={() => setShowSettings(false)} />
      )}
      {showShare && sessionId && (
        <ShareDialog sessionId={sessionId} sessionLabel={sessionName || `${win.title} · ${win.subtitle}`} onClose={() => setShowShare(false)} />
      )}
      </>}
    >
        <TerminalWithFiles target={win.target} name={win.title} place="window"
          onInsert={(text) => viewRef.current?.insertText(text)}>
          <TerminalView
            ref={viewRef}
            target={win.target}
            storageKey={win.id}
            settings={settings}
            active={!win.minimized}
            onStatus={setStatus}
            onSessionId={setSessionId}
          />
        </TerminalWithFiles>
    </WindowShell>
  );
}
