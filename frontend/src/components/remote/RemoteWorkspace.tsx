import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useApi, apiPost } from '../../hooks/useApi';
import { RemoteServer, RemoteServerStatus } from '../../types';
import { useT } from '../../i18n';
import TerminalView, { TerminalStatus, TerminalTarget, TerminalViewHandle } from '../terminal/TerminalView';
import { useTerminalSettings } from '../terminal/settings';
import { useTerminalWindows } from '../terminal/TerminalWindows';
import TerminalSettingsModal from '../terminal/TerminalSettingsModal';
import ShareDialog from '../terminal/ShareDialog';
import ShareLinksPanel from '../terminal/ShareLinksPanel';
import { SshKeyPanel } from '../SshKeyComponents';
import MultiServerDashboard from '../MultiServerDashboard';
import ServerForm from './ServerForm';
import { ServerStatusView, ServerExecView, ServerSettingsView } from './ServerDetail';
import SessionsView, { SessionInfo } from './SessionsView';
import TargetRail from './TargetRail';
import {
  OverviewView, readSelection, rememberSelection, Selection, ServerView, sessionLabel,
} from './workspaceState';
import './remote-workspace.css';


/**
 * One place for everything remote: the list of things you can connect to on
 * the left, and whatever you are doing with the selected one on the right.
 *
 * It replaces the old split where the server list lived under "원격", the
 * terminal under "시스템", and the fleet summary under "대시보드".
 */
