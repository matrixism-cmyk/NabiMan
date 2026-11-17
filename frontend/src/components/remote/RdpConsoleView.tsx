import React, { useEffect, useMemo, useRef, useState } from 'react';
import Guacamole from '../../vendor/guacamole-common.js';
import { sendWhenOpen } from './rdpUpload';
import { useT } from '../../shared/i18n';
import { useFullscreen } from '../../shared/ui/useFullscreen';
import { useChromeHidden } from '../../shared/ui/useChromeHidden';
import { remoteKey, ALT_R } from './rdpKeys';
import { useConsoleView, keepAlive, fixedSize, ConsoleShare } from './consoleView';
import ConsoleSettingsModal from './ConsoleSettingsModal';
import ConsoleDictation from './ConsoleDictation';
import { useConsoleKeyboard } from './ConsoleKeyboard';
import ConsoleAltUserModal from './ConsoleAltUserModal';
import ConsoleSpeedSelect from './ConsoleSpeedSelect';
import RdpDrivePanel from './RdpDrivePanel';
import { rdpTypist } from './rdpTypist';
import { RdpSession } from './rdpSession';
import { noticeText } from './consoleSession';
import { useConsoleSession } from './useConsoleSession';
import { DEFAULT_SPEED, readSpeed, rememberSpeed } from './consoleSpeed';

/** An RDP console in the browser, over NabiMan's guacd tunnel. Guacamole draws
 *  the desktop and carries keyboard, mouse and clipboard — including copy/paste
 *  to and from the VM, natively (no VNC-style keystroke workaround). In the
 *  workspace the connection is the console keeper's: leaving this view lets go
 *  of the screen and the keys, not of the connection. */
