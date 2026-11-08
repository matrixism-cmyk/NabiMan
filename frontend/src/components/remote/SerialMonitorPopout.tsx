import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import 'xterm/css/xterm.css';
import { useT } from '../../shared/i18n';
import { channelName, FromHolder, FromMonitor, MonitorState, monitorTarget } from './serialMonitorChannel';
import './serial-monitor.css';

export { monitorTarget as serialMonitorTarget };

type Holder = MonitorState | 'none';
const ENDINGS = { none: '', lf: '\n', cr: '\r', crlf: '\r\n' } as const;
type Ending = keyof typeof ENDINGS;
/** How long to wait for the tab that holds the port before saying there is none. */
const HOLDER_WAIT_MS = 2500;
/** What the saved log keeps at most. */
const KEEP_CHARS = 4 << 20;

const stamp = () => {
  const d = new Date();
  const p = (n: number, w = 2) => String(n).padStart(w, '0');
  return `[${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}] `;
};

/**
 * A serial monitor in a window of its own (`/remote#serial/<session>/<device>`),
 * to keep beside other work. It does not open the port: the tab bridging the
 * board does, and talks to this window over a BroadcastChannel
 * (serialMonitorChannel.ts) — passing on what the board says and carrying out
 * what is typed here.
 */
export default function SerialMonitorPopout({ sid, name }: { sid: string; name: string }) {
  const { t } = useT();
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<Terminal | null>(null);
  const channel = useRef<BroadcastChannel | null>(null);
  const decoder = useRef(new TextDecoder());
  const atLineStart = useRef(true);
  const saved = useRef('');
  const held = useRef<string[]>([]);
  const [holder, setHolder] = useState<Holder>('connecting');
  const [baud, setBaud] = useState<number | null>(null);
  const [path, setPath] = useState('');
  const [paused, setPaused] = useState(false);
  const pausedRef = useRef(false);
  const [stamps, setStamps] = useState(false);
  const stampsRef = useRef(false);
  const [ending, setEnding] = useState<Ending>('lf');
  const [line, setLine] = useState('');
  const [history, setHistory] = useState<string[]>([]);
  const [at, setAt] = useState<number | null>(null);
  const [notice, setNotice] = useState('');

  useEffect(() => { document.title = `${name} · ${t('serial.monitorTitle')}`; }, [name, t]);
  // A window of its own: the page-wide floating buttons would cover the send bar.
  useEffect(() => {
    document.body.classList.add('is-serial-monitor');
    return () => document.body.classList.remove('is-serial-monitor');
  }, []);

  /** Text from the board onto the screen (and into the saved log), stamped per line when asked. */
  const show = useCallback((text: string) => {
    if (!text) return;
    let out = text;
    if (stampsRef.current) {
      let s = '';
      for (const ch of text) {
        if (atLineStart.current && ch !== '\r' && ch !== '\n') { s += stamp(); atLineStart.current = false; }
        s += ch;
        if (ch === '\n') atLineStart.current = true;
      }
      out = s;
    } else {
      atLineStart.current = /\n$/.test(text);
    }
    saved.current = (saved.current + out).slice(-KEEP_CHARS);
    if (pausedRef.current) held.current.push(out);
    else term.current?.write(out);
  }, []);

  useEffect(() => {
    const el = host.current;
    if (!el) return undefined;
    const tm = new Terminal({
      convertEol: true, scrollback: 20000, disableStdin: true, cursorBlink: false,
      fontFamily: 'Menlo, Consolas, "D2Coding", monospace', fontSize: 13,
      theme: { background: '#0b0d10' },
    });
    const fit = new FitAddon();
    tm.loadAddon(fit);
    tm.open(el);
    try { fit.fit(); } catch { /* not laid out yet */ }
    term.current = tm;
    const onResize = () => { try { fit.fit(); } catch { /* hidden */ } };
    window.addEventListener('resize', onResize);
    return () => { window.removeEventListener('resize', onResize); tm.dispose(); term.current = null; };
  }, []);

  useEffect(() => {
    if (typeof BroadcastChannel === 'undefined') { setHolder('none'); return undefined; }
    const ch = new BroadcastChannel(channelName(sid, name));
    channel.current = ch;
    let heard = false;
    let backlogShown = false;
    const hello = () => ch.postMessage({ type: 'hello' } satisfies FromMonitor);
    ch.onmessage = (e: MessageEvent<FromHolder>) => {
      const m = e.data;
      if (!m) return;
      heard = true;
      if (m.type === 'state') { setHolder(m.state); setBaud(m.baud); setPath(m.path); }
      else if (m.type === 'backlog') {
        // Once: a holder answering again (a second monitor said hello) is not news here.
        if (!backlogShown && m.text) { show(m.text); show(`\r\n\x1b[2m── ${t('serial.monitorLive')} ──\x1b[0m\r\n`); }
        backlogShown = true;
      } else if (m.type === 'data') show(decoder.current.decode(m.bytes, { stream: true }));
      else if (m.type === 'done' && !m.ok) setNotice(m.what === 'send' ? t('serial.monitorSendFailed') : `${t(`serial.${m.what}`)}: ${m.error || t('common.error')}`);
    };
    hello();
    // No tab holds the port yet (or it was closed): keep asking, quietly.
    const wait = setTimeout(() => { if (!heard) setHolder('none'); }, HOLDER_WAIT_MS);
    const again = setInterval(() => { if (!heard) hello(); }, 3000);
    return () => { clearTimeout(wait); clearInterval(again); ch.close(); channel.current = null; };
  }, [sid, name, show, t]);

  const post = (m: FromMonitor) => { setNotice(''); channel.current?.postMessage(m); };
  const send = () => {
    const bytes = new TextEncoder().encode(line + ENDINGS[ending]);
    if (!bytes.length) return;
    post({ type: 'send', bytes });
    if (line) setHistory((h) => [...h.filter((x) => x !== line), line].slice(-50));
    setLine('');
    setAt(null);
  };
  const onKey = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') { e.preventDefault(); send(); return; }
    if (e.key === 'ArrowUp' && history.length) {
      e.preventDefault();
      const i = at === null ? history.length - 1 : Math.max(0, at - 1);
      setAt(i); setLine(history[i]);
    } else if (e.key === 'ArrowDown' && at !== null) {
      e.preventDefault();
      const i = at + 1;
      if (i >= history.length) { setAt(null); setLine(''); } else { setAt(i); setLine(history[i]); }
    }
  };
  const togglePause = () => {
    const next = !paused;
    pausedRef.current = next;
    setPaused(next);
    if (!next && held.current.length) { term.current?.write(held.current.join('')); held.current = []; }
  };
  const toggleStamps = () => { stampsRef.current = !stamps; setStamps(!stamps); };
  const clear = () => { term.current?.clear(); saved.current = ''; held.current = []; };
  const save = () => {
    // Without ANSI colour codes: a plain text log.
    // eslint-disable-next-line no-control-regex
    const text = saved.current.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, '');
    const url = URL.createObjectURL(new Blob([text], { type: 'text/plain;charset=utf-8' }));
    const a = document.createElement('a');
    a.href = url;
    a.download = `${name}-${new Date().toISOString().replace(/[:.]/g, '-')}.log`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  const up = holder === 'connected';

  return (
    <div className="serial-monitor-popout">
      <header className="serial-monitor-bar">
        <strong>{name}</strong>
        <span className={`rw-console-state is-${up ? 'connected' : 'disconnected'}`}>
          {holder === 'none' ? t('serial.monitorNoHolder') : t(`serial.status.${holder}`)}
        </span>
        {baud && <span className="text-secondary">{baud}</span>}
        {path && <code className="serial-monitor-path" title={path}>{path}</code>}
        <span className="serial-monitor-gap" />
        <button className={`btn btn-sm ${paused ? 'btn-warning' : 'btn-secondary'}`} onClick={togglePause}>{t(paused ? 'serial.monitorResume' : 'serial.monitorPause')}</button>
        <button className={`btn btn-sm ${stamps ? 'btn-primary' : 'btn-secondary'}`} onClick={toggleStamps} title={t('serial.monitorStampsHint')}>{t('serial.monitorStamps')}</button>
        <button className="btn btn-secondary btn-sm" onClick={clear}>{t('serial.monitorClear')}</button>
        <button className="btn btn-secondary btn-sm" onClick={save}>{t('serial.monitorSave')}</button>
        <button className="btn btn-secondary btn-sm" disabled={!up} onClick={() => post({ type: 'reset' })} title={t('serial.resetHint')}>{t('serial.reset')}</button>
        <button className="btn btn-secondary btn-sm" disabled={!up} onClick={() => post({ type: 'bootloader' })} title={t('serial.bootloaderHint')}>{t('serial.bootloader')}</button>
      </header>
      {holder === 'none' && <p role="status" className="serial-monitor-note">{t('serial.monitorNoHolderHint')}</p>}
      {notice && <p role="alert" className="serial-monitor-note is-error">{notice}</p>}
      <div className="serial-monitor-screen" ref={host} data-testid="serial-monitor-screen" />
      <form className="serial-monitor-send" onSubmit={(e) => { e.preventDefault(); send(); }}>
        <input className="filter-input" value={line} disabled={!up} placeholder={t('serial.monitorSendPlaceholder')}
          aria-label={t('serial.monitorSendPlaceholder')} onChange={(e) => { setLine(e.target.value); setAt(null); }} onKeyDown={onKey} />
        <select className="select-input" value={ending} onChange={(e) => setEnding(e.target.value as Ending)} aria-label={t('serial.monitorEnding')}>
          <option value="none">{t('serial.monitorEndingNone')}</option>
          <option value="lf">LF (\n)</option>
          <option value="cr">CR (\r)</option>
          <option value="crlf">CRLF (\r\n)</option>
        </select>
        <button type="submit" className="btn btn-primary btn-sm" disabled={!up}>{t('serial.monitorSend')}</button>
      </form>
    </div>
  );
}
