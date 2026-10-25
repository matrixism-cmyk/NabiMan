import React from 'react';
import { RemoteServer } from './remoteTypes';
import { useT } from '../../shared/i18n';
import { hasFiles, hasShell, hasTerminal } from './protocols';
import { OverviewView, Selection, ServerView } from './workspaceState';
import { assignedTo, useMember, useSharing } from './member';

interface Props {
  selection: Selection;
  server?: RemoteServer;
  overviewView: OverviewView;
  onOverviewView: (view: OverviewView) => void;
  serial?: boolean;
  serverView: ServerView;
  onServerView: (view: ServerView) => void;
  /** Sessions anywhere, and live ones on this target. */
  allSessions: number;
  liveCount: number;
}

/**
 * The views of whatever is selected. A server shows only what it is set up
 * for: sessions and a terminal when it has a terminal, a file view when its
 * files are all it offers, one-off commands when it has an SSH shell.
 */
export default function WorkspaceTabs({
  selection, server, overviewView, onOverviewView, serverView, onServerView, allSessions, liveCount, serial,
}: Props) {
  const { t } = useT();
  const member = useMember();
  const sharing = useSharing();
  const tab = (view: ServerView, label: React.ReactNode) => (
    <button className={serverView === view ? 'is-active' : ''} onClick={() => onServerView(view)}>{label}</button>
  );
  const overview = (view: OverviewView, label: React.ReactNode) => (
    <button className={overviewView === view ? 'is-active' : ''} onClick={() => onOverviewView(view)}>{label}</button>
  );
  const terminal = selection.kind === 'local' || (server && hasTerminal(server));
  return (
    <nav className="rw-tabs">
      {selection.kind === 'overview' && (
        <>
          {overview('summary', t('remote.summaryTab'))}
          {overview('sessions', <>{t('remote.sessionsTab')} {allSessions > 0 && <em>{allSessions}</em>}</>)}
          {(sharing.sessions || sharing.files) && overview('links', t('share.panelTab'))}
          {!member && overview('keys', t('remote.sshKey'))}
          {!member && overview('recordings', t('remote.rec.tab'))}
          {serial && overview('serial', t('remote.serialTab'))}
          {overview('settings', t('common.settings'))}
        </>
      )}
      {terminal && (
        <>
          {tab('sessions', <>{t('remote.sessionsTab')} {liveCount > 0 && <em>{liveCount}</em>}</>)}
          {tab('terminal', t('tab.terminal'))}
        </>
      )}
      {server && (
        <>
          {!hasTerminal(server) && hasFiles(server) && tab('files', t('sftp.tab'))}
          {tab('status', t('remote.statusView'))}
          {hasShell(server) && !member && tab('exec', t('remote.exec'))}
          {!assignedTo(member, server) && tab('settings', t('common.settings'))}
        </>
      )}
    </nav>
  );
}

/**
 * The view to show for a server: one it does not offer (a terminal on a
 * files-only server, commands without a shell) falls back to one it does.
 */
export function viewFor(server: RemoteServer | undefined, view: ServerView, member = false): ServerView {
  if (!server) return view;
  if ((view === 'sessions' || view === 'terminal') && !hasTerminal(server)) return hasFiles(server) ? 'files' : 'status';
  if (view === 'files' && (hasTerminal(server) || !hasFiles(server))) return hasTerminal(server) ? 'terminal' : 'status';
  if (view === 'exec' && (member || !hasShell(server))) return 'status';
  if (view === 'settings' && assignedTo(member, server)) return 'status';
  return view;
}
