import { useSyncExternalStore } from 'react';
import { consoleKeeper } from './consoleKeeper';

/** A snapshot is at most this wide: enough for a thumbnail, too small to read. */
export const SHOT_WIDTH = 320;
/** Older than this, a picture is "the screen last seen", not the screen now. */
export const SHOT_FRESH_MS = 30000;
/** How many consoles' pictures one tab remembers. */
export const SHOT_MAX = 12;

export interface ConsoleShot {
  /** A small JPEG, as a data URL. */
  url: string;
  /** When it was taken (ms). */
  at: number;
}

/** What the snapshots need of the console keeper. */
interface Keeper {
  get(key: string): unknown;
  hidden(): string[];
  subscribe(fn: () => void): () => void;
}

/** What they need of a console session, RDP or VNC, without loading either library. */
interface Shown {
  alive?: () => boolean;
  status?: string;
  /** RDP: Guacamole's client; its display flattens its layers into one canvas. */
  client?: { getDisplay?: () => { flatten?: () => HTMLCanvasElement } | null } | null;
  /** VNC: the element noVNC draws its canvas in. */
  screen?: HTMLElement;
}

const isUp = (session: Shown | undefined) =>
  Boolean(session && session.alive?.() && (session.status === undefined || session.status === 'connected'));

/** The canvas the console's screen is on, if it has drawn one. */
function canvasOf(session: Shown): HTMLCanvasElement | null {
  const display = session.client?.getDisplay?.();
  if (display?.flatten) return display.flatten();
  return session.screen?.querySelector?.('canvas') || null;
}

/** The screen, scaled down. Throws when the canvas may not be read. */
function shrink(canvas: HTMLCanvasElement): string {
  if (!canvas.width || !canvas.height) return '';
  const small = document.createElement('canvas');
  small.width = Math.min(SHOT_WIDTH, canvas.width);
  small.height = Math.max(1, Math.round(canvas.height * small.width / canvas.width));
  const context = small.getContext('2d');
  if (!context) return '';
  context.drawImage(canvas, 0, 0, small.width, small.height);
  const url = small.toDataURL('image/jpeg', 0.6);
  return url.startsWith('data:image/') ? url : '';
}

/**
 * Small pictures of the VM consoles this tab is connected to, for the start
 * menu to tell them apart. A picture is only ever taken from a connection that
 * is already up here (on show or kept): nothing connects to make one. They
 * live in this module's memory and nowhere else — a screen may show private
 * things — and go when the console is hung up on purpose, when the account may
 * no longer open it, and when the account signs out. A console whose
 * connection ended some other way keeps its last picture, as the screen last
 * seen.
 */
export function createSnapshots(keeper: Keeper, now: () => number = Date.now) {
  const shots = new Map<string, ConsoleShot>();
  const listeners = new Set<() => void>();
  let changes = 0;
  const publish = () => { changes += 1; listeners.forEach((fn) => fn()); };

  /** Takes the picture of one console, if this tab has it connected and its screen can be read. */
  const take = (key: string): boolean => {
    const session = keeper.get(key) as Shown | undefined;
    if (!isUp(session)) return false;
    let url = '';
    try {
      const canvas = canvasOf(session as Shown);
      if (canvas) url = shrink(canvas);
    } catch { /* a screen that may not be read has no picture */ }
    if (!url) return false;
    shots.delete(key); // newest last
    shots.set(key, { url, at: now() });
    while (shots.size > SHOT_MAX) shots.delete(shots.keys().next().value as string);
    return true;
  };

  const refresh = (keys: string[]) => { if (keys.map(take).some(Boolean)) publish(); };
  const remove = (keys: string[]) => { if (keys.map((key) => shots.delete(key)).some(Boolean)) publish(); };

  // A console going out of sight: the screen as it was left.
  const unwatch = keeper.subscribe(() => refresh(keeper.hidden()));

  return {
    /** Takes fresh pictures of those of `keys` that are connected here. */
    refresh,
    /** The picture of a console, if there is one. */
    shot: (key: string): ConsoleShot | undefined => shots.get(key),
    /** Whether this tab has the console connected right now. */
    held: (key: string) => isUp(keeper.get(key) as Shown | undefined),
    /** Whether the picture is the screen as it is now, rather than as last seen. */
    fresh: (key: string) => {
      const shot = shots.get(key);
      return Boolean(shot) && isUp(keeper.get(key) as Shown | undefined) && now() - (shot as ConsoleShot).at < SHOT_FRESH_MS;
    },
    /** The console was hung up on purpose (or deleted). */
    forget: (key: string) => remove([key]),
    /** Keeps only the consoles the account may open. */
    retain: (allowed: string[]) => remove(Array.from(shots.keys()).filter((key) => !allowed.includes(key))),
    clear: () => remove(Array.from(shots.keys())),
    size: () => shots.size,
    version: () => changes,
    subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn); }; },
    stop: unwatch,
  };
}

/** This tab's console pictures. */
export const consoleSnapshots = createSnapshots(consoleKeeper);

// Signing out (or in as someone else) leaves no picture behind.
if (typeof window !== 'undefined') window.addEventListener('nabiman_auth_changed', () => consoleSnapshots.clear());

/** The pictures, re-rendering when one changes. */
export function useConsoleSnapshots() {
  useSyncExternalStore(consoleSnapshots.subscribe, consoleSnapshots.version);
  return consoleSnapshots;
}
