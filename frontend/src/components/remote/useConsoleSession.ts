import { useMemo, useSyncExternalStore } from 'react';
import { consoleKeeper } from './consoleKeeper';
import type { ConsoleSession } from './consoleSession';
import { consoleSnapshots } from './consoleSnapshots';

/**
 * The connection behind a console view. In the workspace (`kept`) it is the
 * keeper's: found there when the console was left open earlier, and left there
 * — still connected — when the view goes away. A popout window or a share
 * page owns its connection and closes it with the view.
 */
export function useConsoleSession<S extends ConsoleSession>(
  key: string, kept: boolean, kind: new (...args: any[]) => S, make: () => S,
) {
  const session = useMemo(() => {
    const held = kept ? consoleKeeper.get(key) : undefined;
    return held instanceof kind ? held : make();
  }, [key, kept]); // eslint-disable-line react-hooks/exhaustive-deps
  useSyncExternalStore(session.watch, session.version);

  return useMemo(() => ({
    session,
    /** The view is up and has the screen: connect, unless it already is. */
    begin() { if (kept) consoleKeeper.adopt(key, session); session.start(); },
    /** The view goes away. */
    leave() { if (kept) consoleKeeper.hide(key); else session.close(); },
    /** Connect again, as the user asked. */
    reconnect() { if (kept) consoleKeeper.adopt(key, session); session.reconnect(); },
    /** Disconnect, as the user asked. */
    hangUp() {
      if (kept && consoleKeeper.get(key) === session) consoleKeeper.drop(key); else session.close();
      // Hung up on purpose: its picture goes too.
      if (kept) consoleSnapshots.forget(key);
    },
  }), [session, key, kept]);
}
