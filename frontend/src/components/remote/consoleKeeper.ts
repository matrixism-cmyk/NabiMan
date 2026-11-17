import { useSyncExternalStore } from 'react';

/** A console left open but out of sight is disconnected after this long. */
export const KEEP_HIDDEN_MS = 30 * 60 * 1000;
/** How many console connections one browser tab holds at once. */
export const KEEP_MAX = 3;

/** A console connection the keeper can hold on to while no view shows it. */
export interface KeptSession {
  /** Ends the connection for good. */
  close(): void;
  /** Whether a connection is up or on its way — something worth keeping. */
  alive(): boolean;
  /** Calls `changed` whenever that may have changed; returns how to stop. */
  watch(changed: () => void): () => void;
}

interface Entry<S> { session: S; hiddenAt: number | null; timer?: ReturnType<typeof setTimeout>; unwatch: () => void }

/**
 * Holds VM console connections outside the React tree, so leaving a console
 * (another server, another menu) does not hang up: the view lets go of the
 * screen and the keeper keeps the connection until the view comes back, the
 * console has been out of sight for KEEP_HIDDEN_MS, a newer console needs its
 * place (KEEP_MAX), or the account signs out.
 */
export function createKeeper<S extends KeptSession = KeptSession>(now: () => number = Date.now) {
  const entries = new Map<string, Entry<S>>();
  const listeners = new Set<() => void>();
  let hidden: string[] = [];

  const publish = () => {
    const next = Array.from(entries).filter(([, e]) => e.hiddenAt !== null && e.session.alive()).map(([key]) => key);
    if (next.length === hidden.length && next.every((key, i) => key === hidden[i])) return;
    hidden = next;
    listeners.forEach((fn) => fn());
  };

  const drop = (key: string) => {
    const entry = entries.get(key);
    if (!entry) return;
    entries.delete(key);
    if (entry.timer) clearTimeout(entry.timer);
    entry.unwatch();
    try { entry.session.close(); } catch { /* already gone */ }
    publish();
  };

  /** Over the limit, the console out of sight the longest goes first. */
  const makeRoom = () => {
    while (entries.size > KEEP_MAX) {
      const longest = Array.from(entries.entries()).filter(([, e]) => e.hiddenAt !== null)
        .sort((x, y) => (x[1].hiddenAt as number) - (y[1].hiddenAt as number))[0];
      if (!longest) return;
      drop(longest[0]);
    }
  };

  /** A view shows the console again. */
  const show = (key: string) => {
    const entry = entries.get(key);
    if (!entry) return;
    if (entry.timer) clearTimeout(entry.timer);
    entry.timer = undefined;
    entry.hiddenAt = null;
    publish();
  };

  return {
    get: (key: string): S | undefined => entries.get(key)?.session,
    /** Takes a new connection, shown by a view right now. */
    adopt(key: string, session: S) {
      if (entries.get(key)?.session === session) { show(key); return; }
      drop(key);
      const unwatch = session.watch(() => {
        const entry = entries.get(key);
        // Nothing is kept for a console that ended while out of sight.
        if (entry?.session === session && entry.hiddenAt !== null && !session.alive()) drop(key);
        else publish();
      });
      entries.set(key, { session, hiddenAt: null, unwatch });
      makeRoom();
      publish();
    },
    show,
    /** The view let go: keep the connection, for a while. */
    hide(key: string) {
      const entry = entries.get(key);
      if (!entry) return;
      if (!entry.session.alive()) { drop(key); return; }
      if (entry.timer) clearTimeout(entry.timer);
      entry.hiddenAt = now();
      entry.timer = setTimeout(() => drop(key), KEEP_HIDDEN_MS);
      publish();
    },
    drop,
    dropAll() { Array.from(entries.keys()).forEach(drop); },
    /** Drops every console not among `allowed` (the ones the account may open). */
    retain(allowed: string[]) {
      Array.from(entries.keys()).filter((key) => !allowed.includes(key)).forEach(drop);
    },
    /** The consoles kept connected out of sight. The same array until it changes. */
    hidden: () => hidden,
    size: () => entries.size,
    subscribe(fn: () => void) { listeners.add(fn); return () => { listeners.delete(fn); }; },
  };
}

/** This tab's console connections. A popout window or a share page has its own. */
export const consoleKeeper = createKeeper();

// Signing out (or in as someone else) hangs up everything that was kept.
if (typeof window !== 'undefined') window.addEventListener('nabiman_auth_changed', () => consoleKeeper.dropAll());

/** The keys of the consoles kept connected while not on screen. */
export function useKeptConsoles(): string[] {
  return useSyncExternalStore(consoleKeeper.subscribe, consoleKeeper.hidden);
}
