import { useEffect, useLayoutEffect, useRef } from 'react';
import { useTerminalWindows } from '../terminal/TerminalWindows';
import { onConsolePoppedOut, popOutConsole } from './consoleHandOver';
import type { VmConsole } from './vncApi';
import type { Selection } from './workspaceState';

/** What the workspace's pane needs to know about the selected console's window. */
export interface ConsolePlace {
  /** The selected console is open as a window on this page: the pane does not show it too. */
  windowed: boolean;
  /** "분리": the console goes to a window on this page. */
  detach: (c: VmConsole) => void;
  /** "새창으로 분리": the console goes to a browser window of its own. */
  popOut: (c: Pick<VmConsole, 'name' | 'protocol'>) => void;
  /** "창으로 가기": brings the window to the front. */
  goToWindow: () => void;
  /** "여기로 가져오기": closes the window, so the pane shows the console. */
  bringHere: () => void;
}

/**
 * One connection, one place. A console shows either in the workspace's pane
 * or in a window, never both: when a window opens for the console the pane
 * shows (from here, the rail or the start menu), the pane lets go and goes to
 * the overview — as a terminal's 분리 does; selecting a console that already
 * has a window leaves the window alone and the pane says where it is.
 */
export function useConsoleWindows({ selection, select, consoles }: {
  selection: Selection;
  select: (next: Selection) => void;
  consoles: VmConsole[] | null;
}): ConsolePlace {
  const { windows, openConsole, focusWindow, closeWindow, syncConsoles } = useTerminalWindows();
  const names = windows.flatMap((w) => (w.kind === 'console' ? [w.console.name] : []));
  const key = names.join('\n');
  const shown = selection.kind === 'console' ? selection.vm : null;
  const here = shown === null ? undefined : windows.find((w) => w.kind === 'console' && w.console.name === shown);

  // A console the account may no longer open has no window either.
  useEffect(() => { if (consoles) syncConsoles(consoles); }, [consoles, syncConsoles]);

  // A window that was not there a moment ago, for the console the pane shows: it moved.
  const before = useRef(names);
  useLayoutEffect(() => {
    const had = before.current;
    before.current = names;
    if (shown !== null && names.includes(shown) && !had.includes(shown)) select({ kind: 'overview' });
  }, [key]); // eslint-disable-line react-hooks/exhaustive-deps

  // Handed to a browser window: the pane lets go as well.
  useEffect(() => onConsolePoppedOut((name) => { if (name === shown) select({ kind: 'overview' }); }), [shown, select]);

  return {
    windowed: Boolean(here),
    detach: openConsole,
    popOut: popOutConsole,
    goToWindow: () => { if (here) focusWindow(here.id); },
    bringHere: () => { if (here) closeWindow(here.id); },
  };
}
