import React from 'react';
import { useT } from '../../i18n';
import { TerminalStatus } from './TerminalView';

/**
 * The small pill in the corner of a pane. It is a button because every state
 * it shows is one the operator can act on: retry a dropped connection, or
 * start a new session after one ended.
 */
export default function TerminalStatusPill({ status, onRetry }: { status: TerminalStatus; onRetry: () => void }) {
  const { t } = useT();
  if (status === 'connected') return null;

  const label = status === 'reconnecting' ? t('terminal.statusReconnecting')
    : status === 'connecting' ? t('terminal.statusConnecting')
    : status === 'ended' ? t('terminal.statusEnded')
    : t('terminal.statusDisconnected');

  return (
    <button
      type="button"
      className={`terminal-view-status terminal-view-status-${status}`}
      onClick={onRetry}
      title={t('terminal.clickToRetry')}
    >
      {label}
    </button>
  );
}
