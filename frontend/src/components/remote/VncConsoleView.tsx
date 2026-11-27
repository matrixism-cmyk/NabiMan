import React, { useEffect, useRef, useState } from 'react';
import { useT } from '../../shared/i18n';
import { useFullscreen } from '../../shared/ui/useFullscreen';
import { useChromeHidden } from '../../shared/ui/useChromeHidden';
import { useConsoleView, keepAlive, ConsoleShare } from './consoleView';
import ConsoleSettingsModal from './ConsoleSettingsModal';
import ConsoleDetachButtons from './ConsoleDetachButtons';
import ConsoleDictation from './ConsoleDictation';
import { useConsoleKeyboard } from './ConsoleKeyboard';
import ConsoleSpeedSelect from './ConsoleSpeedSelect';
import { sendKeysym, typeText } from './vncInput';
import { VncSession } from './vncSession';
import { noticeText } from './consoleSession';
import { useConsoleSession } from './useConsoleSession';
import { DEFAULT_SPEED, readSpeed, rememberSpeed } from './consoleSpeed';

/** A browser VNC console for an internal VM, over NabiMan's own proxy. noVNC
 *  (RFB) draws into the canvas; the WebSocket is admitted by a one-use ticket.
 *  In the workspace the connection is the console keeper's: leaving this view
 *  lets go of the screen, not of the connection. */
