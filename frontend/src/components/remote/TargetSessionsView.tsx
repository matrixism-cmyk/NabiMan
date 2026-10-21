import React from 'react';
import { useT } from '../../shared/i18n';
import SessionsView, { SessionInfo } from './SessionsView';

interface Props {
  /** The server's name, or "로컬 셸". */
  name: string;
  sessions: SessionInfo[];
  currentSessionId?: string;
  onRefresh: () => void;
  onAttach: (session: SessionInfo) => void;
  onPopOut: (session: SessionInfo) => void;
  onShare: (session: SessionInfo) => void;
  onNewSession: () => void;
  onOpenWindow: () => void;
}

/**
 * What a click on a rail entry shows first: the shells already running on
 * that target, so joining one is the obvious move and starting another is a
 * deliberate one.
 */
export default function TargetSessionsView({
  name, sessions, currentSessionId, onRefresh, onAttach, onPopOut, onShare, onNewSession, onOpenWindow,
}: Props) {
  const { t } = useT();
  return (
    <div className="rw-view rw-view-scroll">
      <SessionsView
        sessions={sessions}
        currentSessionId={currentSessionId}
        title={t('remote.targetSessions', { name })}
        onRefresh={onRefresh}
        onAttach={onAttach}
        onPopOut={onPopOut}
        onShare={onShare}
        actions={(
          <>
            <button className="btn btn-primary btn-sm" onClick={onNewSession}>＋ {t('remote.menuNewSession')}</button>
            <button className="btn btn-secondary btn-sm rw-wide-only" onClick={onOpenWindow}>⧉ {t('remote.menuWindow')}</button>
          </>
        )}
        empty={<p className="text-secondary">{t('remote.noTargetSessions')}</p>}
      />
    </div>
  );
}
