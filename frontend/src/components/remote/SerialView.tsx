import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useT } from '../../shared/i18n';
import { serialTicket } from './vncApi';

const BAUDS = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];
type Status = 'idle' | 'connecting' | 'connected';

/**
 * Bridge a serial device on the operator's own machine (an ESP32 on USB, say)
 * to the server: the browser holds the real port over the Web Serial API and
 * relays its bytes to a NabiMan WebSocket, which exposes a PTY the server's
 * tools open — so arduino-cli, esptool or a monitor run server-side (and the
 * server-side Claude) reach the board as if it were local.
 */
export default function SerialView() {
  const { t } = useT();
  const supported = typeof navigator !== 'undefined' && 'serial' in navigator;
  const [name, setName] = useState('esp32');
  const [baud, setBaud] = useState(115200);
  const [status, setStatus] = useState<Status>('idle');
  const [path, setPath] = useState('');
  const [error, setError] = useState('');
  const [log, setLog] = useState('');
  const port = useRef<any>(null);
  const ws = useRef<WebSocket | null>(null);
  const reader = useRef<any>(null);
  const writer = useRef<any>(null);
  const logEnd = useRef<HTMLDivElement>(null);

  const cleanup = useCallback(() => {
    try { reader.current?.cancel(); } catch { /* ignore */ }
    try { writer.current?.releaseLock(); } catch { /* ignore */ }
    try { ws.current?.close(); } catch { /* ignore */ }
    try { port.current?.close(); } catch { /* ignore */ }
    reader.current = writer.current = ws.current = port.current = null;
  }, []);
  useEffect(() => cleanup, [cleanup]);
  useEffect(() => { logEnd.current?.scrollIntoView(); }, [log]);

  const append = (s: string) => setLog((l) => (l.length > 20000 ? l.slice(-16000) : l) + s);

  const connect = async () => {
    setError('');
    if (!supported) { setError(t('serial.unsupported')); return; }
    try {
      setStatus('connecting');
      const chosen = await (navigator as any).serial.requestPort();
      await chosen.open({ baudRate: baud });
      port.current = chosen;
      const ticket = await serialTicket(name);
      const socket = new WebSocket(`${window.location.origin.replace('http', 'ws')}/api/remote/serial?serial=${encodeURIComponent(name)}&ticket=${ticket}`);
      socket.binaryType = 'arraybuffer';
      ws.current = socket;
      writer.current = chosen.writable.getWriter();
      socket.onmessage = (e) => {
        if (typeof e.data === 'string') { try { setPath(JSON.parse(e.data).path || ''); } catch { /* not the path */ } return; }
        writer.current?.write(new Uint8Array(e.data)).catch(() => { /* port gone */ });
      };
      socket.onclose = () => { setStatus('idle'); setPath(''); cleanup(); };
      socket.onerror = () => setError(t('serial.wsError'));
      socket.onopen = () => {
        setStatus('connected');
        (async () => {
          const rd = chosen.readable.getReader();
          reader.current = rd;
          const dec = new TextDecoder();
          try {
            for (;;) {
              const { value, done } = await rd.read();
              if (done) break;
              if (value && value.length) { socket.send(value); append(dec.decode(value, { stream: true })); }
            }
          } catch { /* reader cancelled */ }
        })();
      };
    } catch (e) {
      setStatus('idle');
      setError(e instanceof Error ? e.message : String(e));
      cleanup();
    }
  };

  const disconnect = () => { cleanup(); setStatus('idle'); setPath(''); };

  // Pulse DTR/RTS to reset an ESP32 into its running firmware (browser-side).
  const reset = async () => {
    const p = port.current; if (!p) return;
    try {
      await p.setSignals({ dataTerminalReady: false, requestToSend: true });
      await new Promise((r) => setTimeout(r, 120));
      await p.setSignals({ dataTerminalReady: true, requestToSend: false });
    } catch { /* unsupported */ }
  };

  return (
    <div className="rw-view rw-view-scroll">
      <div className="rw-section">
        <div className="rw-section-head"><h3>{t('remote.serialTab')}</h3></div>
        <p className="text-secondary">{t('serial.intro')}</p>
        {!supported && <p role="alert" className="message">{t('serial.unsupported')}</p>}

        <div className="rw-form" style={{ maxWidth: 520 }}>
          <div className="form-group"><label>{t('serial.name')}</label>
            <input className="filter-input" value={name} disabled={status !== 'idle'}
              onChange={(e) => setName(e.target.value.replace(/[^a-zA-Z0-9_-]/g, ''))} placeholder="esp32" /></div>
          <div className="form-group"><label>{t('serial.baud')}</label>
            <select className="select-input" value={baud} disabled={status !== 'idle'} onChange={(e) => setBaud(Number(e.target.value))}>
              {BAUDS.map((b) => <option key={b} value={b}>{b}</option>)}
            </select></div>
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            {status === 'idle'
              ? <button className="btn btn-primary btn-sm" onClick={connect} disabled={!supported}>{t('serial.connect')}</button>
              : <button className="btn btn-danger btn-sm" onClick={disconnect}>{t('serial.disconnect')}</button>}
            {status === 'connected' && <button className="btn btn-secondary btn-sm" onClick={reset} title={t('serial.resetHint')}>{t('serial.reset')}</button>}
            <span className={`rw-console-state is-${status === 'connected' ? 'connected' : 'disconnected'}`}>{t(`serial.status.${status}`)}</span>
          </div>
          {error && <p role="alert" className="message" onClick={() => setError('')}>{error}</p>}
        </div>

        {path && (
          <div className="rw-section" style={{ marginTop: 14 }}>
            <p className="text-secondary" style={{ marginBottom: 6 }}>{t('serial.pathLabel')}</p>
            <div style={{ display: 'flex', gap: 8 }}>
              <input readOnly className="filter-input" value={path} style={{ flex: 1, fontFamily: 'monospace' }} onFocus={(e) => e.target.select()} />
              <button className="btn btn-secondary btn-sm" onClick={() => navigator.clipboard.writeText(path).catch(() => { /* ignore */ })}>{t('common.copy')}</button>
            </div>
            <small className="text-secondary">{t('serial.pathHint')}</small>
          </div>
        )}

        {status === 'connected' && (
          <div className="rw-section" style={{ marginTop: 14 }}>
            <p className="text-secondary" style={{ marginBottom: 6 }}>{t('serial.monitor')}</p>
            <pre className="serial-monitor">{log}<div ref={logEnd} /></pre>
          </div>
        )}
      </div>
    </div>
  );
}