export default function VncConsoleView({ vm, label, canEdit, onEdit, onDelete, canGrant, onGrant, canShare, onShare, popout, share, windowed, focused, onDetach, onPopOut }: {
  vm: string; label: string; canEdit?: boolean; onEdit?: () => void; onDelete?: () => void; canGrant?: boolean; onGrant?: () => void; canShare?: boolean; onShare?: () => void; popout?: boolean; share?: ConsoleShare;
  /** Shown inside a floating window: a compact bar, and no way out of the pane (it is out already). */
  windowed?: boolean;
  /** In a window: whether it is the focused one. Keys go to the VM only then. */
  focused?: boolean;
  onDetach?: () => void; onPopOut?: () => void;
}) {
  const { t } = useT();
  const host = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const kept = !popout && !share;
  const { session, begin, leave, reconnect, hangUp } = useConsoleSession(vm, kept, VncSession, () => new VncSession(vm, share));
  const { status, vmClip } = session;
  const { full, toggle } = useFullscreen(root);
  const [folded, setFolded] = useChromeHidden();
  const [cview, setCview] = useConsoleView();
  const [showSettings, setShowSettings] = useState(false);
  const [scale, setScale] = useState(true);
  const [speed, setSpeed] = useState(() => (share ? DEFAULT_SPEED : readSpeed(vm)));
  const [clipOpen, setClipOpen] = useState(false);
  const [sendText, setSendText] = useState('');
  const [pasted, setPasted] = useState(false);
  useEffect(() => { session.autoReconnect = keepAlive(cview); });

  // While this view is up it holds the screen; going away gives it back and
  // leaves the connection to the keeper.
  useEffect(() => {
    const el = host.current;
    if (!el) return undefined;
    session.attach(el);
    begin();
    return () => { session.detach(); leave(); };
  }, [session]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => { session.setScale(scale); }, [session, scale]);

  // In a window, the keyboard follows the window's focus. noVNC takes keys
  // only while its canvas has the focus: given when the window comes to the
  // front, taken away (held keys let go) when another does or it is minimised
  // — and again should a connection come up in a window that is not in front.
  useEffect(() => {
    const rfb = session.rfb as (typeof session.rfb & { focus?: (o?: FocusOptions) => void }) | null;
    if (focused === undefined || !rfb) return;
    try {
      if (!focused) rfb.blur();
      else if (status === 'connected' && !share?.viewOnly) rfb.focus?.({ preventScroll: true });
    } catch { /* not connected */ }
  }, [session, focused, status]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { session.setSpeed(speed); }, [session, speed]);

  // Ctrl/Cmd+V while the console has focus pastes the browser clipboard into the
  // VM as keystrokes — no panel, no VM changes. (ASCII is reliable; Hangul/IME
  // may not map on a plain VNC server.)
  useEffect(() => {
    const el = host.current;
    if (!el) return undefined;
    const onKey = (e: KeyboardEvent) => {
      if (!share?.viewOnly && (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && (e.key === 'v' || e.key === 'V')) {
        e.preventDefault(); e.stopImmediatePropagation();
        navigator.clipboard.readText().then((text) => {
          if (text && session.rfb) { typeText(session.rfb, text); setPasted(true); setTimeout(() => setPasted(false), 1200); }
        }).catch(() => { /* needs clipboard permission */ });
      }
    };
    el.addEventListener('keydown', onKey, true);
    return () => el.removeEventListener('keydown', onKey, true);
  }, [session]); // eslint-disable-line react-hooks/exhaustive-deps

  const openClip = async () => {
    try { const text = await navigator.clipboard.readText(); if (text) setSendText(text); } catch { /* no permission */ }
    setClipOpen((o) => !o);
  };

  // Voice dictation typed into the VM (ASCII reliable; Hangul limited on VNC).
  const dictate = (text: string) => { if (session.rfb) typeText(session.rfb, text); };
  const dictateKeys = (keys: string) => { if (session.rfb) typeText(session.rfb, keys); };

  const notice = noticeText(t, session.notice);
  const kbd = useConsoleKeyboard({
    enabled: status === 'connected' && !share?.viewOnly,
    screen: host,
    send: (keysym, down) => { try { if (session.rfb) sendKeysym(session.rfb, keysym, down); } catch { /* not ready */ } },
  });

  return (
    <div className={`rw-view rw-console ${folded && !windowed ? 'is-folded' : ''} ${windowed ? 'is-windowed' : ''}`} ref={root}>
      {folded && popout && (
        <button type="button" className="chrome-handle" onClick={() => setFolded(false)}
          title={t('app.showChrome')} aria-label={t('app.showChrome')}>▾</button>
      )}
      <div className="rw-console-bar">
        {!windowed && <strong>{label}</strong>}
        <span className={`rw-console-state is-${status}`}>{t(`remote.console.${status}`)}</span>
        {share?.viewOnly && <span className="rw-proto-tag">{t('remote.console.share.viewOnly')}</span>}
        <span className="rw-console-spacer" />
        {!share && <ConsoleSpeedSelect value={speed} onChange={(next) => { setSpeed(next); rememberSpeed(vm, next); }} />}
        <label className="rw-console-toggle"><input type="checkbox" checked={scale} onChange={e => setScale(e.target.checked)} /> {t('remote.console.fit')}</label>
        {status === 'connected' && !share?.viewOnly && <ConsoleDictation share={share} view={cview} onText={dictate} onKeys={dictateKeys} />}
        {kbd.button}
        {!share?.viewOnly && <button className={`btn btn-secondary btn-sm ${clipOpen ? 'is-active' : ''}`} onClick={openClip} disabled={status !== 'connected'}>{t('remote.console.clip.btn')}</button>}
        {!popout && !windowed && <ConsoleDetachButtons onDetach={onDetach} onPopOut={onPopOut || (() => { window.open(`/remote#console/${encodeURIComponent(vm)}`, `vnccon_${vm}`, 'width=1300,height=900,menubar=no,toolbar=no,location=no'); })} />}
        <button className="btn btn-secondary btn-sm" onClick={toggle} title={t('remote.console.fullscreen')} aria-label={t('remote.console.fullscreen')} aria-pressed={full}>⛶</button>
        {!windowed && <button className="btn btn-secondary btn-sm" onClick={() => setFolded(true)} title={t('app.hideChrome')} aria-label={t('app.hideChrome')}>▴</button>}
        <button className="btn btn-secondary btn-sm" onClick={() => setShowSettings(true)} title={t('remote.console.settings')} aria-label={t('remote.console.settings')}>⚙</button>
        {!share?.viewOnly && <button className="btn btn-secondary btn-sm" onClick={() => { try { session.rfb?.sendCtrlAltDel(); } catch { /* not ready */ } }} disabled={status !== 'connected'}>Ctrl+Alt+Del</button>}
        {kept && status !== 'disconnected' && <button className="btn btn-secondary btn-sm" onClick={hangUp} title={t('remote.console.hangUpHint')}>{t('remote.console.hangUp')}</button>}
        {status === 'disconnected' && <button className="btn btn-primary btn-sm" onClick={reconnect}>{t('remote.console.reconnect')}</button>}
        {canEdit && <button className="btn btn-secondary btn-sm" onClick={onEdit}>{t('common.edit')}</button>}
        {canGrant && <button className="btn btn-secondary btn-sm" onClick={onGrant}>{t('remote.console.access.btn')}</button>}
        {canShare && <button className="btn btn-secondary btn-sm" onClick={onShare}>{t('remote.console.share.btn')}</button>}
        {canEdit && <button className="btn btn-danger btn-sm" onClick={onDelete}>{t('common.delete')}</button>}
      </div>
      {clipOpen && (
        <div className="rw-console-clip">
          <label>{t('remote.console.clip.toVm')}</label>
          <textarea value={sendText} onChange={e => setSendText(e.target.value)} rows={2} placeholder={t('remote.console.clip.placeholder')} />
          <div className="rw-console-clip-row">
            <button className="btn btn-primary btn-sm" disabled={!sendText || status !== 'connected'} onClick={() => { try { session.rfb?.clipboardPasteFrom(sendText); } catch { /* not connected: the button is disabled then anyway */ } }}>{t('remote.console.clip.send')}</button>
            <button className="btn btn-secondary btn-sm" disabled={!sendText || status !== 'connected'} onClick={() => { if (session.rfb) typeText(session.rfb, sendText); }}>{t('remote.console.clip.type')}</button>
            <span className="rw-console-clip-hint">{t('remote.console.clip.hint')}</span>
          </div>
          <label>{t('remote.console.clip.fromVm')}</label>
          <div className="rw-console-clip-row">
            <input readOnly value={vmClip} placeholder={t('remote.console.clip.fromVmEmpty')} />
            <button className="btn btn-secondary btn-sm" disabled={!vmClip} onClick={() => navigator.clipboard.writeText(vmClip).catch(() => { /* clipboard refused: the text stays on screen to select */ })}>{t('common.copy')}</button>
          </div>
        </div>
      )}
      {notice && <p role="alert" className="message rw-console-error" onClick={() => session.dismiss()}>{notice}</p>}
      {pasted && <div className="rw-console-toast">{t('remote.console.clip.pasted')}</div>}
      <div ref={host} className="rw-console-screen" tabIndex={-1} />
      {kbd.panel}
      {showSettings && <ConsoleSettingsModal view={cview} onChange={setCview} onClose={() => setShowSettings(false)} share={share} />}
    </div>
  );
}
