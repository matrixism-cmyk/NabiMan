import { sessionSerialTicket } from './vncApi';
import {
  BridgeState, Notice, PortBridge, errText, portKey, serialApi, serialSupported,
} from './serialPortBridge';

/**
 * Serial devices attached to terminal sessions (`/api/terminal/serial`): each
 * is one PortBridge, keyed by session id and a name unique within the session.
 * They live outside React, so switching tabs or panes keeps them up. What was
 * attached is remembered per session; opening the session again reattaches it
 * with no picker (Chrome keeps the port grant). See docs/serial-devices-design.md.
 */

export interface SessionDevice extends BridgeState {
  sid: string;
  name: string;
  baud: number;
}
type Remembered = { name: string; baud: number; device: string };

const SAVED = 'nabiman_serial_devices';
export const NAME_RE = /^[a-z0-9_-]{1,24}$/;

function remembered(): Record<string, Remembered[]> {
  try {
    const v = JSON.parse(localStorage.getItem(SAVED) || '{}');
    return v && typeof v === 'object' && !Array.isArray(v) ? v : {};
  } catch { return {}; }
}
function remember(edit: (all: Record<string, Remembered[]>) => void) {
  const all = remembered();
  edit(all);
  for (const sid of Object.keys(all)) if (!all[sid]?.length) delete all[sid];
  try { localStorage.setItem(SAVED, JSON.stringify(all)); } catch { /* private mode */ }
}

const bridges = new Map<string, { sid: string; name: string; baud: number; bridge: PortBridge }>();
const keyOf = (sid: string, name: string) => `${sid}/${name}`;
const listeners = new Set<() => void>();
let snapshot: SessionDevice[] = [];
function changed() {
  snapshot = Array.from(bridges.values()).map(({ sid, name, baud, bridge }) => ({ ...bridge.state, sid, name, baud }));
  listeners.forEach((l) => l());
}

export const subscribe = (l: () => void) => { listeners.add(l); return () => { listeners.delete(l); }; };
export const getDevices = () => snapshot;

function make(sid: string, name: string, baud: number, device: string) {
  const bridge = new PortBridge({
    baud,
    device,
    url: async () => {
      const ticket = await sessionSerialTicket(sid, name);
      return `${window.location.origin.replace('http', 'ws')}/api/terminal/serial?session_id=${sid}&serial=${name}&ticket=${ticket}`;
    },
    onChange: changed,
    lock: `nabiman-serial:${keyOf(sid, name)}`,
  });
  bridges.set(keyOf(sid, name), { sid, name, baud, bridge });
  return bridge;
}

/** The first of `esp32`, `esp32-2`, … not in `taken`. */
export function suggestName(taken: string[], base = 'esp32') {
  for (let i = 1; ; i++) {
    const name = i === 1 ? base : `${base}-${i}`;
    if (!taken.includes(name)) return name;
  }
}

/** Ask the operator for a port (needs a click) and attach it to session `sid` as `name`.
 *  Resolves to why it failed, or null. */
export async function attach(sid: string, name: string, baud: number): Promise<Notice | null> {
  if (!serialSupported()) return { key: 'serial.unsupported' };
  if (!NAME_RE.test(name)) return { key: 'serial.dev.badName' };
  if (bridges.has(keyOf(sid, name))) return { key: 'serial.dev.nameTaken', vars: { name } };
  let chosen: any;
  try {
    chosen = await serialApi().requestPort();
  } catch (e) {
    return { key: 'serial.raw', vars: { text: errText(e) } };
  }
  const device = portKey(chosen);
  const bridge = make(sid, name, baud, device);
  if (!(await bridge.startWith(chosen))) {
    bridges.delete(keyOf(sid, name));
    changed();
    return bridge.state.error;
  }
  remember((all) => { all[sid] = [...(all[sid] || []).filter((d) => d.name !== name), { name, baud, device }]; });
  changed();
  return null;
}

/** Bridge again whatever was attached to `sid` before, with no picker. */
export function resumeSession(sid: string) {
  if (!serialSupported()) return;
  for (const d of remembered()[sid] || []) {
    if (bridges.has(keyOf(sid, d.name))) continue;
    void make(sid, d.name, d.baud, d.device).resume();
  }
  changed();
}

/** Detach a device and stop it coming back next time. */
export async function detach(sid: string, name: string) {
  const entry = bridges.get(keyOf(sid, name));
  bridges.delete(keyOf(sid, name));
  remember((all) => { all[sid] = (all[sid] || []).filter((d) => d.name !== name); });
  changed();
  await entry?.bridge.stop();
  changed();
}

/** Sessions that ended take their devices with them. */
export function pruneSessions(alive: Set<string>) {
  for (const { sid, name } of Array.from(bridges.values())) if (!alive.has(sid)) void detach(sid, name);
  remember((all) => { for (const sid of Object.keys(all)) if (!alive.has(sid)) delete all[sid]; });
}

/** Choose the board again in the picker for a device waiting for it (its grant was lost, say). */
export async function repick(sid: string, name: string): Promise<Notice | null> {
  const entry = bridges.get(keyOf(sid, name));
  if (!entry) return null;
  try {
    await entry.bridge.adopt(await serialApi().requestPort());
  } catch (e) {
    return { key: 'serial.raw', vars: { text: errText(e) } };
  }
  const device = entry.bridge.device;
  remember((all) => { all[sid] = (all[sid] || []).map((d) => (d.name === name ? { ...d, device } : d)); });
  changed();
  return null;
}

const bridgeOf = (sid: string, name: string) => bridges.get(keyOf(sid, name))?.bridge;
export const reset = (sid: string, name: string) => bridgeOf(sid, name)?.reset() ?? Promise.resolve();
export const bootloader = (sid: string, name: string) => bridgeOf(sid, name)?.bootloader() ?? Promise.resolve();