export default function RemoteWorkspace() {
  const { t } = useT();
  const { settings } = useTerminalSettings();
  const { openTerminal } = useTerminalWindows();
  const viewRef = useRef<TerminalViewHandle>(null);

  const { data: serverData, refetch: refetchServers } = useApi<RemoteServer[]>('/api/remote-servers', 30000);
  const { data: sessionData, refetch: refetchSessions } = useApi<SessionInfo[]>('/api/terminal/sessions', 10000);
  const servers = useMemo(() => serverData || [], [serverData]);
  const sessions = useMemo(() => sessionData || [], [sessionData]);

  const [selection, setSelection] = useState<Selection>(readSelection);
  const [serverView, setServerView] = useState<ServerView>('terminal');
  const [overviewView, setOverviewView] = useState<OverviewView>('summary');
  const [manualPassword, setManualPassword] = useState(false);
  const [joinSessionId, setJoinSessionId] = useState<string | undefined>();
  const [connected, setConnected] = useState<Set<string>>(new Set());
  const [currentSessionId, setCurrentSessionId] = useState('');
  const [termStatus, setTermStatus] = useState<TerminalStatus>('connecting');
  const [showSettings, setShowSettings] = useState(false);
  const [showShare, setShowShare] = useState(false);
  const [filter, setFilter] = useState('');
  const [checkingAll, setCheckingAll] = useState(false);
  const [adhocForm, setAdhocForm] = useState({ host: '', port: '22', user: 'root' });
  const [adhocTarget, setAdhocTarget] = useState<{ host: string; port: number; user: string } | null>(null);

  useEffect(() => { rememberSelection(selection); }, [selection]);

  const selectedServer = selection.kind === 'server'
    ? servers.find(s => s.id === selection.id)
    : undefined;

  // A server deleted elsewhere must not leave the pane pointing at nothing.
  useEffect(() => {
    if (selection.kind === 'server' && servers.length > 0 && !selectedServer) {
      setSelection({ kind: 'overview' });
    }
  }, [selection, servers, selectedServer]);

  const select = useCallback((next: Selection, view: ServerView = 'terminal') => {
    setSelection(next);
    setServerView(view);
    setManualPassword(false);
    setJoinSessionId(undefined);
    setCurrentSessionId('');
  }, []);

  const label = selection.kind === 'local' ? 'local'
    : selection.kind === 'server' && selectedServer ? sessionLabel(selectedServer)
    : selection.kind === 'adhoc' && adhocTarget ? `ssh:${adhocTarget.user}@${adhocTarget.host}:${adhocTarget.port}`
    : null;

  const liveSessions = useMemo(
    () => (label ? sessions.filter(s => s.label === label && s.alive) : []),
    [sessions, label],
  );

  const terminalTarget: TerminalTarget | null =
    selection.kind === 'local' ? { kind: 'local' }
    : selection.kind === 'server' && selectedServer
      ? { kind: 'ssh', host: selectedServer.host, port: selectedServer.port, user: selectedServer.user,
          serverId: selectedServer.id, skipSavedPassword: manualPassword }
    : selection.kind === 'adhoc' && adhocTarget
      ? { kind: 'ssh', host: adhocTarget.host, port: adhocTarget.port, user: adhocTarget.user }
    : null;

  const storageKey = joinSessionId ? `ws_join_${joinSessionId}`
    : selection.kind === 'local' ? 'ws_local'
    : selection.kind === 'server' ? `ws_srv_${selection.id}${manualPassword ? '_manual' : ''}`
    : selection.kind === 'adhoc' && adhocTarget ? `ws_adhoc_${adhocTarget.user}@${adhocTarget.host}:${adhocTarget.port}`
    : '';

  // Selecting a target never dials on its own: it attaches when a session is
  // already running, and otherwise waits for an explicit "접속".
  const attached = Boolean(terminalTarget)
    && (Boolean(joinSessionId) || liveSessions.length > 0 || connected.has(storageKey));

  const connect = (skipSaved = false) => {
    setManualPassword(skipSaved);
    const key = selection.kind === 'server' ? `ws_srv_${selection.id}${skipSaved ? '_manual' : ''}` : storageKey;
    setConnected(prev => new Set(prev).add(key));
    setServerView('terminal');
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
        ? { kind: 'ssh', host: server.host, port: server.port, user: server.user, serverId: server.id }
        : { kind: session.label.startsWith('ssh:') ? 'ssh' : 'local' },
      { title: server?.name || session.label, joinSessionId: session.id },
    );
  };

  const checkAll = async () => {
    setCheckingAll(true);
    await apiPost<RemoteServerStatus[]>('/api/remote-servers/check-all', {});
    setCheckingAll(false);
    refetchServers();
  };

  const online = servers.filter(s => s.status === 'online').length;

  const countFor = (server: RemoteServer) =>
    sessions.filter(s => s.label === sessionLabel(server) && s.alive).length;
  const localCount = sessions.filter(s => s.label === 'local' && s.alive).length;

  const isServerTarget = selection.kind === 'server' && Boolean(selectedServer);
  const showTerminalPane = attached && Boolean(terminalTarget);
  const terminalVisible = (isServerTarget && serverView === 'terminal')
    || selection.kind === 'local'
    || (selection.kind === 'adhoc' && Boolean(adhocTarget));

  const headTitle = selection.kind === 'overview' ? t('remote.title')
    : selection.kind === 'new' ? t('remote.addRemoteServer')
    : selection.kind === 'local' ? t('remote.localShell')
    : selection.kind === 'adhoc' ? t('remote.adhoc')
    : selectedServer?.name || '';

  const headSub = selection.kind === 'server' && selectedServer
    ? `${selectedServer.user}@${selectedServer.host}:${selectedServer.port}`
    : selection.kind === 'adhoc' && adhocTarget
      ? `${adhocTarget.user}@${adhocTarget.host}:${adhocTarget.port}`
      : selection.kind === 'local' ? t('remote.localShellHint')
      : '';

  return (
    <div className="rw">
      <TargetRail
        servers={servers}
        selection={selection}
        online={online}
        filter={filter}
        checkingAll={checkingAll}
        localSessions={localCount}
        sessionsFor={countFor}
        onSelect={select}
        onFilter={setFilter}
        onCheckAll={checkAll}
      />

      <section className="rw-work">
        <header className="rw-head">
          <div className="rw-head-title">
            <h2>{headTitle}</h2>
            {headSub && <code>{headSub}</code>}
            {selectedServer?.has_password && (
              <span className="tag-badge" title={t('remote.passwordStoredHint')}>🔑 {t('remote.passwordStored')}</span>
            )}
            {selectedServer?.tags.map(tag => <span key={tag} className="tag-badge">{tag}</span>)}
          </div>

          <nav className="rw-tabs">
            {selection.kind === 'overview' && (
              <>
                <button className={overviewView === 'summary' ? 'is-active' : ''} onClick={() => setOverviewView('summary')}>{t('remote.summaryTab')}</button>
                <button className={overviewView === 'sessions' ? 'is-active' : ''} onClick={() => setOverviewView('sessions')}>{t('remote.sessionsTab')} {sessions.length > 0 && <em>{sessions.length}</em>}</button>
                <button className={overviewView === 'links' ? 'is-active' : ''} onClick={() => setOverviewView('links')}>{t('share.panelTab')}</button>
                <button className={overviewView === 'keys' ? 'is-active' : ''} onClick={() => setOverviewView('keys')}>{t('remote.sshKey')}</button>
              </>
            )}
            {isServerTarget && (
              <>
                <button className={serverView === 'terminal' ? 'is-active' : ''} onClick={() => setServerView('terminal')}>{t('tab.terminal')}</button>
                <button className={serverView === 'status' ? 'is-active' : ''} onClick={() => setServerView('status')}>{t('remote.statusView')}</button>
                <button className={serverView === 'exec' ? 'is-active' : ''} onClick={() => setServerView('exec')}>{t('remote.exec')}</button>
                <button className={serverView === 'settings' ? 'is-active' : ''} onClick={() => setServerView('settings')}>{t('common.settings')}</button>
              </>
            )}
          </nav>

          <div className="rw-head-actions">
            {showTerminalPane && terminalVisible && (
              <>
                <span className={`status-badge ${termStatus === 'connected' ? 'up' : 'down'}`}>
                  {termStatus === 'connected' ? t('terminal.connected')
                    : termStatus === 'reconnecting' ? t('terminal.reconnecting')
                    : termStatus === 'ended' ? t('terminal.sessionEnded')
                    : t('terminal.disconnected')}
                </span>
                <button className="btn btn-secondary btn-sm" onClick={() => viewRef.current?.newSession()}>{t('terminal.newSession')}</button>
                <button className="btn btn-secondary btn-sm" onClick={detach} title={t('remote.detachHint')}>⧉ {t('remote.detach')}</button>
                {currentSessionId && (
                  <button className="btn btn-secondary btn-sm" onClick={() => setShowShare(true)} title={t('share.hint')}>
                    🔗 {t('share.button')}
                  </button>
                )}
              </>
            )}
            <button className="btn btn-secondary btn-sm" onClick={() => setShowSettings(true)} title={t('terminal.settings')}>⚙</button>
          </div>
        </header>

        <div className="rw-body">
          {/* The terminal stays mounted while other views are open, so switching
              to 상태 and back does not drop the shell. */}
          {showTerminalPane && terminalTarget && (
            <div className={`rw-view rw-view-terminal ${terminalVisible ? '' : 'rw-view-hidden'}`}>
              <TerminalView
                key={storageKey}
                ref={viewRef}
                target={terminalTarget}
                storageKey={storageKey}
                settings={settings}
                joinSessionId={joinSessionId}
                active={terminalVisible}
                onStatus={setTermStatus}
                onSessionId={setCurrentSessionId}
              />
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
                  onRefresh={refetchSessions} onAttach={attachSession} onPopOut={popOutSession} />
              )}
              {overviewView === 'links' && <ShareLinksPanel />}
              {overviewView === 'keys' && <SshKeyPanel />}
            </div>
          )}

          {selection.kind === 'new' && (
            <div className="rw-view rw-view-scroll">
              <div className="rw-section">
                <div className="rw-section-head"><h3>{t('remote.addRemoteServer')}</h3></div>
                <ServerForm
                  onSaved={(server) => { refetchServers(); select({ kind: 'server', id: server.id }); }}
                  onCancel={() => select({ kind: 'overview' })}
                />
                <p className="text-secondary">{t('remote.noServersKeyHint')}</p>
              </div>
            </div>
          )}

          {selection.kind === 'adhoc' && !adhocTarget && (
            <div className="rw-view rw-view-scroll">
              <div className="rw-section">
                <div className="rw-section-head"><h3>{t('remote.adhoc')}</h3></div>
                <p className="text-secondary">{t('remote.adhocHint')}</p>
                <form className="filter-row" onSubmit={(e) => {
                  e.preventDefault();
                  if (!adhocForm.host.trim()) return;
                  const target = { host: adhocForm.host.trim(), port: parseInt(adhocForm.port, 10) || 22, user: adhocForm.user.trim() || 'root' };
                  setAdhocTarget(target);
                  setConnected(prev => new Set(prev).add(`ws_adhoc_${target.user}@${target.host}:${target.port}`));
                }}>
                  <input className="filter-input" placeholder={t('terminal.hostPlaceholder')} value={adhocForm.host}
                    onChange={e => setAdhocForm({ ...adhocForm, host: e.target.value })} required />
                  <input className="filter-input" style={{ maxWidth: 90 }} placeholder={t('terminal.portPlaceholder')}
                    value={adhocForm.port} onChange={e => setAdhocForm({ ...adhocForm, port: e.target.value })} />
                  <input className="filter-input" style={{ maxWidth: 140 }} placeholder={t('terminal.userPlaceholder')}
                    value={adhocForm.user} onChange={e => setAdhocForm({ ...adhocForm, user: e.target.value })} />
                  <button type="submit" className="btn btn-primary btn-sm">{t('common.connect')}</button>
                </form>
              </div>
            </div>
          )}

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

      {showSettings && <TerminalSettingsModal onClose={() => setShowSettings(false)} />}
      {showShare && currentSessionId && (
        <ShareDialog
          sessionId={currentSessionId}
          sessionLabel={`${headTitle} · ${headSub}`}
          onClose={() => setShowShare(false)}
        />
      )}
    </div>
  );
}
