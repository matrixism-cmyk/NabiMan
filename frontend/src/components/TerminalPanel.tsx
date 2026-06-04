import React, { useEffect, useRef, useState, useCallback } from 'react';
import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import 'xterm/css/xterm.css';
import { useT } from '../i18n';
import { apiRequest, apiPost } from '../hooks/useApi';

interface SshTarget { host: string; port: string; user: string; }
interface SessionInfo { id: string; label: string; owner: string; ws_count: number; shared: boolean; age_secs: number; timeout_secs: number; }

const THEME = {
  background: '#0f1923', foreground: '#e0e6ed', cursor: '#3b82f6', cursorAccent: '#0f1923',
  selectionBackground: '#3b82f655', selectionForeground: '#ffffff', selectionInactiveBackground: '#3b82f633',
  black: '#1a2634', red: '#e74c3c', green: '#2ecc71', yellow: '#f39c12', blue: '#3b82f6',
  magenta: '#9b59b6', cyan: '#1abc9c', white: '#e0e6ed',
  brightBlack: '#4a5a6a', brightRed: '#ff6b6b', brightGreen: '#6bff8d', brightYellow: '#ffd93d',
  brightBlue: '#6bb3ff', brightMagenta: '#c471ed', brightCyan: '#45e6c6', brightWhite: '#ffffff',
};

interface TerminalPanelProps {
  sshTarget?: { host: string; port: number; user: string } | null;
  onSshConnected?: () => void;
  isVisible?: boolean;
}

function getSessionKey(mode: string, ssh?: SshTarget): string {
  if (mode === 'ssh' && ssh) return `pty_ssh_${ssh.user}@${ssh.host}:${ssh.port}`;
  return 'pty_local';
}

