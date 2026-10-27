import React, { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { useT } from '../../shared/i18n';
import * as bridge from './serialBridge';

const BAUDS = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];

/**
 * The Serial tab: a view onto the serial bridge (see serialBridge.ts), which
 * lives outside this component so leaving the tab does not end it.
 */
export default function SerialView() {
  const { t } = useT();
  const supported = bridge.serialSupported();
  const s = useSyncExternalStore(bridge.subscribe, bridge.getState);
  const saved = bridge.loadSaved();
  const [name, setName] = useState(saved?.name || 'esp32');
  const [baud, setBaud] = useState(saved?.baud || 115200);
  const logEnd = useRef<HTMLDivElement>(null);

  useEffect(() => { bridge.setTranslator(t); }, [t]);
  useEffect(() => { void bridge.resume(); }, []);
  useEffect(() => { logEnd.current?.scrollIntoView({ block: 'nearest' }); }, [s.log]);

  const live = s.status === 'connected' && s.portUp;
  const state = s.status === 'idle' ? 'idle'
    : !s.socketUp ? 'connecting'
    : !s.portUp ? 'waiting' : 'connected';
  const run = (f: () => Promise<void>) => () => { f().catch(() => { /* unsupported, or the board dropped off USB */ }); };

  return (
    <div className="rw-view rw-view-scroll">
      <div className="rw-section">
        <div className="rw-section-head"><h3>{t('remote.serialTab')}</h3></div>
        <p className="text-secondary">{t('serial.intro')}</p>
        {!supported && <p role="alert" className="message">{t('serial.unsupported')}</p>}

        <div className="rw-form" style={{ maxWidth: 520 }}>
          <div className="form-group"><label>{t('serial.name')}</label>
            <input className="filter-input" value={name} disabled={s.status !== 'idle'}
              onChange={(e) => setName(e.target.value.replace(/[^a-zA-Z0-9_-]/g, ''))} placeholder="esp32" /></div>
          <div className="form-group"><label>{t('serial.baud')}</label>
            <select className="select-input" value={baud} disabled={s.status !== 'idle'} onChange={(e) => setBaud(Number(e.target.value))}>
              {BAUDS.map((b) => <option key={b} value={b}>{b}</option>)}
            </select></div>
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            {s.status === 'idle'
              ? <button className="btn btn-primary btn-sm" onClick={() => void bridge.start(name, baud)} disabled={!supported}>{t('serial.connect')}</button>
              : <button className="btn btn-danger btn-sm" onClick={() => void bridge.stop()}>{t('serial.disconnect')}</button>}
            {live && <>
              <button className="btn btn-secondary btn-sm" onClick={run(bridge.reset)} title={t('serial.resetHint')}>{t('serial.reset')}</button>
              <button className="btn btn-secondary btn-sm" onClick={run(bridge.bootloader)} title={t('serial.bootloaderHint')}>{t('serial.bootloader')}</button>
            </>}
            <span className={`rw-console-state is-${live ? 'connected' : 'disconnected'}`}>{t(`serial.status.${state}`)}</span>
          </div>
          {s.status !== 'idle' && !s.portUp && <p className="text-secondary">{t('serial.waitingHint')}</p>}
          {s.status === 'idle' && saved?.auto === false && <p className="text-secondary">{t('serial.autoOff')}</p>}
          {s.error && <p role="alert" className="message">{bridge.noticeToText(s.error)}</p>}
        </div>

        {s.path && (
          <div className="rw-section" style={{ marginTop: 14 }}>
            <p className="text-secondary" style={{ marginBottom: 6 }}>{t('serial.pathLabel')}</p>
            <div style={{ display: 'flex', gap: 8 }}>
              <input readOnly className="filter-input" value={s.path} style={{ flex: 1, fontFamily: 'monospace' }} onFocus={(e) => e.target.select()} />
              <button className="btn btn-secondary btn-sm" onClick={() => navigator.clipboard.writeText(s.path).catch(() => { /* ignore */ })}>{t('common.copy')}</button>
            </div>
            <small className="text-secondary">{t('serial.pathHint')}</small>
          </div>
        )}

        {s.status !== 'idle' && (
          <div className="rw-section" style={{ marginTop: 14 }}>
            <p className="text-secondary" style={{ marginBottom: 6 }}>{t('serial.monitor')}</p>
            <pre className="serial-monitor">{s.log}<div ref={logEnd} /></pre>
          </div>
        )}
      </div>
    </div>
  );
}
