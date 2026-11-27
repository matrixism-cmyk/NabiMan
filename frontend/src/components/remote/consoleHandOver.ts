import { consoleKeeper } from './consoleKeeper';
import { openConsoleWindow, VmConsole } from './vncApi';

const EVENT = 'nabiman_console_popout';

/**
 * "새창으로 분리": the console goes to a browser window of its own, which
 * makes its own connection. This tab hangs up first — two RDP logins of one
 * account throw each other out — and whatever showed the console here (a
 * window on the page, the workspace's pane) is told to let go.
 * Returns the browser window, or null when the browser refused to open one.
 */
export function popOutConsole(c: Pick<VmConsole, 'name' | 'protocol'>): Window | null {
  consoleKeeper.drop(c.name);
  const popup = openConsoleWindow(c);
  if (!popup) return null;
  window.dispatchEvent(new CustomEvent<string>(EVENT, { detail: c.name }));
  try { popup.focus(); } catch { /* the browser keeps the focus where it is */ }
  return popup;
}

/** Calls `gone` with the console's name whenever one was handed to a browser window. */
export function onConsolePoppedOut(gone: (name: string) => void): () => void {
  const heard = (e: Event) => gone((e as CustomEvent<string>).detail);
  window.addEventListener(EVENT, heard);
  return () => window.removeEventListener(EVENT, heard);
}
