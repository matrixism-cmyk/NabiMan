import { tileZones, zoneRect } from './snapLayouts';
import { clamp, ConsoleRef, DEFAULT_H, DEFAULT_W, MIN_H, MIN_W, TASKBAR_H, TermWindow } from './windowGeometry';

/** The window manager's rules, apart from React: what a saved layout loads
 *  as, where a new window goes, and what the bar's buttons do to them all. */

export const LAYOUT_KEY = 'nabiman_term_windows';

/** One saved window as the record of today, or null when it cannot be one.
 *  Layouts saved before VM consoles had windows carry no kind: terminals. */
export function normalizeWindow(raw: unknown): TermWindow | null {
  const w = raw as Record<string, unknown> | null;
  if (!w || typeof w !== 'object' || !w.id) return null;
  if (w.kind === 'console') {
    const c = w.console as Partial<ConsoleRef> | undefined;
    if (!c || typeof c.name !== 'string' || !c.name) return null;
    const ref: ConsoleRef = { name: c.name, label: String(c.label || c.name), protocol: c.protocol === 'rdp' ? 'rdp' : 'vnc', builtin: Boolean(c.builtin) };
    return { ...w, kind: 'console', console: ref } as unknown as TermWindow;
  }
  if (!w.target) return null;
  return { ...w, kind: 'terminal' } as unknown as TermWindow;
}

export function loadLayout(key = LAYOUT_KEY): TermWindow[] {
  try {
    const raw = localStorage.getItem(key)
      || (key === `${LAYOUT_KEY}:admin` ? localStorage.getItem(LAYOUT_KEY) : null);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.map(normalizeWindow).filter((w): w is TermWindow => w !== null);
  } catch {
    return [];
  }
}

export function saveLayout(windows: TermWindow[], key = LAYOUT_KEY) {
  try { localStorage.setItem(key, JSON.stringify(windows));
    if (key === `${LAYOUT_KEY}:admin`) localStorage.removeItem(LAYOUT_KEY); } catch { /* storage blocked or full: only what this browser remembers is lost */ }
}

/** Where the next window opens: stepped down from the last few, inside the page. */
export function placeNew(count: number, vw: number, vh: number, want = { w: DEFAULT_W, h: DEFAULT_H }) {
  const w = Math.min(want.w, Math.max(MIN_W, vw - 40));
  const h = Math.min(want.h, Math.max(MIN_H, vh - 120));
  const offset = (count % 6) * 28;
  return {
    x: clamp(60 + offset, 0, Math.max(0, vw - w - 20)),
    y: clamp(70 + offset, 0, Math.max(0, vh - h - TASKBAR_H - 20)),
    w, h,
  };
}

/** "정렬": every window that shows, terminal or console, in a grid that fills the page. */
export function tileWindows(windows: TermWindow[], vw: number, vh: number): TermWindow[] {
  const open = windows.filter((w) => !w.minimized);
  if (open.length === 0) return windows;
  // The same places the layout picker drops a window into (snapLayouts).
  const zones = tileZones(open.length);
  let i = 0;
  return windows.map((w) => {
    if (w.minimized) return w;
    const place = zoneRect(zones[i], vw, vh);
    i += 1;
    return { ...w, maximized: false, ...place };
  });
}

/** "계단식": every window that shows, stepped down from the top left. */
export function cascadeWindows(windows: TermWindow[], vw: number, vh: number): TermWindow[] {
  let i = 0;
  return windows.map((w) => {
    if (w.minimized) return w;
    const offset = i * 32;
    i += 1;
    return {
      ...w, maximized: false,
      x: 40 + offset, y: 60 + offset,
      w: Math.min(DEFAULT_W, vw - offset - 80),
      h: Math.min(DEFAULT_H, vh - offset - 140),
    };
  });
}

/** Keeps windows reachable when the browser window shrinks. */
export function clampWindows(windows: TermWindow[], vw: number, vh: number): TermWindow[] {
  return windows.map((w) => ({
    ...w,
    x: clamp(w.x, 0, Math.max(0, vw - 120)),
    y: clamp(w.y, 0, Math.max(0, vh - 80)),
  }));
}

/** The window the keys belong to: the top one of those that show. */
export function focusedWindowId(windows: TermWindow[]): string | null {
  let top: TermWindow | null = null;
  windows.forEach((w) => { if (!w.minimized && (!top || w.z > top.z)) top = w; });
  return top ? (top as TermWindow).id : null;
}

export const consoleWindowOf = (windows: TermWindow[], name: string) =>
  windows.find((w) => w.kind === 'console' && w.console.name === name);

type Listed = { name: string; label: string; protocol?: 'vnc' | 'rdp'; builtin: boolean };

/**
 * Console windows against the consoles the account may open, once that list
 * has loaded: a window whose console is no longer listed goes (deleted, or
 * access taken away), and the others take the list's label and protocol.
 * `null` is an account that may not open consoles at all. The same array comes
 * back when nothing changed.
 */
export function keepListedConsoles(windows: TermWindow[], listed: Listed[] | null): TermWindow[] {
  let changed = false;
  const next: TermWindow[] = [];
  windows.forEach((w) => {
    if (w.kind !== 'console') { next.push(w); return; }
    const now = listed?.find((c) => c.name === w.console.name);
    if (!now) { changed = true; return; }
    const protocol = now.protocol === 'rdp' ? 'rdp' : 'vnc';
    if (now.label === w.console.label && protocol === w.console.protocol && Boolean(now.builtin) === w.console.builtin) { next.push(w); return; }
    changed = true;
    next.push({ ...w, title: now.label, subtitle: protocol.toUpperCase(), console: { name: now.name, label: now.label, protocol, builtin: Boolean(now.builtin) } });
  });
  return changed ? next : windows;
}
