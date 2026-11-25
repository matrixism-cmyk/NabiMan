import { Dispatch, SetStateAction, useCallback, useEffect } from 'react';
import { apiRequest } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';
import { confirmDialog, noticeDialog } from '../../shared/ui/ConfirmDialog';
import type { TerminalStatus } from '../terminal/TerminalView';

interface Options {
  /** The inline pane is on show. */
  visible: boolean;
  status: TerminalStatus;
  setStatus: (status: TerminalStatus) => void;
  /** The pane's own key, and the session in it (empty until it has one). */
  storageKey: string;
  sessionId: string;
  /** The sessions as last listed, for the name to ask about. */
  sessions: { id: string; title: string }[];
  setConnected: Dispatch<SetStateAction<Set<string>>>;
  setJoinSessionId: (id: string | undefined) => void;
  /** Back to where the target's sessions are listed. */
  showSessions: () => void;
  refetchSessions: () => void;
}

/**
 * Ending the session in the workspace's own pane, and leaving the pane once
 * its session is over.
 *
 * A floating window has a power button; the inline pane had none, so a
 * session could only be ended from the list or by detaching it first. And a
 * session that ended — by this button, by `exit`, or from elsewhere — left
 * its last screen standing until the target was clicked again. Both now end
 * the same way: the pane lets go and the target's session list shows, without
 * the one that is gone.
 */
export function useSessionEnd({
  visible, status, setStatus, storageKey, sessionId, sessions, setConnected, setJoinSessionId, showSessions, refetchSessions,
}: Options) {
  const { t } = useT();

  const leave = useCallback(() => {
    // Not 'ended' any more: the next shell opened here must not be taken for the one that just went.
    setStatus('connecting');
    setConnected((prev) => { if (!prev.has(storageKey)) return prev; const next = new Set(prev); next.delete(storageKey); return next; });
    setJoinSessionId(undefined);
    showSessions();
    refetchSessions();
  }, [refetchSessions, setConnected, setJoinSessionId, setStatus, showSessions, storageKey]);

  // The session is over (the shell exited, or it was ended here or elsewhere): the pane has nothing more to show.
  useEffect(() => {
    if (visible && status === 'ended') leave();
  }, [visible, status, leave]);

  /** Ends the session in the pane, after asking. */
  const end = useCallback(async () => {
    if (!sessionId) return;
    const name = sessions.find((s) => s.id === sessionId)?.title || t('terminal.unnamedSession');
    if (!(await confirmDialog({ message: t('terminal.endSessionConfirm', { name }), danger: true }))) return;
    const res = await apiRequest(`/api/terminal/sessions/${encodeURIComponent(sessionId)}`, { method: 'DELETE' })
      .catch((e: unknown) => ({ success: false, message: e instanceof Error ? e.message : String(e) }));
    // The pane stays when the session could not be ended, and says why.
    if (!res.success) { await noticeDialog(`${t('terminal.endFailed')}\n${res.message || t('common.error')}`); return; }
    leave();
  }, [leave, sessionId, sessions, t]);

  return { end };
}
