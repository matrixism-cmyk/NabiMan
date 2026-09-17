import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../i18n';
import { apiRequest, apiPost } from '../hooks/useApi';
import TerminalView, { TerminalStatus, TerminalTarget, TerminalViewHandle } from './terminal/TerminalView';
import { useTerminalSettings } from './terminal/settings';
import { useTerminalWindows } from './terminal/TerminalWindows';
import TerminalSettingsModal from './terminal/TerminalSettingsModal';

interface SessionInfo {
  id: string; label: string; owner: string; ws_count: number; shared: boolean;
  age_secs: number; timeout_secs: number; scrollback: number; keepalive: boolean; alive: boolean;
}

interface TerminalPanelProps {
  sshTarget?: { host: string; port: number; user: string; serverId?: string } | null;
  onSshConnected?: () => void;
  isVisible?: boolean;
}

function formatAge(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}

/**
 * The docked terminal (System → Terminal). One pane, plus the session list
 * shared with the floating windows — every session here can also be popped out
 * into its own window.
 */
export default function TerminalPanel({ sshTarget: externalSshTarget, onSshConnected, isVisible }: TerminalPanelProps) {
  const { t } = useT();
  const { settings } = useTerminalSettings();
  const { openTerminal } = useTerminalWindows();
  const viewRef = useRef<TerminalViewHandle>(null);

  const [target, setTarget] = useState<TerminalTarget>({ kind: 'local' });
  const [joinSessionId, setJoinSessionId] = useState<string | undefined>();
  const [status, setStatus] = useState<TerminalStatus>('connecting');
  const [form, setForm] = useState({ host: '', port: '22', user: 'root' });
  const [showSshForm, setShowSshForm] = useState(false);
  const [showSessions, setShowSessions] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [currentSessionId, setCurrentSessionId] = useState('');

  // The storage key identifies the pane: changing it swaps in a fresh screen
  // with its own remembered session and scrollback.
  const storageKey = useMemo(() => {
    if (joinSessionId) return `dock_join_${joinSessionId}`;
    if (target.kind === 'local') return 'dock_local';
    return `dock_ssh_${target.user}@${target.host}:${target.port}`;
  }, [joinSessionId, target]);

  const refreshSessions = useCallback(async () => {
    const res = await apiRequest<SessionInfo[]>('/api/terminal/sessions');
    if (res.success && res.data) setSessions(res.data);
  }, []);

  useEffect(() => {
    if (!showSessions) return;
    refreshSessions();
    const id = window.setInterval(refreshSessions, 10000);
    return () => window.clearInterval(id);
  }, [showSessions, refreshSessions]);

  useEffect(() => {
    if (externalSshTarget && externalSshTarget.host) {
      setJoinSessionId(undefined);
      setTarget({
        kind: 'ssh',
        host: externalSshTarget.host,
        port: externalSshTarget.port,
        user: externalSshTarget.user,
        serverId: externalSshTarget.serverId,
      });
      setForm({ host: externalSshTarget.host, port: String(externalSshTarget.port), user: externalSshTarget.user });
      setShowSshForm(false);
      onSshConnected?.();
    }
  }, [externalSshTarget, onSshConnected]);

  const handleSshConnect = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.host) return;
    setJoinSessionId(undefined);
    setTarget({ kind: 'ssh', host: form.host, port: parseInt(form.port, 10) || 22, user: form.user || 'root' });
    setShowSshForm(false);
  };

  const handleJoin = (session: SessionInfo) => {
    setShowSessions(false);
    setJoinSessionId(session.id);
    setTarget(session.label.startsWith('ssh:') ? { kind: 'ssh' } : { kind: 'local' });
  };

  const handleKill = async (session: SessionInfo) => {
    if (!window.confirm(t('terminal.endSessionConfirm', { name: session.label }))) return;
    await apiRequest(`/api/terminal/sessions/${session.id}`, { method: 'DELETE' });
    refreshSessions();
  };

  const popOut = (session: SessionInfo) => {
    const ssh = session.label.startsWith('ssh:');
    openTerminal(ssh ? { kind: 'ssh' } : { kind: 'local' }, { title: session.label, joinSessionId: session.id });
    setShowSessions(false);
  };

  const label = target.kind === 'ssh'
    ? `SSH: ${target.user || ''}@${target.host || (joinSessionId ? 'session' : '')}`
    : t('terminal.local');

  return (
    <div className="panel terminal-panel">
      <div className="panel-header">
        <h2>{t('tab.terminal')} ({label})</h2>
        <div className="terminal-controls">
          <span className="terminal-hint" title="Ctrl+Shift+C/V: 복사·붙여넣기 · 휠: 스크롤(과거 출력) · Ctrl+휠: 확대/축소">
            Ctrl+Shift+C/V
          </span>
          <span className={`status-badge ${status === 'connected' ? 'up' : 'down'}`}>
            {status === 'connected' ? t('terminal.connected')
              : status === 'reconnecting' ? t('terminal.reconnecting')
              : status === 'ended' ? t('terminal.sessionEnded')
              : t('terminal.disconnected')}
          </span>
          <button className="btn btn-primary btn-sm" onClick={() => setShowSshForm(!showSshForm)}>SSH</button>
          <button className="btn btn-secondary btn-sm" onClick={() => { setJoinSessionId(undefined); setTarget({ kind: 'local' }); }}>
            {t('terminal.local')}
          </button>
          <button
            className="btn btn-secondary btn-sm"
            title={t('terminal.openWindow')}
            onClick={() => openTerminal(target.kind === 'ssh' && target.host ? target : { kind: 'local' })}
          >⧉ {t('terminal.openWindow')}</button>
          <button className="btn btn-secondary btn-sm" onClick={() => viewRef.current?.newSession()} title={t('terminal.newSession')}>+</button>
          <button className="btn btn-secondary btn-sm" onClick={() => setShowSettings(true)} title={t('terminal.settings')}>⚙</button>
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => { setShowSessions(!showSessions); if (!showSessions) refreshSessions(); }}
            title={t('terminal.sessions')}
          >&#9776;</button>
        </div>
      </div>

      {showSshForm && (
        <form onSubmit={handleSshConnect} className="ssh-form">
          <input placeholder={t('terminal.hostPlaceholder')} value={form.host}
            onChange={(e) => setForm({ ...form, host: e.target.value })} required />
          <input placeholder={t('terminal.portPlaceholder')} value={form.port}
            onChange={(e) => setForm({ ...form, port: e.target.value })} style={{ width: 70 }} />
          <input placeholder={t('terminal.userPlaceholder')} value={form.user}
            onChange={(e) => setForm({ ...form, user: e.target.value })} style={{ width: 100 }} />
          <button type="submit" className="btn btn-primary btn-sm">{t('common.connect')}</button>
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => setShowSshForm(false)}>{t('common.cancel')}</button>
        </form>
      )}

      {showSessions && (
        <div className="terminal-sessions">
          <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>{t('terminal.activeSessions')}</div>
          {sessions.length === 0 ? (
            <div className="text-secondary" style={{ fontSize: 12 }}>{t('terminal.noSessions')}</div>
          ) : (
            <table className="data-table" style={{ fontSize: 12 }}>
              <thead>
                <tr>
                  <th>{t('terminal.label')}</th><th>{t('remote.user')}</th><th>{t('terminal.viewers')}</th>
                  <th>{t('terminal.age')}</th><th>{t('terminal.scrollbackShort')}</th>
                  <th>{t('terminal.keepalive')}</th><th>{t('common.actions')}</th>
                </tr>
              </thead>
              <tbody>
                {sessions.map((s) => (
                  <tr key={s.id} style={{ background: s.id === currentSessionId ? 'rgba(59,130,246,0.1)' : undefined }}>
                    <td><strong>{s.label}</strong>{!s.alive && <span className="status-badge down" style={{ marginLeft: 6 }}>dead</span>}</td>
                    <td>{s.owner}</td>
                    <td>{s.ws_count}</td>
                    <td>{formatAge(s.age_secs)}</td>
                    <td>{s.scrollback.toLocaleString()}</td>
                    <td>
                      <button
                        className={`btn btn-sm ${s.keepalive ? 'btn-primary' : 'btn-secondary'}`}
                        onClick={async () => {
                          await apiPost(`/api/terminal/sessions/${s.id}/keepalive`, { keepalive: !s.keepalive });
                          refreshSessions();
                        }}
                      >{s.keepalive ? 'ON' : 'OFF'}</button>
                    </td>
                    <td>
                      {s.id !== currentSessionId && (
                        <button className="btn btn-primary btn-sm" style={{ marginRight: 4 }} onClick={() => handleJoin(s)}>
                          {t('terminal.join')}
                        </button>
                      )}
                      <button className="btn btn-secondary btn-sm" style={{ marginRight: 4 }} onClick={() => popOut(s)}>⧉</button>
                      <button
                        className={`btn btn-sm ${s.shared ? 'btn-warning' : 'btn-secondary'}`}
                        style={{ marginRight: 4 }}
                        onClick={async () => {
                          await apiPost(`/api/terminal/sessions/${s.id}/share`, { shared: !s.shared });
                          refreshSessions();
                        }}
                      >{s.shared ? t('terminal.shared') : t('terminal.share')}</button>
                      <button className="btn btn-danger btn-sm" onClick={() => handleKill(s)}>✕</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      <div className="terminal-container">
        <TerminalView
          key={storageKey}
          ref={viewRef}
          target={target}
          storageKey={storageKey}
          settings={settings}
          joinSessionId={joinSessionId}
          active={isVisible}
          onStatus={setStatus}
          onSessionId={setCurrentSessionId}
        />
      </div>

      {showSettings && <TerminalSettingsModal onClose={() => setShowSettings(false)} />}
    </div>
  );
}
