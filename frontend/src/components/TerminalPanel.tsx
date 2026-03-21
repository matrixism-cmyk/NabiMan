import React, { useEffect, useRef, useState, useCallback } from 'react';
import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import 'xterm/css/xterm.css';

interface SshTarget {
  host: string;
  port: string;
  user: string;
}

const THEME = {
  background: '#0f1923',
  foreground: '#e0e6ed',
  cursor: '#3b82f6',
  cursorAccent: '#0f1923',
  selectionBackground: '#3b82f655',
  selectionForeground: '#ffffff',
  selectionInactiveBackground: '#3b82f633',
  black: '#1a2634',
  red: '#e74c3c',
  green: '#2ecc71',
  yellow: '#f39c12',
  blue: '#3b82f6',
  magenta: '#9b59b6',
  cyan: '#1abc9c',
  white: '#e0e6ed',
  brightBlack: '#4a5a6a',
  brightRed: '#ff6b6b',
  brightGreen: '#6bff8d',
  brightYellow: '#ffd93d',
  brightBlue: '#6bb3ff',
  brightMagenta: '#c471ed',
  brightCyan: '#45e6c6',
  brightWhite: '#ffffff',
};

export default function TerminalPanel() {
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

  const connect = useCallback((sshParams?: SshTarget) => {
    if (wsRef.current) wsRef.current.close();
    if (terminalRef.current) terminalRef.current.dispose();

    const token = sessionStorage.getItem('nabiman_token') || '';
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    let wsUrl = `${protocol}//${window.location.host}/api/terminal?token=${encodeURIComponent(token)}`;

    if (sshParams) {
      wsUrl += `&ssh_host=${encodeURIComponent(sshParams.host)}`;
      wsUrl += `&ssh_port=${encodeURIComponent(sshParams.port)}`;
      wsUrl += `&ssh_user=${encodeURIComponent(sshParams.user)}`;
    }

    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: 'block',
      fontSize,
      fontFamily: "'Fira Code', 'Cascadia Code', 'JetBrains Mono', 'Menlo', monospace",
      fontWeight: '400',
      fontWeightBold: '700',
      lineHeight: 1.15,
      letterSpacing: 0,
      theme: THEME,
      allowProposedApi: true,
      scrollback: 10000,
      rightClickSelectsWord: true,
      convertEol: true,
    });

    const fitAddon = new FitAddon();
    const webLinksAddon = new WebLinksAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(webLinksAddon);

    terminalRef.current = term;
    fitRef.current = fitAddon;

    if (termRef.current) {
      term.open(termRef.current);
      fitAddon.fit();
    }

    // Clipboard: Ctrl+Shift+C = copy, Ctrl+Shift+V = paste
    term.attachCustomKeyEventHandler((ev: KeyboardEvent) => {
      if (ev.type !== 'keydown') return true;

      // Ctrl+Shift+C → copy selection
      if (ev.ctrlKey && ev.shiftKey && ev.key === 'C') {
        const sel = term.getSelection();
        if (sel) {
          navigator.clipboard.writeText(sel).catch(() => {});
        }
        return false;
      }

      // Ctrl+Shift+V → paste from clipboard
      if (ev.ctrlKey && ev.shiftKey && ev.key === 'V') {
        navigator.clipboard.readText().then((text) => {
          if (wsRef.current?.readyState === WebSocket.OPEN) {
            wsRef.current.send(text);
          }
        }).catch(() => {});
        return false;
      }

      // Ctrl+Plus/Minus → font size
      if (ev.ctrlKey && ev.key === '=') {
        setFontSize(s => Math.min(s + 1, 28));
        return false;
      }
      if (ev.ctrlKey && ev.key === '-') {
        setFontSize(s => Math.max(s - 1, 8));
        return false;
      }

      return true;
    });

    // Right-click context menu paste
    termRef.current?.addEventListener('contextmenu', (ev) => {
      ev.preventDefault();
      const sel = term.getSelection();
      if (sel) {
        navigator.clipboard.writeText(sel).catch(() => {});
        term.clearSelection();
      } else {
        navigator.clipboard.readText().then((text) => {
          if (wsRef.current?.readyState === WebSocket.OPEN) {
            wsRef.current.send(text);
          }
        }).catch(() => {});
      }
    });

    const ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';
    wsRef.current = ws;

    ws.onopen = () => {
      setConnected(true);
      setError('');
      ws.send(`\x01RESIZE:${term.cols}:${term.rows}`);
    };

    ws.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        term.write(new Uint8Array(event.data));
      } else {
        term.write(event.data);
      }
    };

    ws.onerror = () => {
      setError('WebSocket connection failed');
      setConnected(false);
    };

    ws.onclose = () => {
      setConnected(false);
      term.write('\r\n\x1b[31m--- Session ended ---\x1b[0m\r\n');
    };

    term.onData((data) => {
      if (ws.readyState === WebSocket.OPEN) ws.send(data);
    });

    term.onBinary((data) => {
      if (ws.readyState === WebSocket.OPEN) {
        const buf = new Uint8Array(data.length);
        for (let i = 0; i < data.length; i++) buf[i] = data.charCodeAt(i);
        ws.send(buf.buffer);
      }
    });

    term.onResize(({ cols, rows }) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(`\x01RESIZE:${cols}:${rows}`);
      }
    });
  }, [fontSize]);

  // Update font size on existing terminal
  useEffect(() => {
    if (terminalRef.current) {
      terminalRef.current.options.fontSize = fontSize;
      fitRef.current?.fit();
    }
  }, [fontSize]);

  useEffect(() => {
    connect();

    const handleResize = () => fitRef.current?.fit();
    window.addEventListener('resize', handleResize);

    return () => {
      window.removeEventListener('resize', handleResize);
      wsRef.current?.close();
      terminalRef.current?.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleSshConnect = (e: React.FormEvent) => {
    e.preventDefault();
    if (!sshTarget.host) return;
    setMode('ssh');
    setShowSshForm(false);
    connect(sshTarget);
  };

  const handleLocalConnect = () => {
    setMode('local');
    setShowSshForm(false);
    connect();
  };

  return (
    <div className="panel terminal-panel">
      <div className="panel-header">
        <h2>Terminal {mode === 'ssh' ? `(SSH: ${sshTarget.user}@${sshTarget.host})` : '(Local)'}</h2>
        <div className="terminal-controls">
          <span className="terminal-hint">Ctrl+Shift+C/V: Copy/Paste</span>
          <span className="terminal-hint">Ctrl+/-: Font Size</span>
          <span className={`status-badge ${connected ? 'up' : 'down'}`}>
            {connected ? 'Connected' : 'Disconnected'}
          </span>
          <button className="btn btn-primary btn-sm" onClick={() => setShowSshForm(!showSshForm)}>
            SSH
          </button>
          <button className="btn btn-secondary btn-sm" onClick={handleLocalConnect}>
            Local
          </button>
        </div>
      </div>

      {showSshForm && (
        <form onSubmit={handleSshConnect} className="ssh-form">
          <input
            placeholder="Host (IP or hostname)"
            value={sshTarget.host}
            onChange={e => setSshTarget({ ...sshTarget, host: e.target.value })}
            required
          />
          <input
            placeholder="Port"
            value={sshTarget.port}
            onChange={e => setSshTarget({ ...sshTarget, port: e.target.value })}
            style={{ width: 70 }}
          />
          <input
            placeholder="User"
            value={sshTarget.user}
            onChange={e => setSshTarget({ ...sshTarget, user: e.target.value })}
            style={{ width: 100 }}
          />
          <button type="submit" className="btn btn-primary btn-sm">Connect</button>
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => setShowSshForm(false)}>Cancel</button>
        </form>
      )}

      {error && <div className="message" style={{ borderColor: '#e74c3c44', background: '#e74c3c11' }}>{error}</div>}
      <div ref={termRef} className="terminal-container" />
    </div>
  );
}
