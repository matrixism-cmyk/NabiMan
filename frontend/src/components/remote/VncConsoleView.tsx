import React, { useEffect, useRef, useState } from 'react';
import RFB from '@novnc/novnc/lib/rfb.js';
import { useT } from '../../shared/i18n';
import { useFullscreen } from '../../shared/ui/useFullscreen';
import { describeFault, reconnectDelay, RECONNECT_MAX } from './consoleFault';
import { consoleWsUrl, shareWsUrl } from './vncApi';
import { typeText } from './vncInput';
import AccountDictation from '../terminal/dictation/AccountDictation';

type Status = 'connecting' | 'connected' | 'disconnected';

/** A browser VNC console for an internal VM, over NabiMan's own proxy. noVNC
 *  (RFB) draws into the canvas; the WebSocket is admitted by a one-use ticket. */
export default function VncConsoleView({ vm, label, canEdit, onEdit, onDelete, canGrant, onGrant, canShare, onShare, popout, share }: {
  vm: string; label: string; canEdit?: boolean; onEdit?: () => void; onDelete?: () => void; canGrant?: boolean; onGrant?: () => void; canShare?: boolean; onShare?: () => void; popout?: boolean; share?: { token: string; viewOnly: boolean; pw?: string };
}) {
  const { t } = useT();
  const host = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const rfb = useRef<RFB | null>(null);
  const retries = useRef(0);
  const reconnectTimer = useRef<number | undefined>(undefined);
  const { full, toggle } = useFullscreen(root);
  const [status, setStatus] = useState<Status>('connecting');
  const [error, setError] = useState('');
  const [scale, setScale] = useState(true);
  const [attempt, setAttempt] = useState(0);
  const [clipOpen, setClipOpen] = useState(false);
  const [sendText, setSendText] = useState('');
  const [vmClip, setVmClip] = useState('');
  const [pasted, setPasted] = useState(false);

  useEffect(() => {
    let live = true;
    let failed = false;
    const controller = new AbortController();
    setStatus('connecting'); setError('');
    // On an unexpected drop, auto-reconnect with backoff; a security failure
    // (bad VNC password/auth) is shown, not retried. The guard keeps one drop
    // from firing twice.
    const fail = (fault?: { code?: number; message?: string }) => {
      if (!live || failed) return;
      failed = true;
      const { message, retry } = describeFault(t, fault);
      if (retry && retries.current < RECONNECT_MAX) {
        const n = ++retries.current;
        setStatus('connecting');
        setError(t('remote.console.reconnecting', { n, max: RECONNECT_MAX }));
        reconnectTimer.current = window.setTimeout(() => { if (live) setAttempt(a => a + 1); }, reconnectDelay(n));
      } else {
        setStatus('disconnected'); setError(message);
      }
    };
    (async () => {
      try {
        const url = share ? shareWsUrl(share.token, share.pw) : await consoleWsUrl(vm, controller.signal);
        if (!live || !host.current) return;
        const client = new RFB(host.current, url, { wsProtocols: ['binary'] });
        client.scaleViewport = scale;
        client.resizeSession = !scale;
        client.viewOnly = Boolean(share?.viewOnly);
        client.background = '#0b0d10';
        client.addEventListener('connect', () => { if (live) { retries.current = 0; setStatus('connected'); setError(''); } });
        client.addEventListener('disconnect', (e: Event) => {
          if ((e as CustomEvent<{ clean: boolean }>).detail?.clean) setStatus('disconnected');
          else fail();
        });
        client.addEventListener('securityfailure', () => fail({ message: 'securityfailure' }));
        // Text copied inside the VM arrives here (if the guest supports it).
        client.addEventListener('clipboard', (e: Event) => live && setVmClip((e as CustomEvent<{ text: string }>).detail?.text ?? ''));
        rfb.current = client;
      } catch (cause) {
        if (!controller.signal.aborted) fail({ message: cause instanceof Error ? cause.message : undefined });
      }
    })();
    return () => {
      live = false; controller.abort();
      if (reconnectTimer.current) window.clearTimeout(reconnectTimer.current);
      try { rfb.current?.disconnect(); } catch { /* already gone */ }
      rfb.current = null;
    };
  }, [vm, attempt]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => { if (rfb.current) { rfb.current.scaleViewport = scale; rfb.current.resizeSession = !scale; } }, [scale]);

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
          if (text && rfb.current) { typeText(rfb.current, text); setPasted(true); setTimeout(() => setPasted(false), 1200); }
        }).catch(() => { /* needs clipboard permission */ });
      }
    };
    el.addEventListener('keydown', onKey, true);
    return () => el.removeEventListener('keydown', onKey, true);
  }, []);

  const openClip = async () => {
    try { const text = await navigator.clipboard.readText(); if (text) setSendText(text); } catch { /* no permission */ }
    setClipOpen((o) => !o);
  };

  // Voice dictation typed into the VM (ASCII reliable; Hangul limited on VNC).
  const dictate = (text: string) => { if (rfb.current) typeText(rfb.current, text); };
  const dictateKeys = (keys: string) => {
    const r = rfb.current; if (!r) return;
    for (const ch of keys) {
      if (ch === '\r' || ch === '\n') r.sendKey(0xff0d, null);
      else if (ch === '\x7f' || ch === '\b') r.sendKey(0xff08, null);
      else { const cp = ch.codePointAt(0) ?? 0; if (cp) r.sendKey(cp < 0x100 ? cp : 0x01000000 + cp, null); }
    }
  };

  return (
    <div className="rw-view rw-console" ref={root}>
      <div className="rw-console-bar">
        <strong>{label}</strong>
        <span className={`rw-console-state is-${status}`}>{t(`remote.console.${status}`)}</span>
        {share?.viewOnly && <span className="rw-proto-tag">{t('remote.console.share.viewOnly')}</span>}
        <span className="rw-console-spacer" />
        <label className="rw-console-toggle"><input type="checkbox" checked={scale} onChange={e => setScale(e.target.checked)} /> {t('remote.console.fit')}</label>
        {status === 'connected' && !share?.viewOnly && <AccountDictation variant="bar" onText={dictate} onKeys={dictateKeys} />}
        {!share?.viewOnly && <button className={`btn btn-secondary btn-sm ${clipOpen ? 'is-active' : ''}`} onClick={openClip} disabled={status !== 'connected'}>{t('remote.console.clip.btn')}</button>}
        {!popout && <button className="btn btn-secondary btn-sm" onClick={() => window.open(`/remote#console/${encodeURIComponent(vm)}`, `vnccon_${vm}`, 'width=1300,height=900,menubar=no,toolbar=no,location=no')}>{t('remote.console.detach')}</button>}
        <button className="btn btn-secondary btn-sm" onClick={toggle} title={t('remote.console.fullscreen')} aria-label={t('remote.console.fullscreen')} aria-pressed={full}>⛶</button>
        {!share?.viewOnly && <button className="btn btn-secondary btn-sm" onClick={() => { try { rfb.current?.sendCtrlAltDel(); } catch { /* not ready */ } }} disabled={status !== 'connected'}>Ctrl+Alt+Del</button>}
        {status === 'disconnected' && <button className="btn btn-primary btn-sm" onClick={() => { retries.current = 0; setAttempt(a => a + 1); }}>{t('remote.console.reconnect')}</button>}
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
            <button className="btn btn-primary btn-sm" disabled={!sendText || status !== 'connected'} onClick={() => { try { rfb.current?.clipboardPasteFrom(sendText); } catch { /* ignore */ } }}>{t('remote.console.clip.send')}</button>
            <button className="btn btn-secondary btn-sm" disabled={!sendText || status !== 'connected'} onClick={() => { if (rfb.current) typeText(rfb.current, sendText); }}>{t('remote.console.clip.type')}</button>
            <span className="rw-console-clip-hint">{t('remote.console.clip.hint')}</span>
          </div>
          <label>{t('remote.console.clip.fromVm')}</label>
          <div className="rw-console-clip-row">
            <input readOnly value={vmClip} placeholder={t('remote.console.clip.fromVmEmpty')} />
            <button className="btn btn-secondary btn-sm" disabled={!vmClip} onClick={() => navigator.clipboard.writeText(vmClip).catch(() => { /* ignore */ })}>{t('common.copy')}</button>
          </div>
        </div>
      )}
      {error && <p role="alert" className="message rw-console-error" onClick={() => setError('')}>{error}</p>}
      {pasted && <div className="rw-console-toast">{t('remote.console.clip.pasted')}</div>}
      <div ref={host} className="rw-console-screen" tabIndex={-1} />
    </div>
  );
}
