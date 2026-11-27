import type { TermWindow } from './windowGeometry';

/**
 * "모두 숨기기 / 모두 보이기" on the terminal bar.
 *
 * While any window shows, the button hides them all (minimised: each keeps
 * its session, its screen and its place) and remembers which ones were
 * showing. With none showing it brings those back — not the ones that were
 * already minimised before — or every window when there is nothing to go by
 * (they were minimised one by one, or the page was reloaded).
 */
export function toggleAllWindows(windows: TermWindow[], remembered: string[]): { windows: TermWindow[]; remembered: string[] } {
  const showing = windows.filter((w) => !w.minimized).map((w) => w.id);
  if (showing.length > 0) {
    return { windows: windows.map((w) => (w.minimized ? w : { ...w, minimized: true })), remembered: showing };
  }
  const back = windows.filter((w) => remembered.includes(w.id)).map((w) => w.id);
  const ids = back.length > 0 ? back : windows.map((w) => w.id);
  return { windows: windows.map((w) => (ids.includes(w.id) ? { ...w, minimized: false } : w)), remembered };
}
