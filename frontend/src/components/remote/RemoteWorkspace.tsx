import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useApi, apiPost } from '../../shared/api/useApi';
import { RemoteServer, RemoteServerStatus } from './remoteTypes';
import { useT } from '../../shared/i18n';
import TerminalView, { TerminalStatus, TerminalTarget, TerminalViewHandle } from '../terminal/TerminalView';
import { useSessionTerminalSettings } from '../terminal/sessionSettings';
import { useTerminalWindows } from '../terminal/TerminalWindows';
import { paneEnded, pruneSessionZooms, startFresh } from '../terminal/terminalStorage';
import TerminalSettingsModal from '../terminal/TerminalSettingsModal';
import ShareDialog from '../terminal/ShareDialog';
import ShareLinksPanel from '../terminal/ShareLinksPanel';
import MobileKeyBar from '../terminal/MobileKeyBar';
import { SshKeyPanel } from './SshKeyComponents';
import MultiServerDashboard from './MultiServerDashboard';
import ServerForm from './ServerForm';
import { ServerStatusView, ServerExecView, ServerSettingsView } from './ServerDetail';
import SessionsView, { SessionInfo } from './SessionsView';
import TargetSessionsView from './TargetSessionsView';
import TargetRail from './TargetRail';
import AdhocConnect from './AdhocConnect';
import ConsoleArea from './ConsoleArea';
import ConsoleAccessModal from './ConsoleAccessModal';
import ConsoleShareModal from './ConsoleShareModal';
import { deleteConsole, VmConsole } from './vncApi';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import TerminalWithFiles, { filesScope } from './TerminalWithFiles';
import RemoteSettingsView from './RemoteSettingsView';
import RecordingsView from './RecordingsView';
import WorkspaceActions from './WorkspaceActions';
import { useCompact, useFullScreenPane, useMobilePane } from './useCompact';
import WorkspaceTabs, { viewFor } from './WorkspaceTabs';
import FilesView from './FilesView';
import ProtocolBadges from './ProtocolBadges';
import { addressOf, hasFiles, hasTerminal } from './protocols';
import { useTargetActions } from './useTargetActions';
import {
  OverviewView, readPaneState, readSelection, rememberPaneState, rememberSelection, Selection, ServerView,
  sessionLabel,
} from './workspaceState';
import { useMember } from './member';
import './remote-workspace.css';
/**
 * One place for everything remote: the list of things you can connect to on
 * the left, and whatever you are doing with the selected one on the right.
 *
 * A phone shows one half at a time, like an app: the list, then the work
 * full screen with a way back, a key bar under the terminal, and the file
 * panel over it on demand.
 */
