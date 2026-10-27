import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useT } from '../../shared/i18n';
import { serialTicket } from './vncApi';

const BAUDS = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];
const REATTACH_EVERY_MS = 1000;
type Status = 'idle' | 'connecting' | 'connected';

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));
/** USB vendor:product, so the same board is found again after it re-enumerates. */
const portKey = (p: any) => { const i = p?.getInfo?.() ?? {}; return `${i.usbVendorId ?? ''}:${i.usbProductId ?? ''}`; };
/** Espressif's built-in USB-Serial/JTAG (ESP32-C3/S3/C6…): DTR is the boot pin, RTS the reset. */
const isUsbJtag = (p: any) => { const i = p?.getInfo?.() ?? {}; return i.usbVendorId === 0x303a && i.usbProductId === 0x1001; };

/**
 * Bridge a serial device on the operator's own machine (an ESP32 on USB, say)
 * to the server: the browser holds the real port over the Web Serial API and
 * relays its bytes to a NabiMan WebSocket, which exposes a PTY the server's
 * tools open — so arduino-cli, esptool or a monitor run server-side (and the
 * server-side Claude) reach the board as if it were local.
 *
 * The socket (and so the server PTY) outlives the port: boards on native USB
 * (ESP32-C3 SuperMini etc.) drop off the bus whenever the chip resets, so a lost
 * port is reopened when the same device comes back instead of ending the bridge.
 */
