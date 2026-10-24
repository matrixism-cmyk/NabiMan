import React, { useEffect, useRef, useState } from 'react';
import Guacamole from '../../vendor/guacamole-common.js';
import { useT } from '../../shared/i18n';
import { rdpTicket } from './vncApi';
import AccountDictation from '../terminal/dictation/AccountDictation';

type Status = 'connecting' | 'connected' | 'disconnected';

/** An RDP console in the browser, over NabiMan's guacd tunnel. Guacamole draws
 *  the desktop and carries keyboard, mouse and clipboard — including copy/paste
 *  to and from the VM, natively (no VNC-style keystroke workaround). */
export default function RdpConsoleView({ target, label, canEdit, onEdit, onDelete, canGrant, onGrant, canShare, onShare, popout, share }: {
  target: string; label: string; canEdit?: boolean; onEdit?: () => void; onDelete?: () => void;
  canGrant?: boolean; onGrant?: () => void; canShare?: boolean; onShare?: () => void; popout?: boolean; share?: { token: string; viewOnly: boolean };
}) {
  const { t } = useT();
  const host = useRef<HTMLDivElement>(null);
  const client = useRef<any>(null);
  const keyboard = useRef<any>(null);
  const [status, setStatus] = useState<Status>('connecting');
  const [error, setError] = useState('');
  const [attempt, setAttempt] = useState(0);

  // Keep guacd's clipboard in step with the browser so Ctrl+V inside the VM
  // pastes what was copied locally.
  const pushClipboard = () => {
    const c = client.current; if (!c) return;
    navigator.clipboard.readText().then((text) => {
      if (!text) return;
      const stream = c.createClipboardStream('text/plain');
      const writer = new Guacamole.StringWriter(stream);
      writer.sendText(text); writer.sendEnd();
    }).catch(() => { /* no permission */ });
  };

  // Voice dictation typed into the VM. RDP accepts Unicode keystrokes, so Hangul
  // works too (unlike VNC).
  const key = (c: any, cp: number) => { const ks = cp < 0x100 ? cp : 0x01000000 + cp; c.sendKeyEvent(1, ks); c.sendKeyEvent(0, ks); };
  const dictate = (text: string) => { const c = client.current; if (!c) return; for (const ch of text) { const cp = ch.codePointAt(0) ?? 0; if (cp) key(c, cp); } };
  const dictateKeys = (keys: string) => {
    const c = client.current; if (!c) return;
    for (const ch of keys) {
      if (ch === '\r' || ch === '\n') { c.sendKeyEvent(1, 0xff0d); c.sendKeyEvent(0, 0xff0d); }
      else if (ch === '\x7f' || ch === '\b') { c.sendKeyEvent(1, 0xff08); c.sendKeyEvent(0, 0xff08); }
      else { const cp = ch.codePointAt(0) ?? 0; if (cp) key(c, cp); }
    }
  };

  useEffect(() => {
    let live = true;
    setStatus('connecting'); setError('');
    const el = host.current;
    if (!el) return undefined;
    const tunnel = new Guacamole.WebSocketTunnel(share ? `api/cshare/${encodeURIComponent(share.token)}/ws` : 'api/remote/rdp');
    const c = new Guacamole.Client(tunnel);
    client.current = c;
    const display = c.getDisplay();
    el.innerHTML = '';
    el.appendChild(display.getElement());

    const fit = () => {
      const w = display.getWidth(), h = display.getHeight();
      if (!w || !h) return;
      const scale = Math.min(el.clientWidth / w, el.clientHeight / h, 1);
      display.scale(scale);
    };
    display.onresize = fit;

    c.onstatechange = (state: number) => {
      if (!live) return;
      if (state === 3) { setStatus('connected'); fit(); }
      else if (state === 4 || state === 5) setStatus('disconnected');
    };
    c.onerror = (err: { message?: string }) => { if (live) { setStatus('disconnected'); setError(err?.message || t('remote.console.lost')); } };
    // Text copied inside the VM → the local clipboard.
    c.onclipboard = (stream: any, mimetype: string) => {
      if (mimetype.indexOf('text') !== 0) return;
      const reader = new Guacamole.StringReader(stream);
      let text = '';
      reader.ontext = (t2: string) => { text += t2; };
      reader.onend = () => { navigator.clipboard.writeText(text).catch(() => { /* ignore */ }); };
    };

    const dims = () => `GUAC_WIDTH=${Math.max(el.clientWidth || 1280, 640)}&GUAC_HEIGHT=${Math.max(el.clientHeight || 800, 480)}&GUAC_DPI=96`;
    if (share) {
      c.connect(dims());
    } else {
      rdpTicket(target).then((ticket) => {
        if (!live) return;
        c.connect(`target=${encodeURIComponent(target)}&ticket=${ticket}&${dims()}`);
      }).catch((e) => { if (live) { setStatus('disconnected'); setError(e instanceof Error ? e.message : t('common.error')); } });
    }

    // Input (skipped entirely for a view-only share).
    let kb: any = null;
    if (!share?.viewOnly) {
      const mouse = new Guacamole.Mouse(display.getElement());
      mouse.onmousedown = mouse.onmouseup = mouse.onmousemove = (state: any) => c.sendMouseState(state);
      kb = new Guacamole.Keyboard(el);
      kb.onkeydown = (keysym: number) => c.sendKeyEvent(1, keysym);
      kb.onkeyup = (keysym: number) => c.sendKeyEvent(0, keysym);
      keyboard.current = kb;
      el.addEventListener('mouseenter', pushClipboard);
      window.addEventListener('focus', pushClipboard);
    }

    return () => {
      live = false;
      el.removeEventListener('mouseenter', pushClipboard);
      window.removeEventListener('focus', pushClipboard);
      try { if (kb) { kb.onkeydown = null; kb.onkeyup = null; } } catch { /* ignore */ }
      try { c.disconnect(); } catch { /* already gone */ }
      client.current = null; keyboard.current = null;
    };
  }, [target, attempt]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="rw-view rw-console">
      <div className="rw-console-bar">
        <strong>{label}</strong>
        <span className="rw-proto-tag">RDP</span>
        {share?.viewOnly && <span className="rw-proto-tag">{t('remote.console.share.viewOnly')}</span>}
        <span className={`rw-console-state is-${status}`}>{t(`remote.console.${status}`)}</span>
        <span className="rw-console-spacer" />
        {status === 'connected' && !share?.viewOnly && <AccountDictation variant="bar" onText={dictate} onKeys={dictateKeys} />}
        {!popout && <button className="btn btn-secondary btn-sm" onClick={() => window.open(`/remote#rdp/${encodeURIComponent(target)}`, `rdp_${target}`, 'width=1300,height=900,menubar=no,toolbar=no,location=no')}>{t('remote.console.detach')}</button>}
        {!share?.viewOnly && <button className="btn btn-secondary btn-sm" onClick={() => { try { const c = client.current; c?.sendKeyEvent(1, 0xffe3); c?.sendKeyEvent(1, 0xffe9); c?.sendKeyEvent(1, 0xffff); c?.sendKeyEvent(0, 0xffff); c?.sendKeyEvent(0, 0xffe9); c?.sendKeyEvent(0, 0xffe3); } catch { /* not ready */ } }} disabled={status !== 'connected'}>Ctrl+Alt+Del</button>}
        {status === 'disconnected' && <button className="btn btn-primary btn-sm" onClick={() => setAttempt(a => a + 1)}>{t('remote.console.reconnect')}</button>}
        {canEdit && <button className="btn btn-secondary btn-sm" onClick={onEdit}>{t('common.edit')}</button>}
        {canGrant && <button className="btn btn-secondary btn-sm" onClick={onGrant}>{t('remote.console.access.btn')}</button>}
        {canShare && <button className="btn btn-secondary btn-sm" onClick={onShare}>{t('remote.console.share.btn')}</button>}
        {canEdit && <button className="btn btn-danger btn-sm" onClick={onDelete}>{t('common.delete')}</button>}
      </div>
      {error && <p role="alert" className="message rw-console-error" onClick={() => setError('')}>{error}</p>}
      <div ref={host} className="rw-console-screen rw-rdp-screen" tabIndex={0} />
    </div>
  );
}
