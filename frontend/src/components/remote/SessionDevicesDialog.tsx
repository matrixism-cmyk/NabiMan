import React, { useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { useT } from '../../shared/i18n';
import { apiRequest } from '../../shared/api/useApi';
import Modal from '../../shared/ui/Modal';
import * as devices from './serialDevices';
import { noticeText, serialSupported, setTranslator, type Notice } from './serialPortBridge';

const BAUDS = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];
const LIST_EVERY_MS = 3000;

/** A device on the session as the server lists it (anyone's, not only this browser's). */
interface Listed { name: string; path: string; attached_by: string; mine: boolean; since_unix: number }

/** How many of this browser's devices are attached to `sid` — for the button label. */
export function useSessionDeviceCount(sid: string | undefined) {
  const all = useSyncExternalStore(devices.subscribe, devices.getDevices);
  return sid ? all.filter((d) => d.sid === sid).length : 0;
}

/**
 * The serial devices of one terminal session: this browser's (with their port,
 * reset and bootloader controls) and anyone else's the server reports, plus a
 * form to attach another board. See serialDevices.ts.
 */
export default function SessionDevicesDialog({ sessionId, sessionName, onClose }: {
  sessionId: string; sessionName?: string; onClose: () => void;
}) {
  const { t } = useT();
  useEffect(() => { setTranslator(t); }, [t]);
  const all = useSyncExternalStore(devices.subscribe, devices.getDevices);
  const mine = useMemo(() => all.filter((d) => d.sid === sessionId), [all, sessionId]);
  const [listed, setListed] = useState<Listed[]>([]);
  const [error, setError] = useState<Notice | null>(null);
  const [busy, setBusy] = useState(false);
  const [baud, setBaud] = useState(115200);
  const taken = useMemo(() => Array.from(new Set([...mine.map((d) => d.name), ...listed.map((d) => d.name)])), [mine, listed]);
  const [name, setName] = useState('');
  const nameValue = name || devices.suggestName(taken);

  useEffect(() => {
    let live = true;
    const load = async () => {
      const r = await apiRequest<Listed[]>(`/api/terminal/sessions/${encodeURIComponent(sessionId)}/serial`).catch(() => null);
      if (live && r?.success && Array.isArray(r.data)) setListed(r.data);
    };
    void load();
    const timer = setInterval(() => { void load(); }, LIST_EVERY_MS);
    return () => { live = false; clearInterval(timer); };
  }, [sessionId]);

  const add = async () => {
    setError(null);
    if (taken.includes(nameValue)) { setError({ key: 'serial.dev.nameTaken', vars: { name: nameValue } }); return; }
    setBusy(true);
    const why = await devices.attach(sessionId, nameValue, baud);
    setBusy(false);
    if (why) setError(why); else setName('');
  };
  const run = (f: () => Promise<void>) => () => { f().catch((e) => setError({ key: 'serial.raw', vars: { text: String(e?.message || e) } })); };
  const others = listed.filter((d) => !d.mine || !mine.some((m) => m.name === d.name));
  const copy = (path: string) => navigator.clipboard.writeText(path).catch(() => { /* ignore */ });

  return (
    <Modal onClose={onClose} title={t('serial.dev.title', { session: sessionName || sessionId })} className="serial-dev-dialog">
      <div className="serial-dev">
        <p className="text-secondary">{t('serial.dev.intro')}</p>
        {!serialSupported() && <p role="alert" className="message">{t('serial.unsupported')}</p>}

        {mine.length === 0 && others.length === 0 && <p className="text-secondary">{t('serial.dev.none')}</p>}

        {mine.map((d) => {
          const state = !d.socketUp ? 'connecting' : !d.portUp ? 'waiting' : 'connected';
          const tail = d.log.split(/\r?\n/).slice(-6).join('\n');
          return (
            <section key={d.name} className="serial-dev-item">
              <div className="serial-dev-row">
                <strong>{d.name}</strong>
                <span className={`rw-console-state is-${state === 'connected' ? 'connected' : 'disconnected'}`}>{t(`serial.status.${state}`)}</span>
                <span className="text-secondary">{d.baud}</span>
                <span style={{ flex: 1 }} />
                {state === 'connected' && <>
                  <button className="btn btn-secondary btn-sm" onClick={run(() => devices.reset(sessionId, d.name))} title={t('serial.resetHint')}>{t('serial.reset')}</button>
                  <button className="btn btn-secondary btn-sm" onClick={run(() => devices.bootloader(sessionId, d.name))} title={t('serial.bootloaderHint')}>{t('serial.bootloader')}</button>
                </>}
                {state === 'waiting' && <button className="btn btn-secondary btn-sm" onClick={() => void devices.repick(sessionId, d.name).then((why) => why && setError(why))}>{t('serial.dev.repick')}</button>}
                <button className="btn btn-danger btn-sm" onClick={() => void devices.detach(sessionId, d.name)}>{t('serial.dev.detach')}</button>
              </div>
              {d.path && (
                <div className="serial-dev-row">
                  <input readOnly className="filter-input serial-dev-path" value={d.path} onFocus={(e) => e.target.select()} />
                  <button className="btn btn-secondary btn-sm" onClick={() => copy(d.path)}>{t('common.copy')}</button>
                </div>
              )}
              {d.error && <p role="alert" className="message">{noticeText(d.error)}</p>}
              {state !== 'connected' && d.stuck && <p role="alert" className="message">{noticeText(d.stuck)}</p>}
              {tail.trim() && <pre className="serial-monitor serial-dev-tail">{tail}</pre>}
            </section>
          );
        })}

        {others.map((d) => (
          <section key={`other-${d.name}`} className="serial-dev-item is-other">
            <div className="serial-dev-row">
              <strong>{d.name}</strong>
              <span className="text-secondary">{t('serial.dev.attachedBy', { who: d.attached_by })}</span>
            </div>
            <div className="serial-dev-row">
              <input readOnly className="filter-input serial-dev-path" value={d.path} onFocus={(e) => e.target.select()} />
              <button className="btn btn-secondary btn-sm" onClick={() => copy(d.path)}>{t('common.copy')}</button>
            </div>
          </section>
        ))}

        <section className="serial-dev-add">
          <div className="serial-dev-row">
            <label className="serial-dev-field">{t('serial.name')}
              <input className="filter-input" value={nameValue} maxLength={24}
                onChange={(e) => setName(e.target.value.toLowerCase().replace(/[^a-z0-9_-]/g, ''))} />
            </label>
            <label className="serial-dev-field">{t('serial.baud')}
              <select className="select-input" value={baud} onChange={(e) => setBaud(Number(e.target.value))}>
                {BAUDS.map((b) => <option key={b} value={b}>{b}</option>)}
              </select>
            </label>
            <button className="btn btn-primary btn-sm" onClick={() => void add()} disabled={busy || !serialSupported()}>{t('serial.dev.add')}</button>
          </div>
          <small className="text-secondary">{t('serial.dev.hint')}</small>
          {error && <p role="alert" className="message">{noticeText(error)}</p>}
        </section>
      </div>
    </Modal>
  );
}