export default function SerialView() {
  const { t } = useT();
  const supported = typeof navigator !== 'undefined' && 'serial' in navigator;
  const [name, setName] = useState('esp32');
  const [baud, setBaud] = useState(115200);
  const [status, setStatus] = useState<Status>('idle');
  const [portUp, setPortUp] = useState(false);
  const [path, setPath] = useState('');
  const [error, setError] = useState('');
  const [log, setLog] = useState('');
  const port = useRef<any>(null);
  const ws = useRef<WebSocket | null>(null);
  const reader = useRef<any>(null);
  const writer = useRef<any>(null);
  const pumping = useRef<Promise<void> | null>(null);
  const deviceKey = useRef('');
  const baudRef = useRef(baud);
  const stopping = useRef(false);
  const reattaching = useRef(false);
  const reattachTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const logEnd = useRef<HTMLDivElement>(null);

  const append = useCallback((s: string) => setLog((l) => (l.length > 20000 ? l.slice(-16000) : l) + s), []);

  const cleanup = useCallback(async () => {
    stopping.current = true;
    if (reattachTimer.current) clearInterval(reattachTimer.current);
    reattachTimer.current = null;
    const socket = ws.current; const p = port.current;
    ws.current = port.current = null;
    try { socket?.close(); } catch { /* ignore */ }
    try { await reader.current?.cancel(); } catch { /* ignore */ }
    try { await pumping.current; } catch { /* ignore */ }
    try { writer.current?.releaseLock(); } catch { /* ignore */ }
    reader.current = writer.current = pumping.current = null;
    try { await p?.close(); } catch { /* ignore */ }
    setPortUp(false);
  }, []);
  useEffect(() => () => { void cleanup(); }, [cleanup]);
  useEffect(() => { logEnd.current?.scrollIntoView(); }, [log]);

  /** Read the port until it goes away, relaying to the socket; then wait for it to come back. */
  const pump = useCallback(async (p: any) => {
    const dec = new TextDecoder();
    let reason = '';
    // A non-fatal error (framing, overrun) hands back a fresh readable; a lost device leaves none.
    while (p.readable && port.current === p) {
      const rd = p.readable.getReader();
      reader.current = rd;
      try {
        for (;;) {
          const { value, done } = await rd.read();
          if (done) break;
          if (!value?.length) continue;
          if (ws.current?.readyState === WebSocket.OPEN) ws.current.send(value);
          append(dec.decode(value, { stream: true }));
        }
      } catch (e) {
        reason = errText(e);
      } finally {
        try { rd.releaseLock(); } catch { /* ignore */ }
      }
    }
    if (stopping.current || port.current !== p) return;
    try { writer.current?.releaseLock(); } catch { /* ignore */ }
    reader.current = writer.current = port.current = null;
    try { await p.close(); } catch { /* already gone */ }
    setPortUp(false);
    append(`\r\n[${t('serial.portLost', { reason: reason || '-' })}]\r\n`);
  }, [append, t]);

  const openPort = useCallback(async (p: any) => {
    await p.open({ baudRate: baudRef.current });
    if (!p.readable || !p.writable) {
      try { await p.close(); } catch { /* ignore */ }
      throw new Error(t('serial.portGone'));
    }
    port.current = p;
    writer.current = p.writable.getWriter();
    setPortUp(true);
    pumping.current = pump(p);
  }, [pump, t]);

  /** The board reset and re-enumerated: find it among the ports this page may use and reopen it. */
  const reattach = useCallback(async () => {
    if (stopping.current || port.current || reattaching.current) return;
    reattaching.current = true;
    try {
      const ports: any[] = await (navigator as any).serial.getPorts();
      for (const p of ports) {
        if (portKey(p) !== deviceKey.current || p.readable) continue;
        try {
          await openPort(p);
          append(`[${t('serial.portBack')}]\r\n`);
          return;
        } catch { /* still enumerating, or held by another app — next tick */ }
      }
    } catch { /* ignore */ } finally {
      reattaching.current = false;
    }
  }, [append, openPort, t]);

  const connect = async () => {
    setError('');
    if (!supported) { setError(t('serial.unsupported')); return; }
    stopping.current = false;
    baudRef.current = baud;
    try {
      setStatus('connecting');
      const chosen = await (navigator as any).serial.requestPort();
      deviceKey.current = portKey(chosen);
      await openPort(chosen);
      reattachTimer.current = setInterval(() => { void reattach(); }, REATTACH_EVERY_MS);
      const ticket = await serialTicket(name);
      const socket = new WebSocket(`${window.location.origin.replace('http', 'ws')}/api/remote/serial?serial=${encodeURIComponent(name)}&ticket=${ticket}`);
      socket.binaryType = 'arraybuffer';
      ws.current = socket;
      socket.onmessage = (e) => {
        if (typeof e.data === 'string') { try { setPath(JSON.parse(e.data).path || ''); } catch { /* not the path */ } return; }
        writer.current?.write(new Uint8Array(e.data)).catch(() => { /* port gone; reattach picks it up */ });
      };
      socket.onclose = (e) => {
        if (!stopping.current) setError(t('serial.wsClosed', { code: e.code, reason: e.reason || '-' }));
        setStatus('idle'); setPath('');
        void cleanup();
      };
      socket.onerror = () => setError(t('serial.wsError'));
      socket.onopen = () => setStatus('connected');
    } catch (e) {
      setStatus('idle');
      setError(errText(e));
      void cleanup();
    }
  };

  const disconnect = () => { void cleanup(); setStatus('idle'); setPath(''); };

  /** Drive DTR/RTS (browser-side) through the given [dtr, rts, holdMs] steps. */
  const signals = async (steps: Array<[boolean, boolean, number]>) => {
    const p = port.current; if (!p) return;
    try {
      for (const [dtr, rts, ms] of steps) {
        await p.setSignals({ dataTerminalReady: dtr, requestToSend: rts });
        if (ms) await sleep(ms);
      }
    } catch { /* unsupported, or the board already dropped off USB */ }
  };
  // Reset with the boot pin high: the board runs its firmware.
  const reset = () => signals([[false, true, 120], [false, false, 0]]);
  // Reset with the boot pin low: the ROM bootloader waits for esptool.
  const bootloader = () => signals(isUsbJtag(port.current)
    ? [[false, false, 100], [true, false, 100], [false, true, 100], [false, false, 0]]
    : [[false, true, 120], [true, false, 60], [false, false, 0]]);

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
            {status === 'connected' && portUp && <>
              <button className="btn btn-secondary btn-sm" onClick={reset} title={t('serial.resetHint')}>{t('serial.reset')}</button>
              <button className="btn btn-secondary btn-sm" onClick={bootloader} title={t('serial.bootloaderHint')}>{t('serial.bootloader')}</button>
            </>}
            <span className={`rw-console-state is-${status === 'connected' && portUp ? 'connected' : 'disconnected'}`}>
              {status === 'connected' && !portUp ? t('serial.status.waiting') : t(`serial.status.${status}`)}
            </span>
          </div>
          {status === 'connected' && !portUp && <p className="text-secondary">{t('serial.waitingHint')}</p>}
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
