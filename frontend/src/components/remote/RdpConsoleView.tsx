import React, { useEffect, useMemo, useRef, useState } from 'react';
import Guacamole from '../../vendor/guacamole-common.js';
import { useT } from '../../shared/i18n';
import { useFullscreen } from '../../shared/ui/useFullscreen';
import { useChromeHidden } from '../../shared/ui/useChromeHidden';
import { remoteKey, ALT_R } from './rdpKeys';
import { describeFault, reconnectDelay, RECONNECT_MAX } from './consoleFault';
import { useConsoleView, keepAlive, fixedSize, ConsoleShare } from './consoleView';
import ConsoleSettingsModal from './ConsoleSettingsModal';
import ConsoleDictation from './ConsoleDictation';
import { useConsoleKeyboard } from './ConsoleKeyboard';
import ConsoleAltUserModal from './ConsoleAltUserModal';
import RdpDrivePanel from './RdpDrivePanel';
import { rdpTicket, consoleCreds } from './vncApi';
import { rdpTypist } from './rdpTypist';
import { saveBlob } from './consoleDownload';
import { rdpZoom } from './rdpZoom';

type Status = 'connecting' | 'connected' | 'disconnected';

/** An RDP console in the browser, over NabiMan's guacd tunnel. Guacamole draws
 *  the desktop and carries keyboard, mouse and clipboard — including copy/paste
 *  to and from the VM, natively (no VNC-style keystroke workaround). */
