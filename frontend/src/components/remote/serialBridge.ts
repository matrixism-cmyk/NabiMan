import { serialTicket } from './vncApi';
import {
  BridgeState, PortBridge, errText, noticeText, portKey, serialApi, serialSupported, setTranslator,
} from './serialPortBridge';

/**
 * The admin Serial tab's one global bridge (`/api/remote/serial`). Once a board
 * has been chosen, Chrome remembers the grant, so the next visit reconnects with
 * no picker. Session-attached devices live in serialDevices.ts.
 */

export type { BridgeStatus, BridgeState, Notice } from './serialPortBridge';
export { serialSupported, setTranslator };
export const noticeToText = noticeText;

const SAVED = 'nabiman_serial_bridge';
type Saved = { name: string; baud: number; device: string; auto: boolean };

export function loadSaved(): Saved | null {
  try { const s = JSON.parse(localStorage.getItem(SAVED) || 'null'); return s && typeof s.name === 'string' ? s : null; } catch { return null; }
}
function save(s: Saved | null) {
  try { if (s) localStorage.setItem(SAVED, JSON.stringify(s)); else localStorage.removeItem(SAVED); } catch { /* private mode */ }
}

const IDLE: BridgeState = { status: 'idle', socketUp: false, portUp: false, path: '', error: null, log: '' };
let bridge: PortBridge | null = null;
let cfg: Saved | null = null;
/** What the tab shows while no bridge runs: idle, perhaps with why the last start failed. */
let idleState: BridgeState = IDLE;
const idle = (error: BridgeState['error']) => { idleState = error ? { ...IDLE, error } : IDLE; changed(); };
const listeners = new Set<() => void>();
const changed = () => listeners.forEach((l) => l());

export const subscribe = (l: () => void) => { listeners.add(l); return () => { listeners.delete(l); }; };
export const getState = () => (bridge ? bridge.state : idleState);

function make(s: Saved) {
  cfg = s;
  bridge = new PortBridge({
    baud: s.baud,
    device: s.device,
    url: async () => {
      const ticket = await serialTicket(s.name);
      return `${window.location.origin.replace('http', 'ws')}/api/remote/serial?serial=${encodeURIComponent(s.name)}&ticket=${ticket}`;
    },
    onChange: changed,
  });
  return bridge;
}

/** Ask the operator for a port (needs a click), then bridge it. */
export async function start(name: string, baud: number) {
  if (!serialSupported()) { idle({ key: 'serial.unsupported' }); return; }
  await stop(false);
  idle(null);
  let chosen: any;
  try {
    chosen = await serialApi().requestPort();
  } catch (e) {
    idle({ key: 'serial.raw', vars: { text: errText(e) } });
    return;
  }
  const s = { name, baud, device: portKey(chosen), auto: true };
  const b = make(s);
  if (await b.startWith(chosen)) save(s);
  else { bridge = null; idle(b.state.error); }
}

/** Bridge the remembered board again without a picker. */
export async function resume() {
  const saved = loadSaved();
  if (bridge || !saved?.auto || !serialSupported()) return;
  await make(saved).resume();
}

/** End the bridge. `forget` also stops the next visit from reconnecting by itself. */
export async function stop(forget = true) {
  const b = bridge;
  bridge = null;
  await b?.stop();
  if (forget && cfg) save({ ...cfg, auto: false });
  changed();
}

export const reset = () => bridge?.reset() ?? Promise.resolve();
export const bootloader = () => bridge?.bootloader() ?? Promise.resolve();
