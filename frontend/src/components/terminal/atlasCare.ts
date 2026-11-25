import type { IDisposable, Terminal } from 'xterm';

/** How long after the last change the letters are drawn afresh. */
export const SETTLE_MS = 220;

/**
 * Keeps WebGL terminals' letters whole.
 *
 * The WebGL renderer draws every letter once into a picture (the texture
 * atlas) and copies it from there — and terminals with the same font share
 * ONE such picture. Clearing it through one terminal (`clearTextureAtlas`,
 * which a font that has just loaded, a size change or the ↻ button ask for)
 * makes that terminal forget where its letters were and draw them again, but
 * the others still point at the old places in a picture that is now being
 * filled with different letters: their new text comes out as slices of other
 * letters. So the picture is only ever cleared for every terminal at once:
 * each forgets, then each draws again.
 */
const cared = new Set<Terminal>();
let timer: ReturnType<typeof setTimeout> | undefined;

/** Draws every WebGL terminal's letters afresh, now. */
export function freshLetters(): void {
  if (timer) { clearTimeout(timer); timer = undefined; }
  const terms = Array.from(cared);
  // All of them forget first (each call empties the shared picture again, which is harmless while nothing has drawn)…
  for (const term of terms) { try { term.clearTextureAtlas(); } catch { cared.delete(term); /* the terminal is gone */ } }
  // …then all of them draw.
  for (const term of terms) { try { term.refresh(0, term.rows - 1); } catch { /* gone */ } }
}

/** The same, once things have settled: a burst of size changes ends in one redraw. */
export function freshLettersSoon(): void {
  if (timer) clearTimeout(timer);
  timer = setTimeout(freshLetters, SETTLE_MS);
}

/**
 * Asks for `term`'s letters to be drawn afresh. For a terminal that shares
 * its picture with others that means all of them; a terminal that does not
 * draw with WebGL (not cared for) is cleared on its own, as before.
 */
export function clearLetters(term: Terminal): void {
  if (cared.has(term)) { freshLetters(); return; }
  try { term.clearTextureAtlas(); term.refresh(0, term.rows - 1); } catch { /* the terminal is gone */ }
}

let watching: (() => void) | null = null;

/** The page-wide reasons to redraw: back from the background, another zoom or screen density. */
function watchPage(win: Window): () => void {
  const shown = () => { if (win.document.visibilityState === 'visible') freshLettersSoon(); };
  win.document.addEventListener('visibilitychange', shown);
  let density: MediaQueryList | null = null;
  const densityChanged = () => { freshLettersSoon(); watchDensity(); };
  function watchDensity() {
    density?.removeEventListener?.('change', densityChanged);
    density = win.matchMedia ? win.matchMedia(`(resolution: ${win.devicePixelRatio || 1}dppx)`) : null;
    density?.addEventListener?.('change', densityChanged);
  }
  watchDensity();
  return () => {
    win.document.removeEventListener('visibilitychange', shown);
    density?.removeEventListener?.('change', densityChanged);
  };
}

/** Looks after a WebGL terminal until disposed: it joins the others, and they are all redrawn when it comes, goes or changes size. */
export function careForAtlas(term: Terminal, win: Window = window): IDisposable {
  cared.add(term);
  if (!watching) watching = watchPage(win);
  const resized: IDisposable | undefined = term.onResize?.(freshLettersSoon);
  // A newcomer starts writing into the picture the others are using.
  freshLettersSoon();
  return {
    dispose: () => {
      resized?.dispose();
      cared.delete(term);
      if (cared.size === 0) {
        if (timer) { clearTimeout(timer); timer = undefined; }
        watching?.();
        watching = null;
      } else {
        freshLettersSoon();
      }
    },
  };
}
