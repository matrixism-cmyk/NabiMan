import React, { useState } from 'react';
import { useT } from '../../shared/i18n';
import TerminalSettingsForm from './TerminalSettingsForm';
import SessionSettingsForm from './SessionSettingsForm';
import DictationSettings from './dictation/DictationSettings';

interface Props {
  onClose: () => void;
  /** Opened from a session: its own settings come first. */
  sessionId?: string;
  sessionName?: string;
}

/**
 * The ⚙ dialog. From a session it opens on that session's settings, which
 * follow the account's until set; the other side holds those defaults for
 * every session (and dictation's, for an administrator).
 */
export default function TerminalSettingsModal({ onClose, sessionId, sessionName }: Props) {
  const { t } = useT();
  const [side, setSide] = useState<'session' | 'all'>(sessionId ? 'session' : 'all');
  return (
    <div className="modal-overlay term-settings-overlay" onClick={onClose}>
      <div className="modal-content term-settings" onClick={(e) => e.stopPropagation()}>
        <h3>{t('terminal.settings')}</h3>
        {sessionId && (
          <div className="term-settings-sides" role="tablist">
            <button type="button" role="tab" aria-selected={side === 'session'} className={side === 'session' ? 'is-active' : ''}
              onClick={() => setSide('session')}>{t('terminal.sessionTab')}{sessionName ? ` · ${sessionName}` : ''}</button>
            <button type="button" role="tab" aria-selected={side === 'all'} className={side === 'all' ? 'is-active' : ''}
              onClick={() => setSide('all')}>{t('terminal.defaultsTab')}</button>
          </div>
        )}
        {side === 'session' && sessionId
          ? <SessionSettingsForm sessionId={sessionId} onDone={onClose} />
          : (<>
            <TerminalSettingsForm onDone={onClose} />
            <DictationSettings />
          </>)}
      </div>
    </div>
  );
}
