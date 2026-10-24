import React, { useEffect, useRef, useState } from 'react';
import RFB from '@novnc/novnc/lib/rfb.js';
import { useT } from '../../shared/i18n';
import { consoleWsUrl } from './vncApi';

type Status = 'connecting' | 'connected' | 'disconnected';

/** A browser VNC console for an internal VM, over NabiMan's own proxy. noVNC
 *  (RFB) draws into the canvas; the WebSocket is admitted by a one-use ticket. */
export default function VncConsoleView({ vm, label, canEdit, onEdit, onDelete, canGrant, onGrant, popout }: {
  vm: string; label: string; canEdit?: boolean; onEdit?: () => void; onDelete?: () => void; canGrant?: boolean; onGrant?: () => void; popout?: boolean;
}) {
  const { t } = useT();
  const host = useRef<HTMLDivElement>(null);
  const rfb = useRef<RFB | null>(null);
  const [status, setStatus] = useState<Status>('connecting');
  const [error, setError] = useState('');
  const [scale, setScale] = useState(true);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let live = true;
    const controller = new AbortController();
    setStatus('connecting'); setError('');
    (async () => {
      try {
        const url = await consoleWsUrl(vm, controller.signal);
        if (!live || !host.current) return;
        const client = new RFB(host.current, url, { wsProtocols: ['binary'] });
        client.scaleViewport = scale;
        client.resizeSession = !scale;
        client.background = '#0b0d10';
        client.addEventListener('connect', () => live && setStatus('connected'));
        client.addEventListener('disconnect', (e: Event) => {
          if (!live) return;
          setStatus('disconnected');
          const clean = (e as CustomEvent<{ clean: boolean }>).detail?.clean;
          if (!clean) setError(t('remote.console.lost'));
        });
        client.addEventListener('securityfailure', () => live && setError(t('remote.console.denied')));
        rfb.current = client;
      } catch (cause) {
        if (live) { setStatus('disconnected'); setError(cause instanceof Error ? cause.message : t('common.error')); }
      }
    })();
    return () => {
      live = false; controller.abort();
      try { rfb.current?.disconnect(); } catch { /* already gone */ }
      rfb.current = null;
    };
  }, [vm, attempt]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => { if (rfb.current) { rfb.current.scaleViewport = scale; rfb.current.resizeSession = !scale; } }, [scale]);

  return (
    <div className="rw-view rw-console">
      <div className="rw-console-bar">
        <strong>{label}</strong>
        <span className={`rw-console-state is-${status}`}>{t(`remote.console.${status}`)}</span>
        <span className="rw-console-spacer" />
        <label className="rw-console-toggle"><input type="checkbox" checked={scale} onChange={e => setScale(e.target.checked)} /> {t('remote.console.fit')}</label>
        {!popout && <button className="btn btn-secondary btn-sm" onClick={() => window.open(`/remote#console/${encodeURIComponent(vm)}`, `vnccon_${vm}`, 'width=1300,height=900,menubar=no,toolbar=no,location=no')}>{t('remote.console.detach')}</button>}
        <button className="btn btn-secondary btn-sm" onClick={() => { try { rfb.current?.sendCtrlAltDel(); } catch { /* not ready */ } }} disabled={status !== 'connected'}>Ctrl+Alt+Del</button>
        {status === 'disconnected' && <button className="btn btn-primary btn-sm" onClick={() => setAttempt(a => a + 1)}>{t('remote.console.reconnect')}</button>}
        {canEdit && <button className="btn btn-secondary btn-sm" onClick={onEdit}>{t('common.edit')}</button>}
        {canGrant && <button className="btn btn-secondary btn-sm" onClick={onGrant}>{t('remote.console.access.btn')}</button>}
        {canEdit && <button className="btn btn-danger btn-sm" onClick={onDelete}>{t('common.delete')}</button>}
      </div>
      {error && <p role="alert" className="message rw-console-error" onClick={() => setError('')}>{error}</p>}
      <div ref={host} className="rw-console-screen" />
    </div>
  );
}
