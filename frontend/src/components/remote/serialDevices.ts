import { sessionSerialTicket } from './vncApi';
import { channelName, FromHolder, FromMonitor, MonitorState } from './serialMonitorChannel';
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
  /** The board's USB vendor:product (decimal), as the bridge looks for it. */
  device: string;
  /** The operator's own note for it, such as the COM port the browser's picker showed. */
  label: string;
}
type Remembered = { name: string; baud: number; device: string; label?: string };
const MAX_LABEL = 40;

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

const bridges = new Map<string, { sid: string; name: string; baud: number; label: string; bridge: PortBridge; channel: BroadcastChannel | null }>();
const keyOf = (sid: string, name: string) => `${sid}/${name}`;
const listeners = new Set<() => void>();
let snapshot: SessionDevice[] = [];
function changed() {
  snapshot = Array.from(bridges.values()).map(({ sid, name, baud, label, bridge }) => ({ ...bridge.state, sid, name, baud, device: bridge.device, label }));
  listeners.forEach((l) => l());
}

export const subscribe = (l: () => void) => { listeners.add(l); return () => { listeners.delete(l); }; };
export const getDevices = () => snapshot;

/** What a monitor window should show for a bridge. */
function monitorState(bridge: Pick<PortBridge, 'state'>): MonitorState {
  return !bridge.state.socketUp ? 'connecting' : !bridge.state.portUp ? 'waiting' : 'connected';
}

/** The BroadcastChannel a device's monitor windows listen on (see serialMonitorChannel.ts). */
export function monitorChannel(sid: string, name: string, baud: number, bridge: () => Pick<PortBridge, 'state' | 'send' | 'reset' | 'bootloader'> | undefined): BroadcastChannel | null {
  if (typeof BroadcastChannel === 'undefined') return null;
  const channel = new BroadcastChannel(channelName(sid, name));
  const post = (m: FromHolder) => { try { channel.postMessage(m); } catch { /* closed */ } };
  channel.onmessage = (e: MessageEvent<FromMonitor>) => {
    const b = bridge();
    if (!b || !e.data) return;
    const m = e.data;
    if (m.type === 'hello') {
      post({ type: 'backlog', text: b.state.log });
      const e = bridges.get(keyOf(sid, name));
      post({ type: 'state', state: monitorState(b), baud, path: b.state.path, device: (b as PortBridge).device ?? '', label: e?.label ?? '' });
    } else if (m.type === 'send' && ArrayBuffer.isView(m.bytes)) {
      const bytes = new Uint8Array(m.bytes.buffer, m.bytes.byteOffset, m.bytes.byteLength);
      void b.send(bytes).then((ok) => post({ type: 'done', what: 'send', ok }));
    } else if (m.type === 'reset' || m.type === 'bootloader') {
      const what = m.type;
      (what === 'reset' ? b.reset() : b.bootloader())
        .then(() => post({ type: 'done', what, ok: true }))
        .catch((err) => post({ type: 'done', what, ok: false, error: errText(err) }));
    }
  };
  return channel;
}

function make(sid: string, name: string, baud: number, device: string, label = '') {
  let channel: BroadcastChannel | null = null;
  let last = '';
  const bridge = new PortBridge({
    baud,
    device,
    url: async () => {
      const ticket = await sessionSerialTicket(sid, name);
      return `${window.location.origin.replace('http', 'ws')}/api/terminal/serial?session_id=${sid}&serial=${name}&ticket=${ticket}`;
    },
    onChange: () => {
      // A monitor hears when the state it shows changes, not on every byte.
      const now = `${monitorState(bridge)}|${bridge.state.path}|${bridge.device}`;
      if (channel && now !== last) {
        last = now;
        announce(sid, name);
      }
      changed();
    },
    onBytes: (bytes) => { try { channel?.postMessage({ type: 'data', bytes } satisfies FromHolder); } catch { /* closed */ } },
    lock: `nabiman-serial:${keyOf(sid, name)}`,
  });
  channel = monitorChannel(sid, name, baud, () => bridges.get(keyOf(sid, name))?.bridge);
  bridges.set(keyOf(sid, name), { sid, name, baud, label, bridge, channel });
  return bridge;
}

/** Tells a device's monitor windows its state, port and label. */
function announce(sid: string, name: string) {
  const e = bridges.get(keyOf(sid, name));
  if (!e?.channel) return;
  try {
    e.channel.postMessage({ type: 'state', state: monitorState(e.bridge), baud: e.baud, path: e.bridge.state.path, device: e.bridge.device, label: e.label } satisfies FromHolder);
  } catch { /* closed */ }
}

/** Note what a device is (the COM port, say), remembered with it in this browser. */
export function setLabel(sid: string, name: string, label: string) {
  const entry = bridges.get(keyOf(sid, name));
  // eslint-disable-next-line no-control-regex
  const clean = label.replace(/[\u0000-\u001f]/g, '').trim().slice(0, MAX_LABEL);
  if (entry) entry.label = clean;
  remember((all) => { all[sid] = (all[sid] || []).map((d) => (d.name === name ? { ...d, label: clean || undefined } : d)); });
  changed();
  announce(sid, name);
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
    void make(sid, d.name, d.baud, d.device, d.label || '').resume();
  }
  changed();
}

/** Detach a device and stop it coming back next time. */
export async function detach(sid: string, name: string) {
  const entry = bridges.get(keyOf(sid, name));
  bridges.delete(keyOf(sid, name));
  try { entry?.channel?.postMessage({ type: 'state', state: 'connecting', baud: entry.baud, path: '' } satisfies FromHolder); } catch { /* closed */ }
  entry?.channel?.close();
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
