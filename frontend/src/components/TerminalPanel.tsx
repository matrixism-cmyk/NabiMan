import React, { useEffect, useRef, useState } from 'react';
import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import 'xterm/css/xterm.css';

export default function TerminalPanel() {
  const termRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState('');

  const connect = () => {
    if (wsRef.current) {
      wsRef.current.close();
    }
    if (terminalRef.current) {
      terminalRef.current.dispose();
    }

    const token = sessionStorage.getItem('nabiman_token') || '';
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/terminal?token=${encodeURIComponent(token)}`;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'Fira Code', 'Cascadia Code', 'JetBrains Mono', monospace",
      theme: {
        background: '#0f1923',
        foreground: '#e0e6ed',
        cursor: '#3b82f6',
        selectionBackground: '#3b82f644',
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
      },
      allowProposedApi: true,
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

    const ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';
    wsRef.current = ws;

    ws.onopen = () => {
      setConnected(true);
      setError('');
      // Send initial size
      const cols = term.cols;
      const rows = term.rows;
      ws.send(`\x01RESIZE:${cols}:${rows}`);
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

    // Send user input to WebSocket
    term.onData((data) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(data);
      }
    });

    // Handle resize
    term.onResize(({ cols, rows }) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(`\x01RESIZE:${cols}:${rows}`);
      }
    });
  };

  useEffect(() => {
    connect();

    const handleResize = () => {
      if (fitRef.current) {
        fitRef.current.fit();
      }
    };
    window.addEventListener('resize', handleResize);

    return () => {
      window.removeEventListener('resize', handleResize);
      if (wsRef.current) wsRef.current.close();
      if (terminalRef.current) terminalRef.current.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="panel terminal-panel">
      <div className="panel-header">
        <h2>Terminal</h2>
        <div className="terminal-controls">
          <span className={`status-badge ${connected ? 'up' : 'down'}`}>
            {connected ? 'Connected' : 'Disconnected'}
          </span>
          <button className="btn btn-secondary btn-sm" onClick={connect}>
            {connected ? 'Reconnect' : 'Connect'}
          </button>
        </div>
      </div>
      {error && <div className="message" style={{ borderColor: '#e74c3c44', background: '#e74c3c11' }}>{error}</div>}
      <div ref={termRef} className="terminal-container" />
    </div>
  );
}