function formatAge(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs/60)}m`;
  return `${Math.floor(secs/3600)}h ${Math.floor((secs%3600)/60)}m`;
}

export default function TerminalPanel({ sshTarget: externalSshTarget, onSshConnected, isVisible }: TerminalPanelProps) {
  const { t } = useT();
  const termRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState('');
  const [mode, setMode] = useState<'local' | 'ssh'>('local');
  const [sshTarget, setSshTarget] = useState<SshTarget>({ host: '', port: '22', user: 'root' });
  const [showSshForm, setShowSshForm] = useState(false);
  const [fontSize, setFontSize] = useState(14);
  const [currentSessionId, setCurrentSessionId] = useState('');
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [showSessions, setShowSessions] = useState(false);

  const refreshSessions = async () => {
    const res = await apiRequest<SessionInfo[]>('/api/terminal/sessions');
    if (res.success && res.data) setSessions(res.data);
  };

  const connect = useCallback((sshParams?: SshTarget, joinSessionId?: string) => {
    if (wsRef.current) wsRef.current.close();
    if (terminalRef.current) terminalRef.current.dispose();

    const token = sessionStorage.getItem('nabiman_token') || '';
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    let wsUrl = `${protocol}//${window.location.host}/api/terminal?token=${encodeURIComponent(token)}`;

    // Join specific session or reconnect to stored session
    const sessionId = joinSessionId || (() => {
      const key = getSessionKey(sshParams ? 'ssh' : 'local', sshParams);
      return sessionStorage.getItem(key);
    })();
    if (sessionId) wsUrl += `&session_id=${encodeURIComponent(sessionId)}`;

    if (sshParams) {
      wsUrl += `&ssh_host=${encodeURIComponent(sshParams.host)}`;
      wsUrl += `&ssh_port=${encodeURIComponent(sshParams.port)}`;
      wsUrl += `&ssh_user=${encodeURIComponent(sshParams.user)}`;
    }

    const term = new Terminal({
      cursorBlink: true, cursorStyle: 'block', fontSize,
      fontFamily: "'Fira Code', 'Cascadia Code', 'JetBrains Mono', 'Menlo', monospace",
      fontWeight: '400', fontWeightBold: '700', lineHeight: 1.15, theme: THEME,
      allowProposedApi: true, scrollback: 10000, rightClickSelectsWord: true, convertEol: true,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());
    terminalRef.current = term;
    fitRef.current = fitAddon;

    if (termRef.current) { term.open(termRef.current); fitAddon.fit(); }

    term.attachCustomKeyEventHandler((ev: KeyboardEvent) => {
      if (ev.type !== 'keydown') return true;
      if (ev.ctrlKey && ev.shiftKey && ev.key === 'C') {
        const sel = term.getSelection();
        if (sel) navigator.clipboard.writeText(sel).catch(() => {});
        return false;
      }
      if (ev.ctrlKey && ev.shiftKey && ev.key === 'V') {
        navigator.clipboard.readText().then(text => {
          if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(text);
        }).catch(() => {});
        return false;
      }
      if (ev.ctrlKey && ev.key === '=') { setFontSize(s => Math.min(s + 1, 28)); return false; }
      if (ev.ctrlKey && ev.key === '-') { setFontSize(s => Math.max(s - 1, 8)); return false; }
      return true;
    });

    termRef.current?.addEventListener('contextmenu', (ev) => {
      ev.preventDefault();
      const sel = term.getSelection();
      if (sel) { navigator.clipboard.writeText(sel).catch(() => {}); term.clearSelection(); }
      else { navigator.clipboard.readText().then(text => {
        if (wsRef.current?.readyState === WebSocket.OPEN) wsRef.current.send(text);
      }).catch(() => {}); }
    });

    const ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';
    wsRef.current = ws;

    ws.onopen = () => {
      setConnected(true); setError('');
      setTimeout(() => {
        if (ws.readyState === WebSocket.OPEN) ws.send(`\x01RESIZE:${term.cols}:${term.rows}`);
      }, 150);
    };

    ws.onmessage = (event) => {
      if (typeof event.data === 'string' && event.data.startsWith('\x02SESSION:')) {
        const sid = event.data.slice(9);
        setCurrentSessionId(sid);
        // Store for reconnection
        const key = getSessionKey(sshParams ? 'ssh' : 'local', sshParams);
        sessionStorage.setItem(key, sid);
        return;
      }
      if (event.data instanceof ArrayBuffer) term.write(new Uint8Array(event.data));
      else term.write(event.data);
    };

    ws.onerror = () => { setError('WebSocket connection failed'); setConnected(false); };
    ws.onclose = () => { setConnected(false); };

    term.onData(data => { if (ws.readyState === WebSocket.OPEN) ws.send(data); });
    term.onBinary(data => {
      if (ws.readyState === WebSocket.OPEN) {
        const buf = new Uint8Array(data.length);
        for (let i = 0; i < data.length; i++) buf[i] = data.charCodeAt(i);
        ws.send(buf.buffer);
      }
    });
    term.onResize(({ cols, rows }) => {
      if (ws.readyState === WebSocket.OPEN) ws.send(`\x01RESIZE:${cols}:${rows}`);
    });
  }, [fontSize]);

  useEffect(() => {
    if (terminalRef.current) { terminalRef.current.options.fontSize = fontSize; fitRef.current?.fit(); }
  }, [fontSize]);

  // Ctrl + wheel zooms the terminal text (8..28). A plain wheel is left for
  // xterm to forward to the PTY: the session runs inside tmux with mouse mode
  // on, so the wheel scrolls tmux's history (past output) like a native
  // terminal. We intercept Ctrl in the capture phase so zoom isn't forwarded.
  useEffect(() => {
    const el = termRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return; // let xterm handle scroll → tmux
      e.preventDefault();
      e.stopPropagation();
      const delta = e.deltaY < 0 ? 1 : -1;
      setFontSize((s) => Math.min(28, Math.max(8, s + delta)));
    };
    el.addEventListener('wheel', onWheel, { passive: false, capture: true });
    return () => el.removeEventListener('wheel', onWheel, { capture: true } as EventListenerOptions);
  }, []);

  useEffect(() => {
    connect();
    const handleResize = () => fitRef.current?.fit();
    window.addEventListener('resize', handleResize);
    return () => { window.removeEventListener('resize', handleResize); wsRef.current?.close(); terminalRef.current?.dispose(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (isVisible && fitRef.current && terminalRef.current) {
      setTimeout(() => { fitRef.current?.fit(); terminalRef.current?.focus(); }, 50);
    }
  }, [isVisible]);

  useEffect(() => {
    if (externalSshTarget && externalSshTarget.host) {
      const target: SshTarget = { host: externalSshTarget.host, port: String(externalSshTarget.port), user: externalSshTarget.user };
      setSshTarget(target); setMode('ssh'); setShowSshForm(false);
      connect(target); onSshConnected?.();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [externalSshTarget]);

  const handleSshConnect = (e: React.FormEvent) => {
    e.preventDefault(); if (!sshTarget.host) return;
    setMode('ssh'); setShowSshForm(false); connect(sshTarget);
  };
  const handleLocalConnect = () => { setMode('local'); setShowSshForm(false); connect(); };
  const handleNewSession = () => {
    const key = getSessionKey(mode, mode === 'ssh' ? sshTarget : undefined);
    sessionStorage.removeItem(key);
    if (mode === 'ssh') connect(sshTarget); else connect();
  };
  const handleJoinSession = (sid: string, label: string) => {
    setShowSessions(false);
    if (label.startsWith('ssh:')) setMode('ssh'); else setMode('local');
    connect(undefined, sid);
  };
  const handleToggleShare = async (sid: string, shared: boolean) => {
    await apiPost(`/api/terminal/sessions/${sid}/share`, { shared: !shared });
    refreshSessions();
  };
  const handleSetTimeout = async (sid: string, timeout_secs: number) => {
    await apiPost(`/api/terminal/sessions/${sid}/timeout`, { timeout_secs });
    refreshSessions();
  };
  const timeoutLabel = (secs: number) => secs === 0 ? '∞' : secs === 1800 ? '30m' : secs === 3600 ? '1h' : secs === 86400 ? '24h' : `${secs}s`;

  return (
    <div className="panel terminal-panel">
      <div className="panel-header">
        <h2>{t('tab.terminal')} {mode === 'ssh' ? `(SSH: ${sshTarget.user}@${sshTarget.host})` : `(${t('terminal.local')})`}</h2>
        <div className="terminal-controls">
          <span
            className="terminal-hint"
            title="Ctrl+Shift+C/V: 복사·붙여넣기  ·  휠: 스크롤(과거 출력)  ·  Ctrl+휠: 확대/축소"
          >
            Ctrl+Shift+C/V
          </span>
          <span className={`status-badge ${connected ? 'up' : 'down'}`}>
            {connected ? t('terminal.connected') : t('terminal.disconnected')}
          </span>
          <button className="btn btn-primary btn-sm" onClick={() => setShowSshForm(!showSshForm)}>SSH</button>
          <button className="btn btn-secondary btn-sm" onClick={handleLocalConnect}>{t('terminal.local')}</button>
          <button className="btn btn-secondary btn-sm" onClick={handleNewSession} title="New session">+</button>
          <button className="btn btn-secondary btn-sm" onClick={() => { setShowSessions(!showSessions); if (!showSessions) refreshSessions(); }}
            title="Sessions">&#9776;</button>
        </div>
      </div>

      {showSshForm && (
        <form onSubmit={handleSshConnect} className="ssh-form">
          <input placeholder={t('terminal.hostPlaceholder')} value={sshTarget.host}
            onChange={e => setSshTarget({ ...sshTarget, host: e.target.value })} required />
          <input placeholder={t('terminal.portPlaceholder')} value={sshTarget.port}
            onChange={e => setSshTarget({ ...sshTarget, port: e.target.value })} style={{ width: 70 }} />
          <input placeholder={t('terminal.userPlaceholder')} value={sshTarget.user}
            onChange={e => setSshTarget({ ...sshTarget, user: e.target.value })} style={{ width: 100 }} />
          <button type="submit" className="btn btn-primary btn-sm">{t('common.connect')}</button>
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => setShowSshForm(false)}>{t('common.cancel')}</button>
        </form>
      )}

      {showSessions && (
        <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 6, padding: 12, marginBottom: 8 }}>
          <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>Active Sessions (tmux)</div>
          {sessions.length === 0 ? <div className="text-secondary" style={{ fontSize: 12 }}>No active sessions</div> : (
            <table className="data-table" style={{ fontSize: 12 }}>
              <thead><tr><th>Label</th><th>Owner</th><th>Users</th><th>Age</th><th>Timeout</th><th>Actions</th></tr></thead>
              <tbody>
                {sessions.map(s => (
                  <tr key={s.id} style={{ background: s.id === currentSessionId ? 'rgba(59,130,246,0.1)' : undefined }}>
                    <td><strong>{s.label}</strong></td>
                    <td>{s.owner}</td>
                    <td>{s.ws_count}</td>
                    <td>{formatAge(s.age_secs)}</td>
                    <td>
                      <select value={s.timeout_secs} onChange={e => handleSetTimeout(s.id, parseInt(e.target.value))}
                        style={{ fontSize: 11, padding: '2px 4px', background: 'var(--bg)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: 3 }}>
                        <option value={1800}>30m</option>
                        <option value={3600}>1h</option>
                        <option value={86400}>24h</option>
                        <option value={0}>Permanent</option>
                      </select>
                    </td>
                    <td>
                      {s.id !== currentSessionId && (
                        <button className="btn btn-primary btn-sm" style={{ marginRight: 4 }}
                          onClick={() => handleJoinSession(s.id, s.label)}>Join</button>
                      )}
                      <button className={`btn btn-sm ${s.shared ? 'btn-warning' : 'btn-secondary'}`}
                        onClick={() => handleToggleShare(s.id, s.shared)}>
                        {s.shared ? 'Shared' : 'Share'}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {error && <div className="message" style={{ borderColor: '#e74c3c44', background: '#e74c3c11' }}>{error}</div>}
      <div ref={termRef} className="terminal-container" />
    </div>
  );
}