export default function RdpConsoleView({ target, label, canEdit, onEdit, onDelete, canGrant, onGrant, canShare, onShare, popout, share }: {
  target: string; label: string; canEdit?: boolean; onEdit?: () => void; onDelete?: () => void;
  canGrant?: boolean; onGrant?: () => void; canShare?: boolean; onShare?: () => void; popout?: boolean; share?: ConsoleShare;
}) {
  const { t } = useT();
  const host = useRef<HTMLDivElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const zoomer = useRef<ReturnType<typeof rdpZoom> | null>(null);
  const [zoom, setZoom] = useState(1);
  const root = useRef<HTMLDivElement>(null);
  const client = useRef<any>(null);
  const keyboard = useRef<any>(null);
  const filesystem = useRef<any>(null);
  const fitRef = useRef<(() => void) | undefined>(undefined);
  const resizeRef = useRef<(() => void) | undefined>(undefined);
  const retries = useRef(0);
  const reconnectTimer = useRef<number | undefined>(undefined);
  const { full, toggle } = useFullscreen(root, () => { fitRef.current?.(); resizeRef.current?.(); });
  const [folded, setFolded] = useChromeHidden();
  const [status, setStatus] = useState<Status>('connecting');
  const [error, setError] = useState('');
  const [attempt, setAttempt] = useState(0);
  const [upload, setUpload] = useState<{ name: string; pct: number; fail?: boolean; got?: boolean } | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [driveReady, setDriveReady] = useState(false);
  const [drivePanel, setDrivePanel] = useState(false);
  const [cview, setCview] = useConsoleView();
  const keepaliveRef = useRef(true);
  keepaliveRef.current = keepAlive(cview);
  const fixed = fixedSize(cview);
  const fixedKey = fixed ? `${fixed.w}x${fixed.h}` : '';
  const [showSettings, setShowSettings] = useState(false);
  // Alternate account for this window (multi-user VM); held in memory only.
  const altCreds = useRef<{ user: string; password: string } | null>(null);
  const [altUser, setAltUser] = useState<string | null>(null);
  const [askUser, setAskUser] = useState(false);

  // Drag a file onto the VM to copy it in, via guacd's redirected drive (it
  // lands on the "NabiMan" drive inside Windows). View-only shares cannot.
  const sendFile = (file: File) => {
    const fs = filesystem.current;
    if (!fs) { setError(t('remote.console.drive.pending')); return; }
    const stream = fs.createOutputStream(file.type || 'application/octet-stream', `/${file.name}`);
    const writer = new Guacamole.BlobWriter(stream);
    writer.onprogress = (_b: Blob, offset: number) => setUpload({ name: file.name, pct: file.size ? Math.round((offset / file.size) * 100) : 0 });
    writer.oncomplete = () => { setUpload({ name: file.name, pct: 100 }); window.setTimeout(() => setUpload(null), 2000); };
    writer.onerror = () => { setUpload({ name: file.name, pct: 0, fail: true }); window.setTimeout(() => setUpload(null), 3000); };
    setUpload({ name: file.name, pct: 0 });
    writer.sendBlob(file);
  };
  const onDrop = (e: React.DragEvent) => {
    e.preventDefault(); setDragOver(false);
    if (share?.viewOnly) return;
    for (const f of Array.from(e.dataTransfer.files)) sendFile(f);
  };

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

  // Voice dictation into the VM, pasted so the VM's 한/영 mode cannot garble it.
  const typist = useMemo(() => rdpTypist(() => client.current), []);

  useEffect(() => {
    let live = true;
    setStatus('connecting'); setError(''); setDriveReady(false); setDrivePanel(false);
    const el = host.current;
    const st = stage.current;
    if (!el || !st) return undefined;
    const tunnel = new Guacamole.WebSocketTunnel(share ? `api/cshare/${encodeURIComponent(share.token)}/ws` : 'api/remote/rdp');
    const c = new Guacamole.Client(tunnel);
    client.current = c;
    const display = c.getDisplay();
    st.innerHTML = '';
    st.appendChild(display.getElement());

    // Fitted whole, or as far as two fingers zoomed in (rdpZoom).
    let touch: any = null;
    setZoom(1);
    const zoomed = rdpZoom({
      stage: st, display, pinchOnly: cview.touch === 'pad', onZoom: (z) => { if (live) setZoom(z); },
      onRescale: (ratio) => { if (cview.touch === 'pad' && touch) { touch.currentState.x *= ratio; touch.currentState.y *= ratio; } },
    });
    zoomer.current = zoomed;
    const fit = zoomed.fit;
    display.onresize = fit;
    fitRef.current = fit;

    // Dynamic resolution: when the window or fullscreen size changes, ask guacd
    // to resize the remote desktop to match (resize-method display-update), so
    // it stays crisp at 1:1 instead of being scaled. Debounced against resize
    // storms; guacd's resize then triggers fit().
    let sizeTimer: number | undefined;
    const pushSize = () => {
      if (fixed) return;
      if (sizeTimer) window.clearTimeout(sizeTimer);
      sizeTimer = window.setTimeout(() => {
        const w = Math.max(el.clientWidth, 640), h = Math.max(el.clientHeight, 480);
        try { client.current?.sendSize(w, h); } catch { /* not connected yet */ }
      }, 300);
    };
    resizeRef.current = pushSize;

    // On an unexpected drop, auto-reconnect with backoff; but a credential or
    // permission failure is shown, not retried (retrying a locked account only
    // makes it worse). `failed` keeps onerror and the state change from both
    // firing it for one drop.
    let failed = false;
    const fail = (fault?: { code?: number; message?: string }) => {
      if (!live || failed) return;
      failed = true;
      const { message, retry } = describeFault(t, fault);
      if (retry && retries.current < RECONNECT_MAX && keepaliveRef.current) {
        const n = ++retries.current;
        setStatus('connecting');
        setError(t('remote.console.reconnecting', { n, max: RECONNECT_MAX }));
        reconnectTimer.current = window.setTimeout(() => { if (live) setAttempt(a => a + 1); }, reconnectDelay(n));
      } else {
        setStatus('disconnected'); setError(message);
      }
    };
    c.onstatechange = (state: number) => {
      if (!live) return;
      if (state === 3) { retries.current = 0; setStatus('connected'); setError(''); fit(); if (!share?.viewOnly) el.focus({ preventScroll: true }); }
      else if (state === 5) fail();
    };
    c.onerror = (err: { code?: number; message?: string }) => fail(err);
    // Text copied inside the VM → the local clipboard.
    c.onclipboard = (stream: any, mimetype: string) => {
      if (mimetype.indexOf('text') !== 0) return;
      const reader = new Guacamole.StringReader(stream);
      let text = '';
      reader.ontext = (t2: string) => { text += t2; };
      reader.onend = () => { navigator.clipboard.writeText(text).catch(() => { /* ignore */ }); };
    };
    // guacd's redirected drive becomes available — keep it for file transfer.
    c.onfilesystem = (object: any) => { filesystem.current = object; if (live) setDriveReady(true); };
    // A file the VM puts in the drive's Download folder: guacd sends it here to save.
    c.onfile = (stream: any, mimetype: string, filename: string) => {
      if (share?.viewOnly) { stream.sendAck('Refused', 0x0303); return; }
      const reader = new Guacamole.BlobReader(stream, mimetype);
      reader.onend = () => { saveBlob(reader.getBlob(), filename); if (live) setUpload({ name: filename, pct: 100, got: true }); window.setTimeout(() => setUpload(null), 3000); };
      stream.sendAck('Ready', 0);
    };

    const dims = () => (fixed ? `GUAC_WIDTH=${fixed.w}&GUAC_HEIGHT=${fixed.h}&GUAC_DPI=96`
      : `GUAC_WIDTH=${Math.max(el.clientWidth || 1280, 640)}&GUAC_HEIGHT=${Math.max(el.clientHeight || 800, 480)}&GUAC_DPI=96`);
    if (share) {
      c.connect((share.pw ? `pw=${encodeURIComponent(share.pw)}&` : '') + dims());
    } else {
      (async () => {
        try {
          const ticket = await rdpTicket(target);
          // An alternate account issues a one-use creds token for this connect.
          const alt = altCreds.current;
          const creds = alt ? `&creds=${await consoleCreds(target, alt.user, alt.password)}` : '';
          if (!live) return;
          c.connect(`target=${encodeURIComponent(target)}&ticket=${ticket}&${dims()}${creds}`);
        } catch (e) { fail({ message: e instanceof Error ? e.message : undefined }); }
      })();
    }

    const focusScreen = () => { if (document.activeElement !== el) el.focus({ preventScroll: true }); };

    // Input (skipped entirely for a view-only share).
    let kb: any = null;
    if (!share?.viewOnly) {
      // Pointer positions are on the scaled screen; the VM wants its own pixels.
      const send = (s: any) => {
        const k = display.getScale() || 1;
        c.sendMouseState(new Guacamole.Mouse.State(s.x / k, s.y / k, s.left, s.middle, s.right, s.up, s.down));
      };
      // A touchpad's pointer can leave a zoomed-in view; the view goes after it.
      const sendPad = (s: any) => { send(s); zoomed.follow(s.x, s.y); };
      const mouse = new Guacamole.Mouse(display.getElement());
      mouse.onmousedown = mouse.onmouseup = mouse.onmousemove = send;
      // Guacamole.Mouse ignores touches, so a phone gets its own: tap where you
      // touch (long press: right click), or a touchpad that moves the pointer.
      touch = cview.touch === 'pad' ? new Guacamole.Mouse.Touchpad(display.getElement()) : new Guacamole.Mouse.Touchscreen(display.getElement());
      touch.onmousedown = touch.onmouseup = send;
      touch.onmousemove = cview.touch === 'pad' ? sendPad : send;
      // Guacamole.Mouse cancels mousedown, which also stops the browser from
      // focusing the screen — so keys went nowhere. Focus it on every press.
      el.addEventListener('mousedown', focusScreen, true);
      kb = new Guacamole.Keyboard(el);
      kb.onkeydown = (keysym: number) => {
        const out = remoteKey(keysym);
        c.sendKeyEvent(1, out.keysym);
        if (out.tap) c.sendKeyEvent(0, out.keysym);
      };
      kb.onkeyup = (keysym: number) => { const out = remoteKey(keysym); if (!out.tap) c.sendKeyEvent(0, out.keysym); };
      keyboard.current = kb;
      el.addEventListener('mouseenter', pushClipboard);
      window.addEventListener('focus', pushClipboard);
    }

    return () => {
      live = false;
      if (reconnectTimer.current) window.clearTimeout(reconnectTimer.current);
      if (sizeTimer) window.clearTimeout(sizeTimer);
      filesystem.current = null;
      zoomed.detach(); zoomer.current = null;
      el.removeEventListener('mousedown', focusScreen, true);
      el.removeEventListener('mouseenter', pushClipboard);
      window.removeEventListener('focus', pushClipboard);
      try { if (kb) { kb.onkeydown = null; kb.onkeyup = null; } } catch { /* ignore */ }
      try { c.disconnect(); } catch { /* already gone */ }
      client.current = null; keyboard.current = null;
    };
  }, [target, attempt, fixedKey, cview.touch]); // eslint-disable-line react-hooks/exhaustive-deps

  const kbd = useConsoleKeyboard({
    enabled: status === 'connected' && !share?.viewOnly,
    // As Unicode, so letters stay letters while the VM is in 한글 mode.
    keysym: (cp) => 0x01000000 + cp,
    send: (keysym, down) => { try { client.current?.sendKeyEvent(down ? 1 : 0, keysym); } catch { /* not ready */ } },
  });

  return (
    <div className={`rw-view rw-console ${folded ? 'is-folded' : ''}`} ref={root}>
      {folded && popout && (
        <button type="button" className="chrome-handle" onClick={() => setFolded(false)}
          title={t('app.showChrome')} aria-label={t('app.showChrome')}>▾</button>
      )}
      <div className="rw-console-bar">
        <strong>{label}</strong>
        <span className="rw-proto-tag">RDP</span>
        {altUser && <span className="rw-proto-tag">{t('remote.console.altUserActive', { user: altUser })}</span>}
        {share?.viewOnly && <span className="rw-proto-tag">{t('remote.console.share.viewOnly')}</span>}
        <span className={`rw-console-state is-${status}`}>{t(`remote.console.${status}`)}</span>
        <span className="rw-console-spacer" />
        {status === 'connected' && !share?.viewOnly && <ConsoleDictation share={share} view={cview} onText={typist.text} onKeys={typist.keys} />}
        {kbd.button}
        {driveReady && !share && <button className={`btn btn-secondary btn-sm ${drivePanel ? 'is-active' : ''}`} onClick={() => setDrivePanel(d => !d)}>{t('remote.console.drive.files')}</button>}
        {!popout && <button className="btn btn-secondary btn-sm" onClick={() => window.open(`/remote#rdp/${encodeURIComponent(target)}`, `rdp_${target}`, 'width=1300,height=900,menubar=no,toolbar=no,location=no')}>{t('remote.console.detach')}</button>}
        <button className="btn btn-secondary btn-sm" onClick={toggle} title={t('remote.console.fullscreen')} aria-label={t('remote.console.fullscreen')} aria-pressed={full}>⛶</button>
        <button className="btn btn-secondary btn-sm" onClick={() => setFolded(true)} title={t('app.hideChrome')} aria-label={t('app.hideChrome')}>▴</button>
        {!share && <button className="btn btn-secondary btn-sm" onClick={() => setAskUser(true)} title={t('remote.console.altUser')} aria-label={t('remote.console.altUser')}>👤</button>}
        <button className="btn btn-secondary btn-sm" onClick={() => setShowSettings(true)} title={t('remote.console.settings')} aria-label={t('remote.console.settings')}>⚙</button>
        {!share?.viewOnly && <button className="btn btn-secondary btn-sm" title={t('remote.console.hangulHint')}
          onClick={() => { try { client.current?.sendKeyEvent(1, ALT_R); client.current?.sendKeyEvent(0, ALT_R); } catch { /* not ready */ } }} disabled={status !== 'connected'}>{t('remote.console.hangul')}</button>}
        {!share?.viewOnly && <button className="btn btn-secondary btn-sm" onClick={() => { try { const c = client.current; c?.sendKeyEvent(1, 0xffe3); c?.sendKeyEvent(1, 0xffe9); c?.sendKeyEvent(1, 0xffff); c?.sendKeyEvent(0, 0xffff); c?.sendKeyEvent(0, 0xffe9); c?.sendKeyEvent(0, 0xffe3); } catch { /* not ready */ } }} disabled={status !== 'connected'}>Ctrl+Alt+Del</button>}
        {status === 'disconnected' && <button className="btn btn-primary btn-sm" onClick={() => { retries.current = 0; setAttempt(a => a + 1); }}>{t('remote.console.reconnect')}</button>}
        {canEdit && <button className="btn btn-secondary btn-sm" onClick={onEdit}>{t('common.edit')}</button>}
        {canGrant && <button className="btn btn-secondary btn-sm" onClick={onGrant}>{t('remote.console.access.btn')}</button>}
        {canShare && <button className="btn btn-secondary btn-sm" onClick={onShare}>{t('remote.console.share.btn')}</button>}
        {canEdit && <button className="btn btn-danger btn-sm" onClick={onDelete}>{t('common.delete')}</button>}
      </div>
      {error && <p role="alert" className="message rw-console-error" onClick={() => setError('')}>{error}</p>}
      <div
        ref={host}
        className={`rw-console-screen rw-rdp-screen ${dragOver ? 'is-dragover' : ''}`}
        tabIndex={0}
        onDrop={share?.viewOnly ? undefined : onDrop}
        onDragOver={share?.viewOnly ? undefined : (e) => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
      >
        <div ref={stage} className="rw-rdp-stage" />
        {zoom > 1 && (
          <button type="button" className="rw-rdp-zoom" onClick={() => zoomer.current?.reset()} title={t('remote.console.zoomHint')}>
            {t('remote.console.zoomFit', { pct: Math.round(zoom * 100) })}
          </button>
        )}
        {dragOver && !share?.viewOnly && <div className="rw-rdp-droplayer">{t('remote.console.drive.hint')}</div>}
        {drivePanel && <RdpDrivePanel target={target} onClose={() => setDrivePanel(false)} />}
        {upload && (
          <div className="rw-rdp-upload">
            {upload.got ? t('remote.console.drive.received', { name: upload.name }) : upload.fail
              ? t('remote.console.drive.failed', { name: upload.name })
              : upload.pct >= 100
                ? t('remote.console.drive.done', { name: upload.name })
                : t('remote.console.drive.sending', { name: upload.name, pct: upload.pct })}
          </div>
        )}
      </div>
      {kbd.panel}
      {showSettings && <ConsoleSettingsModal view={cview} onChange={setCview} onClose={() => setShowSettings(false)} rdp share={share} />}
      {askUser && (
        <ConsoleAltUserModal onClose={() => setAskUser(false)}
          onConnect={(u, p) => { altCreds.current = { user: u, password: p }; setAltUser(u); setAskUser(false); retries.current = 0; setAttempt(a => a + 1); }} />
      )}
    </div>
  );
}