export default function RemoteWorkspace() {
  const { t } = useT();
  const member = useMember();
  const { data: consoles, refetch: refetchConsoles } = useApi<VmConsole[]>('/api/remote/consoles');
  const { data: me } = useApi<{ role: string }>('/api/users/me');
  const canManageConsoles = me?.role === 'admin';
  const [accessConsole, setAccessConsole] = useState<{ key: string; label: string } | null>(null);
  const [shareConsole, setShareConsole] = useState<{ key: string; label: string } | null>(null);
  const { openTerminal } = useTerminalWindows();
  const viewRef = useRef<TerminalViewHandle>(null);
  const { data: serverData, refetch: refetchServers } = useApi<RemoteServer[]>('/api/remote-servers', 30000);
  const { data: sessionData, refetch: refetchSessions } = useApi<SessionInfo[]>('/api/terminal/sessions', 10000);
  const servers = useMemo(() => serverData || [], [serverData]);
  const sessions = useMemo(() => sessionData || [], [sessionData]);
  const [restored] = useState(readPaneState);
  const [selection, setSelection] = useState<Selection>(readSelection);
  const [chosenView, setServerView] = useState<ServerView>(restored.view);
  const [overviewView, setOverviewView] = useState<OverviewView>('summary');
  const [manualPassword, setManualPassword] = useState(false);
  const [joinSessionId, setJoinSessionId] = useState<string | undefined>(restored.join);
  const [connected, setConnected] = useState<Set<string>>(() => new Set(restored.connected));
  // The session each mounted pane is attached to, by pane key.
  const [paneSessions, setPaneSessions] = useState<Record<string, string>>({});
  const [termStatus, setTermStatus] = useState<TerminalStatus>('connecting');
  const [showSettings, setShowSettings] = useState(false);
  const [showShare, setShowShare] = useState(false);
  const [shareTarget, setShareTarget] = useState('');
  const [shareFiles, setShareFiles] = useState(false);
  const [filter, setFilter] = useState('');
  const [checkingAll, setCheckingAll] = useState(false);
  const [checkError, setCheckError] = useState('');
  const [adhocTarget, setAdhocTarget] = useState<{ host: string; port: number; user: string; proto: 'ssh' | 'telnet' } | null>(null);
  const compact = useCompact();
  const { pane, showWork, showList } = useMobilePane(compact, selection.kind !== 'overview');
  const [filesOpen, setFilesOpen] = useState(false);
  useFullScreenPane(pane === 'work');
  useEffect(() => {
    rememberSelection(selection);
    rememberPaneState({ view: chosenView, connected: Array.from(connected), join: joinSessionId });
  }, [selection, chosenView, connected, joinSessionId]);
  // Each session remembers the size it was zoomed to; drop the sizes of
  // sessions that have since ended.
  useEffect(() => {
    if (sessionData) pruneSessionZooms(sessions.filter(s => s.alive).map(s => s.id));
  }, [sessionData, sessions]);
  const selectedServer = selection.kind === 'server' ? servers.find(s => s.id === selection.id) : undefined;
  // A files-only server has no sessions or terminal to open on.
  const serverView = viewFor(selectedServer, chosenView, member);
  // Opening a session list (the fleet's or a target's), or starting a new
  // shell, should show the current state, not whatever the last poll caught.
  useEffect(() => {
    if (selection.kind === 'overview' ? overviewView === 'sessions'
      : (selection.kind === 'server' || selection.kind === 'local') && serverView === 'sessions') refetchSessions();
  }, [selection, overviewView, serverView, refetchSessions]);
  // Nothing selected that is gone (a deleted server) or not a member's to have.
  useEffect(() => {
    if ((selection.kind === 'server' && servers.length > 0 && !selectedServer)
      || (member && selection.kind === 'local')) {
      setSelection({ kind: 'overview' });
    }
  }, [selection, servers, selectedServer, member]);
  // A target opens on the sessions already running there, not on a terminal.
  const select = useCallback((next: Selection, view: ServerView = 'sessions') => {
    setSelection(next);
    setServerView(view);
    setManualPassword(false);
    setJoinSessionId(undefined);
  }, []);
  const label = selection.kind === 'local' ? 'local'
    : selection.kind === 'server' && selectedServer ? sessionLabel(selectedServer)
    : selection.kind === 'adhoc' && adhocTarget ? `${adhocTarget.proto}:${adhocTarget.user}@${adhocTarget.host}:${adhocTarget.port}`
    : null;
  const targetSessions = useMemo(() => (label ? sessions.filter(s => s.label === label) : []), [sessions, label]);
  const liveCount = targetSessions.filter(s => s.alive).length;
  const terminalTarget: TerminalTarget | null =
    selection.kind === 'local' ? { kind: 'local' }
    : selection.kind === 'server' && selectedServer && hasTerminal(selectedServer)
      ? { kind: 'ssh', host: selectedServer.host, port: selectedServer.port, user: selectedServer.user,
          serverId: selectedServer.id, skipSavedPassword: manualPassword, noFiles: !hasFiles(selectedServer) }
    : selection.kind === 'adhoc' && adhocTarget
      ? { kind: 'ssh', host: adhocTarget.host, port: adhocTarget.port, user: adhocTarget.user, telnet: adhocTarget.proto === 'telnet' }
    : null;
  const storageKey = joinSessionId ? `ws_join_${joinSessionId}`
    : selection.kind === 'local' ? 'ws_local'
    : selection.kind === 'server' ? `ws_srv_${selection.id}${manualPassword ? '_manual' : ''}`
    : selection.kind === 'adhoc' && adhocTarget ? `ws_adhoc_${adhocTarget.proto}_${adhocTarget.user}@${adhocTarget.host}:${adhocTarget.port}`
    : '';
  // Selecting a target never dials on its own, not even into a running
  // session: it shows the sessions, and waits for "참여" or "접속".
  const attached = Boolean(terminalTarget)
    && (Boolean(joinSessionId) || connected.has(storageKey));
  const connect = (skipSaved = false) => {
    setManualPassword(skipSaved);
    const key = selection.kind === 'server' ? `ws_srv_${selection.id}${skipSaved ? '_manual' : ''}` : storageKey;
    if (paneEnded(key)) startFresh(key); // asked for: a new shell where the last one ended
    setConnected(prev => new Set(prev).add(key));
    setServerView('terminal');
  };
  const { connectTo, runRailAction } = useTargetActions({
    servers, selection, select, joinSessionId, manualPassword, connected, setConnected,
    setServerView, viewRef, openTerminal, refetchSessions, refetchServers, setError: setCheckError,
  });
  const consoleAction = async (c: VmConsole, action: string) => {
    if (action === 'open') { select({ kind: 'console', vm: c.name, label: c.label, builtin: c.builtin }); showWork(); }
    else if (action === 'detach') window.open(`/remote#${c.protocol === 'rdp' ? 'rdp' : 'console'}/${encodeURIComponent(c.name)}`, `con_${c.name}`, 'width=1300,height=900,menubar=no,toolbar=no,location=no');
    // A new connection opens its own window, so a multi-user VM can hold several.
    else if (action === 'newSession') window.open(`/remote#${c.protocol === 'rdp' ? 'rdp' : 'console'}/${encodeURIComponent(c.name)}`, `con_${c.name}_${Date.now()}`, 'width=1300,height=900,menubar=no,toolbar=no,location=no');
    else if (action === 'edit') select({ kind: 'console-form', edit: { id: c.name, label: c.label, host: c.host || '', port: c.port || (c.protocol === 'rdp' ? 3389 : 5900), protocol: c.protocol || 'vnc', user: c.user || '', builtin: Boolean(c.builtin) } });
    else if (action === 'access') setAccessConsole({ key: c.name, label: c.label });
    else if (action === 'share') setShareConsole({ key: c.name, label: c.label });
    else if (action === 'delete') {
      if (!(await confirmDialog({ message: t('remote.console.deleteConfirm', { label: c.label }), danger: true, confirmLabel: t('common.delete') }))) return;
      await deleteConsole(c.name); await refetchConsoles();
      if (selection.kind === 'console' && selection.vm === c.name) select({ kind: 'overview' });
    }
  };
  const detach = () => {
    if (!terminalTarget) return;
    const title = selection.kind === 'server' && selectedServer ? selectedServer.name
      : selection.kind === 'local' ? t('remote.localShell')
      : `${terminalTarget.user}@${terminalTarget.host}`;
    openTerminal(terminalTarget, { title, joinSessionId: currentSessionId || undefined });
    // The inline pane lets go so the two do not fight over the pane size.
    if (selection.kind === 'server') setServerView('status');
    else select({ kind: 'overview' });
  };
  const attachSession = (session: SessionInfo) => {
    if (session.id === currentSessionId) { setServerView('terminal'); return; } // already open here
    const server = servers.find(s => sessionLabel(s) === session.label);
    if (server) setSelection({ kind: 'server', id: server.id });
    else if (session.label === 'local') setSelection({ kind: 'local' });
    setManualPassword(false);
    setJoinSessionId(session.id);
    setServerView('terminal');
  };
  const popOutSession = (session: SessionInfo) => {
    const server = servers.find(s => sessionLabel(s) === session.label);
    openTerminal(
      server
        ? { kind: 'ssh', host: server.host, port: server.port, user: server.user, serverId: server.id, noFiles: !hasFiles(server) }
        : { kind: session.label === 'local' ? 'local' : 'ssh' },
      { title: server?.name || session.label, joinSessionId: session.id },
    );
  };
  const checkAll = async () => {
    setCheckingAll(true);
    setCheckError('');
    const result = await apiPost<RemoteServerStatus[]>('/api/remote-servers/check-all', {});
    if (!result.success) setCheckError(result.message || t('common.error'));
    setCheckingAll(false);
    refetchServers();
  };
  const online = servers.filter(s => s.status === 'online').length;
  const countFor = (server: RemoteServer) =>
    sessions.filter(s => s.label === sessionLabel(server) && s.alive).length;
  const localCount = sessions.filter(s => s.label === 'local' && s.alive).length;
  const isServerTarget = selection.kind === 'server' && Boolean(selectedServer);
  /** Targets that open on their session list: a saved server or the local shell. */
  const hasSessionsView = (isServerTarget && Boolean(selectedServer && hasTerminal(selectedServer))) || selection.kind === 'local';
  const showTerminalPane = attached && Boolean(terminalTarget);
  // "This pane" is the pane mounted for this target, even kept behind the list.
  const currentSessionId = showTerminalPane ? paneSessions[storageKey] || '' : '';
  useEffect(() => { if (currentSessionId) refetchSessions(); }, [currentSessionId, refetchSessions]);
  const settings = useSessionTerminalSettings(currentSessionId || undefined);
  const terminalVisible = (hasSessionsView && serverView === 'terminal')
    || (selection.kind === 'adhoc' && Boolean(adhocTarget));
  const headTitle = selection.kind === 'overview' ? t('remote.title')
    : selection.kind === 'new' ? t('remote.addRemoteServer')
    : selection.kind === 'local' ? t('remote.localShell')
    : selection.kind === 'adhoc' ? t('remote.adhoc')
    : selectedServer?.name || '';
  const headSub = selection.kind === 'server' && selectedServer
    ? addressOf(selectedServer)
    : selection.kind === 'adhoc' && adhocTarget
      ? `${adhocTarget.user}@${adhocTarget.host}:${adhocTarget.port}`
      : selection.kind === 'local' ? t('remote.localShellHint')
      : '';
  return (
    <div className={`rw ${compact ? `rw-compact rw-pane-${pane}` : ''}`}>
      <TargetRail
        compact={compact}
        servers={servers}
        selection={selection}
        online={online}
        filter={filter}
        checkingAll={checkingAll}
        localSessions={localCount}
        consoles={consoles || []}
        canManageConsoles={canManageConsoles}
        canAddConsole={me?.role === 'admin' || me?.role === 'remote'}
        onAddConsole={() => select({ kind: 'console-form', edit: null })}
        onConsoleAction={consoleAction}
        sessionsFor={countFor}
        onSelect={(next) => { select(next); showWork(); }}
        onAction={(target, action) => {
          runRailAction(target, action);
          if (action !== 'check' && action !== 'delete' && action !== 'window') showWork();
        }}
        onFilter={setFilter}
        onCheckAll={checkAll}
      />
      <section className="rw-work">
        <header className="rw-head">
          <div className="rw-head-title">
            {compact && (
              <button type="button" className="rw-back" onClick={showList} aria-label={t('remote.backToList')}>‹</button>
            )}
            <h2>{headTitle}</h2>
            {selectedServer && <ProtocolBadges server={selectedServer} />}
            {headSub && <code>{headSub}</code>}
            {selectedServer?.has_password && (
              <span className="tag-badge" title={t('remote.passwordStoredHint')}>🔑 {t('remote.passwordStored')}</span>
            )}
            {selectedServer?.tags.map(tag => <span key={tag} className="tag-badge">{tag}</span>)}
          </div>
          <WorkspaceTabs selection={selection} server={selectedServer} overviewView={overviewView}
            onOverviewView={setOverviewView} serverView={serverView} onServerView={setServerView}
            allSessions={sessions.length} liveCount={liveCount} />
          {/* A VM console carries its own bar (settings, share, …), so the
              header's actions — just a lone ⚙ there — are left off for it. */}
          {selection.kind !== 'console' && selection.kind !== 'console-form' && (
            <WorkspaceActions compact={compact} viewRef={viewRef}
              terminal={showTerminalPane && terminalVisible ? {
                status: termStatus, sessionId: currentSessionId,
                sessionName: sessions.find(s => s.id === currentSessionId)?.title || '',
              } : null}
              onRenamed={refetchSessions} onNewSession={() => viewRef.current?.newSession()} onDetach={detach}
              onShareSession={() => { setShareTarget(currentSessionId); setShowShare(true); }}
              onShareFiles={serverView === 'files' && selectedServer ? () => setShareFiles(true) : undefined}
              onSettings={() => setShowSettings(true)}
              files={compact && showTerminalPane && terminalVisible && filesScope(terminalTarget)
                ? { open: filesOpen, toggle: () => setFilesOpen(o => !o) } : undefined} />
          )}
        </header>
        {checkError && <div role="alert" className="message">{checkError}</div>}
        <div className="rw-body">
          {/* The terminal stays mounted while other views are open, so switching
              to 상태 and back does not drop the shell. */}
          {showTerminalPane && terminalTarget && (
            <div className={`rw-view rw-view-terminal ${terminalVisible ? '' : 'rw-view-hidden'}`}>
              <TerminalWithFiles target={terminalTarget} name={headTitle} onInsert={(text) => viewRef.current?.insertText(text)}
                {...(compact ? { open: filesOpen, onOpenChange: setFilesOpen } : {})}>
                <TerminalView
                  key={storageKey}
                  ref={viewRef}
                  target={terminalTarget}
                  storageKey={storageKey}
                  settings={settings}
                  joinSessionId={joinSessionId}
                  active={terminalVisible}
                  onStatus={setTermStatus}
                  onSessionId={(sid) => setPaneSessions(p => (p[storageKey] === sid ? p : { ...p, [storageKey]: sid }))}
                />
              </TerminalWithFiles>
              {compact && <MobileKeyBar handle={viewRef} disabled={termStatus !== 'connected'} />}
            </div>
          )}
          {!showTerminalPane && terminalVisible && terminalTarget && (
            <div className="rw-view rw-connect">
              <div className="rw-connect-card">
                <h3>{headTitle}</h3>
                <code>{headSub}</code>
                <p className="text-secondary">{t('remote.connectHint')}</p>
                <div className="btn-group">
                  <button className="btn btn-primary" onClick={() => connect(false)}>{t('remote.connect')}</button>
                  {liveCount > 0 && (
                    <button className="btn btn-secondary" onClick={() => setServerView('sessions')}>
                      {t('remote.showRunning', { count: liveCount })}
                    </button>
                  )}
                  {selectedServer?.has_password && (
                    <button className="btn btn-secondary" onClick={() => connect(true)} title={t('remote.manualPasswordHint')}>
                      ⌨ {t('remote.manualPassword')}
                    </button>
                  )}
                </div>
              </div>
            </div>
          )}
          {selection.kind === 'overview' && (
            <div className="rw-view rw-view-scroll">
              {overviewView === 'summary' && <MultiServerDashboard />}
              {overviewView === 'sessions' && (
                <SessionsView sessions={sessions} currentSessionId={currentSessionId}
                  onRefresh={refetchSessions} onAttach={attachSession} onPopOut={popOutSession}
                  onShare={(session) => { setShareTarget(session.id); setShowShare(true); }} />
              )}
              {overviewView === 'links' && <ShareLinksPanel />}
              {overviewView === 'keys' && <SshKeyPanel />}
              {overviewView === 'recordings' && <RecordingsView />}
              {overviewView === 'settings' && <RemoteSettingsView />}
            </div>
          )}
          {hasSessionsView && serverView === 'sessions' && (
            <TargetSessionsView name={headTitle} sessions={targetSessions} currentSessionId={currentSessionId}
              onRefresh={refetchSessions} onAttach={attachSession} onPopOut={popOutSession}
              onShare={(session) => { setShareTarget(session.id); setShowShare(true); }}
              onNewSession={() => connectTo(selection, true)}
              onOpenWindow={() => runRailAction(selection, 'window')} />
          )}
          {selection.kind === 'new' && (
            <div className="rw-view rw-view-scroll">
              <div className="rw-section">
                <div className="rw-section-head"><h3>{t('remote.addRemoteServer')}</h3></div>
                <ServerForm
                  onSaved={async (server) => { await refetchServers(); select({ kind: 'server', id: server.id }, 'settings'); }}
                  onCancel={() => select({ kind: 'overview' })}
                />
                {!member && <p className="text-secondary">{t('remote.noServersKeyHint')}</p>}
              </div>
            </div>
          )}
          {selection.kind === 'adhoc' && !adhocTarget && (
            <AdhocConnect onConnect={(target) => {
              setAdhocTarget(target);
              setConnected(prev => new Set(prev).add(`ws_adhoc_${target.proto}_${target.user}@${target.host}:${target.port}`));
            }} />
          )}
          {(selection.kind === 'console' || selection.kind === 'console-form') && (
            <ConsoleArea selection={selection} consoles={consoles || []} canManage={canManageConsoles}
              onSelect={select} onChanged={refetchConsoles}
              onAccess={(key, label) => setAccessConsole({ key, label })}
              onShare={(key, label) => setShareConsole({ key, label })} />
          )}
          {accessConsole && <ConsoleAccessModal console={accessConsole.key} label={accessConsole.label} onClose={() => setAccessConsole(null)} />}
          {shareConsole && <ConsoleShareModal console={shareConsole.key} label={shareConsole.label} onClose={() => setShareConsole(null)} />}
          {isServerTarget && selectedServer && serverView === 'files' && <FilesView server={selectedServer} />}
          {isServerTarget && selectedServer && serverView === 'status' && (
            <div className="rw-view rw-view-scroll"><ServerStatusView server={selectedServer} /></div>
          )}
          {isServerTarget && selectedServer && serverView === 'exec' && (
            <div className="rw-view rw-view-scroll"><ServerExecView server={selectedServer} /></div>
          )}
          {isServerTarget && selectedServer && serverView === 'settings' && (
            <div className="rw-view rw-view-scroll">
              <ServerSettingsView server={selectedServer}
                onChanged={refetchServers}
                onDeleted={() => { refetchServers(); select({ kind: 'overview' }); }} />
            </div>
          )}
        </div>
      </section>
      {shareFiles && selectedServer && (
        <ShareDialog serverId={selectedServer.id} sessionLabel={`${selectedServer.name} · ${headSub}`} onClose={() => setShareFiles(false)} />
      )}
      {showSettings && <TerminalSettingsModal onClose={() => setShowSettings(false)} sessionId={currentSessionId || undefined} sessionName={headTitle} />}
      {showShare && (shareTarget || currentSessionId) && (
        <ShareDialog
          sessionId={shareTarget || currentSessionId}
          sessionLabel={sessions.find(s => s.id === (shareTarget || currentSessionId))?.title
            || `${headTitle} · ${headSub}`}
          onClose={() => { setShowShare(false); setShareTarget(''); }}
        />
      )}
    </div>
  );
}