export default function RdpConsoleView({ target, label, canEdit, onEdit, onDelete, canGrant, onGrant, canShare, onShare, popout, share }: {
  target: string; label: string; canEdit?: boolean; onEdit?: () => void; onDelete?: () => void;
  canGrant?: boolean; onGrant?: () => void; canShare?: boolean; onShare?: () => void; popout?: boolean; share?: ConsoleShare;
}) {
  const { t } = useT();
  const host = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const kept = !popout && !share;
  const { session, begin, leave, reconnect, hangUp } = useConsoleSession(target, kept, RdpSession, () => new RdpSession(target, share));
  const { status, zoom } = session;
  const driveReady = Boolean(session.filesystem);
  const altUser = session.altUser();
  const refit = () => { const el = host.current; if (el) session.resize(el.clientWidth, el.clientHeight); };
  const { full, toggle } = useFullscreen(root, refit);
  const [folded, setFolded] = useChromeHidden();
  const [error, setError] = useState('');
  const [upload, setUpload] = useState<{ name: string; pct: number; fail?: boolean; got?: boolean } | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [drivePanel, setDrivePanel] = useState(false);
  const [cview, setCview] = useConsoleView();
  const fixed = fixedSize(cview);
  const fixedKey = fixed ? `${fixed.w}x${fixed.h}` : '';
  const [speed, setSpeed] = useState(() => (share ? DEFAULT_SPEED : readSpeed(target)));
  const options = { touch: cview.touch, fixed, speed };
  const [showSettings, setShowSettings] = useState(false);
  const [askUser, setAskUser] = useState(false);
  useEffect(() => { session.autoReconnect = keepAlive(cview); });

  // Drag a file onto the VM to copy it in, via guacd's redirected drive (it
  // lands on the "NabiMan" drive inside Windows). View-only shares cannot.
  const sendFile = (file: File) => {
    const fs = session.filesystem;
    if (!fs) { setError(t('remote.console.drive.pending')); return; }
    const stream = fs.createOutputStream(file.type || 'application/octet-stream', `/${file.name}`);
    const failed = () => { setUpload({ name: file.name, pct: 0, fail: true }); window.setTimeout(() => setUpload(null), 3000); };
    setUpload({ name: file.name, pct: 0 });
    sendWhenOpen(stream, file, {
      progress: (offset) => setUpload({ name: file.name, pct: file.size ? Math.round((offset / file.size) * 100) : 0 }),
      done: () => { setUpload({ name: file.name, pct: 100 }); window.setTimeout(() => setUpload(null), 2000); },
      failed,
    });
  };
  const onDrop = (e: React.DragEvent) => {
    e.preventDefault(); setDragOver(false);
    if (share?.viewOnly) return;
    for (const f of Array.from(e.dataTransfer.files)) sendFile(f);
  };

  // Voice dictation into the VM, pasted so the VM's 한/영 mode cannot garble it.
  const typist = useMemo(() => rdpTypist(() => session.client), [session]);

  // While this view is up it holds the screen and the keyboard; going away
  // gives both back and leaves the connection to the keeper.
  useEffect(() => {
    const el = host.current;
    if (!el) return undefined;
    session.configure(options);
    session.attach(el);
    session.onReceived = (name) => { setUpload({ name, pct: 100, got: true }); window.setTimeout(() => setUpload(null), 3000); };
    begin();
    if (share?.viewOnly) return () => { session.detach(); leave(); };
    // Guacamole.Mouse cancels mousedown, which also stops the browser from
    // focusing the screen — so keys went nowhere. Focus it on every press.
    const focusScreen = () => { if (document.activeElement !== el) el.focus({ preventScroll: true }); };
    el.addEventListener('mousedown', focusScreen, true);
    const kb = new Guacamole.Keyboard(el);
    kb.onkeydown = (keysym: number) => {
      const out = remoteKey(keysym);
      session.sendKey(true, out.keysym);
      if (out.tap) session.sendKey(false, out.keysym);
    };
    kb.onkeyup = (keysym: number) => { const out = remoteKey(keysym); if (!out.tap) session.sendKey(false, out.keysym); };
    el.addEventListener('mouseenter', session.pushClipboard);
    window.addEventListener('focus', session.pushClipboard);
    return () => {
      el.removeEventListener('mousedown', focusScreen, true);
      el.removeEventListener('mouseenter', session.pushClipboard);
      window.removeEventListener('focus', session.pushClipboard);
      // Keys still held go up in the VM, so none stays stuck while out of sight.
      try { kb.reset(); kb.onkeydown = null; kb.onkeyup = null; } catch { /* teardown: the keyboard may already be gone */ }
      session.detach(); leave();
    };
  }, [session]); // eslint-disable-line react-hooks/exhaustive-deps

  // A different size, touch mode or speed reconnects with it.
  useEffect(() => { session.configure(options); }, [session, fixedKey, cview.touch, speed]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (status !== 'connected') { setDrivePanel(false); return; }
    refit();
    if (!share?.viewOnly) host.current?.focus({ preventScroll: true });
  }, [session, status]); // eslint-disable-line react-hooks/exhaustive-deps

  const notice = error || noticeText(t, session.notice);
  const ctrlAltDel = () => { [[true, 0xffe3], [true, 0xffe9], [true, 0xffff], [false, 0xffff], [false, 0xffe9], [false, 0xffe3]].forEach(([down, key]) => session.sendKey(down as boolean, key as number)); };

  const kbd = useConsoleKeyboard({
    enabled: status === 'connected' && !share?.viewOnly,
    screen: host,
    // As Unicode, so letters stay letters while the VM is in 한글 mode.
    keysym: (cp) => 0x01000000 + cp,
    send: (keysym, down) => session.sendKey(down, keysym),
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
        {!share && <ConsoleSpeedSelect value={speed} onChange={(next) => { setSpeed(next); rememberSpeed(target, next); }} />}
        {status === 'connected' && !share?.viewOnly && <ConsoleDictation share={share} view={cview} onText={typist.text} onKeys={typist.keys} />}
        {kbd.button}
        {driveReady && !share && <button className={`btn btn-secondary btn-sm ${drivePanel ? 'is-active' : ''}`} onClick={() => setDrivePanel(d => !d)}>{t('remote.console.drive.files')}</button>}
        {!popout && <button className="btn btn-secondary btn-sm" onClick={() => window.open(`/remote#rdp/${encodeURIComponent(target)}`, `rdp_${target}`, 'width=1300,height=900,menubar=no,toolbar=no,location=no')}>{t('remote.console.detach')}</button>}
        <button className="btn btn-secondary btn-sm" onClick={toggle} title={t('remote.console.fullscreen')} aria-label={t('remote.console.fullscreen')} aria-pressed={full}>⛶</button>
        <button className="btn btn-secondary btn-sm" onClick={() => setFolded(true)} title={t('app.hideChrome')} aria-label={t('app.hideChrome')}>▴</button>
        {!share && <button className="btn btn-secondary btn-sm" onClick={() => setAskUser(true)} title={t('remote.console.altUser')} aria-label={t('remote.console.altUser')}>👤</button>}
        <button className="btn btn-secondary btn-sm" onClick={() => setShowSettings(true)} title={t('remote.console.settings')} aria-label={t('remote.console.settings')}>⚙</button>
        {!share?.viewOnly && <button className="btn btn-secondary btn-sm" title={t('remote.console.hangulHint')}
          onClick={() => { session.sendKey(true, ALT_R); session.sendKey(false, ALT_R); }} disabled={status !== 'connected'}>{t('remote.console.hangul')}</button>}
        {!share?.viewOnly && <button className="btn btn-secondary btn-sm" onClick={ctrlAltDel} disabled={status !== 'connected'}>Ctrl+Alt+Del</button>}
        {kept && status !== 'disconnected' && <button className="btn btn-secondary btn-sm" onClick={hangUp} title={t('remote.console.hangUpHint')}>{t('remote.console.hangUp')}</button>}
        {status === 'disconnected' && <button className="btn btn-primary btn-sm" onClick={() => { setError(''); reconnect(); }}>{t('remote.console.reconnect')}</button>}
        {canEdit && <button className="btn btn-secondary btn-sm" onClick={onEdit}>{t('common.edit')}</button>}
        {canGrant && <button className="btn btn-secondary btn-sm" onClick={onGrant}>{t('remote.console.access.btn')}</button>}
        {canShare && <button className="btn btn-secondary btn-sm" onClick={onShare}>{t('remote.console.share.btn')}</button>}
        {canEdit && <button className="btn btn-danger btn-sm" onClick={onDelete}>{t('common.delete')}</button>}
      </div>
      {notice && <p role="alert" className="message rw-console-error" onClick={() => { setError(''); session.dismiss(); }}>{notice}</p>}
      <div
        ref={host}
        className={`rw-console-screen rw-rdp-screen ${dragOver ? 'is-dragover' : ''}`}
        tabIndex={0}
        onDrop={share?.viewOnly ? undefined : onDrop}
        onDragOver={share?.viewOnly ? undefined : (e) => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
      >
        {zoom > 1 && (
          <button type="button" className="rw-rdp-zoom" onClick={() => session.resetZoom()} title={t('remote.console.zoomHint')}>
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
          onConnect={(u, p) => { session.setAccount(u, p); setAskUser(false); setError(''); reconnect(); }} />
      )}
    </div>
  );
}
