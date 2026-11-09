/**
 * How a serial monitor window talks to the window that holds the port.
 *
 * The board is bridged by one tab (PortBridge, under a Web Lock); a monitor
 * opened in a window of its own cannot open the same port again, so it
 * listens on a BroadcastChannel per device instead. The holder announces its
 * state, passes on every chunk the board sends, answers a newcomer with what
 * was said so far, and carries out what the monitor asks: send these bytes,
 * reset, bootloader.
 */

export type MonitorState = 'connected' | 'waiting' | 'connecting';

export type FromHolder =
  | { type: 'state'; state: MonitorState; baud: number; path: string; device?: string; label?: string }
  | { type: 'data'; bytes: Uint8Array }
  | { type: 'backlog'; text: string }
  | { type: 'done'; what: 'send' | 'reset' | 'bootloader'; ok: boolean; error?: string };

export type FromMonitor =
  | { type: 'hello' }
  | { type: 'send'; bytes: Uint8Array }
  | { type: 'reset' }
  | { type: 'bootloader' };

export const channelName = (sid: string, name: string) => `nabiman-serial-monitor:${sid}/${name}`;

/** The page a monitor opens in, and the window name that keeps it to one per device. */
export const monitorUrl = (sid: string, name: string) => `/remote#serial/${encodeURIComponent(sid)}/${encodeURIComponent(name)}`;
export const monitorWindow = (sid: string, name: string) => `serialmon_${sid}_${name}`;

export function openMonitor(sid: string, name: string) {
  window.open(monitorUrl(sid, name), monitorWindow(sid, name), 'width=960,height=640,menubar=no,toolbar=no,location=no');
}

/** `#serial/<sid>/<name>` on /remote, or null. */
export function monitorTarget(path = window.location.pathname, hash = window.location.hash): { sid: string; name: string } | null {
  if (!/^\/remote\/?$/.test(path)) return null;
  const m = /^#serial\/([^/]+)\/([^/]+)$/.exec(hash);
  if (!m) return null;
  try { return { sid: decodeURIComponent(m[1]), name: decodeURIComponent(m[2]) }; } catch { return null; }
}
