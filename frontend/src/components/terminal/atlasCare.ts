import type { IDisposable, Terminal } from 'xterm';

/** How long after the last change of size the glyphs are drawn afresh. */
export const SETTLE_MS = 220;

/**
 * Keeps a WebGL terminal's letters whole.
 *
 * The WebGL renderer draws every letter once into a shared picture (the
 * texture atlas) and copies it from there. After the terminal's canvas has
 * changed size several times in a row — a window dropped into a layout,
 * tiled, maximised and dragged loose — or the page's zoom or the screen's
 * pixel density has changed, or the tab has been in the background, that
 * picture can end up stale and letters come out as slices of other letters.
 * Throwing the picture away makes the renderer draw each letter again, which
 * is what the window's ↻ button does by hand; this does it once things have
 * settled, so nobody has to.
 */
export function careForAtlas(term: Terminal, win: Window = window): IDisposable {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const fresh = () => {
    timer = undefined;
    try { term.clearTextureAtlas(); term.refresh(0, term.rows - 1); } catch { /* the terminal is gone */ }
  };
  const soon = () => { if (timer) clearTimeout(timer); timer = setTimeout(fresh, SETTLE_MS); };

  const resized: IDisposable | undefined = term.onResize?.(soon);
  const shown = () => { if (win.document.visibilityState === 'visible') soon(); };
  win.document.addEventListener('visibilitychange', shown);
  // The page's zoom and a move to another screen both change the pixel density.
  let density: MediaQueryList | null = null;
  const watchDensity = () => {
    density?.removeEventListener?.('change', densityChanged);
    density = win.matchMedia ? win.matchMedia(`(resolution: ${win.devicePixelRatio || 1}dppx)`) : null;
    density?.addEventListener?.('change', densityChanged);
  };
  function densityChanged() { soon(); watchDensity(); }
  watchDensity();

  return {
    dispose: () => {
      if (timer) clearTimeout(timer);
      resized?.dispose();
      win.document.removeEventListener('visibilitychange', shown);
      density?.removeEventListener?.('change', densityChanged);
    },
  };
}
