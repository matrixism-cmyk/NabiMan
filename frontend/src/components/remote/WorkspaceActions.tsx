import React, { RefObject, useState } from 'react';
import { useT } from '../../i18n';
import type { TerminalStatus, TerminalViewHandle } from '../terminal/TerminalView';
import SessionNameEdit from '../terminal/SessionNameEdit';
import AccountDictation from '../terminal/dictation/AccountDictation';
import Modal from '../common/Modal';

interface Props {
  compact: boolean;
  viewRef: RefObject<TerminalViewHandle | null>;
  /** The terminal on show, if one is. */
  terminal: { status: TerminalStatus; sessionId: string; sessionName: string } | null;
  onRenamed: () => void;
  onNewSession: () => void;
  onDetach: () => void;
  onShareSession: () => void;
  /** Sharing a files-only server's files, offered in its file view. */
  onShareFiles?: () => void;
  onSettings: () => void;
  /** On a phone, the file panel opens over the terminal from here. */
  files?: { open: boolean; toggle: () => void };
}

/**
 * What can be done with the work on show. A wide screen lays the buttons out
 * in the header; a phone keeps the ones used while typing (the microphone,
 * the files) and puts the rest under "⋯", in a sheet with room for a thumb.
 */
export default function WorkspaceActions(props: Props) {
  const { t } = useT();
  const { compact, viewRef, terminal } = props;
  const [sheet, setSheet] = useState(false);
  const statusText = (s: TerminalStatus) => (s === 'connected' ? t('terminal.connected')
    : s === 'reconnecting' ? t('terminal.reconnecting')
    : s === 'ended' ? t('terminal.sessionEnded') : t('terminal.disconnected'));
  const dictation = terminal && (
    <AccountDictation variant={compact ? 'bar' : 'head'}
      onText={(text) => viewRef.current?.insertText(text)} onKeys={(keys) => viewRef.current?.sendKeys(keys)} />
  );

  if (!compact) {
    return (
      <div className="rw-head-actions">
        {props.onShareFiles && (
          <button className="btn btn-secondary btn-sm" onClick={props.onShareFiles}>🔗 {t('share.filesButton')}</button>
        )}
        {terminal && (
          <>
            <span className={`status-badge ${terminal.status === 'connected' ? 'up' : 'down'}`}>{statusText(terminal.status)}</span>
            {dictation}
            <button className="btn btn-secondary btn-sm" onClick={props.onNewSession}>{t('terminal.newSession')}</button>
            <button className="btn btn-secondary btn-sm" onClick={props.onDetach} title={t('remote.detachHint')}>⧉ {t('remote.detach')}</button>
            {terminal.sessionId && (<>
              <SessionNameEdit sessionId={terminal.sessionId} onRenamed={props.onRenamed} name={terminal.sessionName} />
              <button className="btn btn-secondary btn-sm" onClick={props.onShareSession} title={t('share.hint')}>
                🔗 {t('share.button')}
              </button>
            </>)}
          </>
        )}
        <button className="btn btn-secondary btn-sm" onClick={props.onSettings} title={t('terminal.settings')}>⚙</button>
      </div>
    );
  }

  const run = (action?: () => void) => () => { setSheet(false); action?.(); };
  return (
    <div className="rw-head-actions">
      {terminal && (
        <span className={`rw-status-dot ${terminal.status === 'connected' ? 'is-up' : 'is-down'}`}
          role="img" aria-label={statusText(terminal.status)} title={statusText(terminal.status)} />
      )}
      {dictation}
      {props.files && (
        <button type="button" className={`rw-icon-btn ${props.files.open ? 'is-on' : ''}`} aria-pressed={props.files.open}
          aria-label={t('sftp.show')} title={t('sftp.show')} onClick={props.files.toggle}>📁</button>
      )}
      <button type="button" className="rw-icon-btn" aria-label={t('remote.moreActions')} aria-haspopup="dialog"
        onClick={() => setSheet(true)}>⋯</button>
      {sheet && (
        <Modal onClose={() => setSheet(false)} className="rw-sheet" title={t('remote.moreActions')}>
          <div className="rw-sheet-list">
            {terminal && (
              <>
                <p className="rw-sheet-status">{statusText(terminal.status)}</p>
                {terminal.sessionId && (
                  <div className="rw-sheet-name">
                    <SessionNameEdit sessionId={terminal.sessionId} onRenamed={props.onRenamed} name={terminal.sessionName} />
                  </div>
                )}
                <button type="button" className="rw-sheet-item" onClick={run(props.onNewSession)}>＋ {t('terminal.newSession')}</button>
                {terminal.sessionId && (
                  <button type="button" className="rw-sheet-item" onClick={run(props.onShareSession)}>🔗 {t('share.button')}</button>
                )}
              </>
            )}
            {props.onShareFiles && (
              <button type="button" className="rw-sheet-item" onClick={run(props.onShareFiles)}>🔗 {t('share.filesButton')}</button>
            )}
            <button type="button" className="rw-sheet-item" onClick={run(props.onSettings)}>⚙ {t('terminal.settings')}</button>
            <button type="button" className="rw-sheet-item is-cancel" onClick={run()}>{t('common.close')}</button>
          </div>
        </Modal>
      )}
    </div>
  );
}
