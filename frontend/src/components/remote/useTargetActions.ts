import React from 'react';
import { apiPost, apiRequest } from '../../hooks/useApi';
import { RemoteServer, RemoteServerStatus } from '../../types';
import { useT } from '../../i18n';
import { TerminalTarget, TerminalViewHandle } from '../terminal/TerminalView';
import { sidKey, writeStored } from '../terminal/terminalStorage';
import { hasFiles, hasTerminal } from './protocols';
import { RailAction, Selection, ServerView } from './workspaceState';

interface Options {
  servers: RemoteServer[];
  selection: Selection;
  select: (next: Selection, view?: ServerView) => void;
  joinSessionId?: string;
  manualPassword: boolean;
  connected: Set<string>;
  setConnected: React.Dispatch<React.SetStateAction<Set<string>>>;
  setServerView: (view: ServerView) => void;
  viewRef: React.RefObject<TerminalViewHandle | null>;
  openTerminal: (target: TerminalTarget, opts?: { title?: string; joinSessionId?: string }) => void;
  refetchSessions: () => void;
  refetchServers: () => void;
  setError: (message: string) => void;
}

/** The pane a target's own (non-joined) terminal lives in. */
export function baseKeyFor(target: Selection): string {
  return target.kind === 'server' ? `ws_srv_${target.id}` : target.kind === 'local' ? 'ws_local' : '';
}

/**
 * What can be done to a connection target from outside its own views: open
 * its terminal, start a new session there, pop it out into a window, check
 * it, edit or delete it. The rail's right-click menu and the buttons over a
 * target's session list both come here, so they behave the same.
 */
export function useTargetActions({
  servers, selection, select, joinSessionId, manualPassword, connected, setConnected,
  setServerView, viewRef, openTerminal, refetchSessions, refetchServers, setError,
}: Options) {
  const { t } = useT();

  const targetFor = (target: Selection): TerminalTarget | null => {
    if (target.kind === 'local') return { kind: 'local' };
    const server = target.kind === 'server' ? servers.find(s => s.id === target.id) : undefined;
    return server && hasTerminal(server)
      ? { kind: 'ssh', host: server.host, port: server.port, user: server.user, serverId: server.id, noFiles: !hasFiles(server) }
      : null;
  };

  /**
   * Open a target's terminal: back into the shell that pane last had, or —
   * with `fresh` — a brand new session alongside whatever is running.
   */
  const connectTo = (target: Selection, fresh: boolean) => {
    const key = baseKeyFor(target);
    if (!key) return;
    const sameTarget = target.kind === selection.kind
      && (target.kind !== 'server' || (selection.kind === 'server' && selection.id === target.id));
    if (fresh && sameTarget && !joinSessionId && !manualPassword && connected.has(key)) {
      // The pane is already up: let it swap its session in place.
      viewRef.current?.newSession();
      setServerView('terminal');
      return;
    }
    if (fresh) writeStored(sidKey(key), '');
    select(target, 'terminal');
    setConnected(prev => new Set(prev).add(key));
  };

  const runRailAction = async (target: Selection, action: RailAction) => {
    const server = target.kind === 'server' ? servers.find(s => s.id === target.id) : undefined;
    if (action === 'connect' || action === 'newSession') { connectTo(target, action === 'newSession'); return; }
    if (action === 'sessions') { select(target, 'sessions'); refetchSessions(); return; }
    if (action === 'status' || action === 'exec' || action === 'files') { select(target, action); return; }
    if (action === 'edit') { select(target, 'settings'); return; }
    if (action === 'window') {
      const terminal = targetFor(target);
      if (terminal) openTerminal(terminal, { title: server?.name || t('remote.localShell') });
      return;
    }
    if (!server) return;
    setError('');
    if (action === 'check') {
      const res = await apiPost<RemoteServerStatus>(`/api/remote-servers/${server.id}/check`, {});
      if (!res.success) setError(res.message || t('common.error'));
      refetchServers();
      return;
    }
    if (!window.confirm(t('remote.deleteConfirm', { name: server.name }))) return;
    const res = await apiRequest(`/api/remote-servers/${server.id}`, { method: 'DELETE' });
    if (!res.success) { setError(res.message || t('common.error')); return; }
    if (selection.kind === 'server' && selection.id === server.id) select({ kind: 'overview' });
    refetchServers();
  };

  return { connectTo, runRailAction };
}
