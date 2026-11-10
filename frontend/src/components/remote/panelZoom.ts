import type React from 'react';

/**
 * Ctrl + wheel over a panel (the server rail, the file panel, a list of
 * sessions) zooms that panel alone, as Ctrl + wheel over a terminal changes
 * only its text size; the browser would otherwise zoom the whole page.
 *
 * A panel opts in with `data-zoom-key="<name>"` and `style={zoomStyle(name)}`;
 * its zoom is remembered in this browser. Terminals are left to their own
 * handler: a zoomed ancestor would throw off xterm's mouse mapping, so no
 * panel that holds a terminal should opt in.
 */

const MIN = 0.7;
const MAX = 1.6;
const STEP = 0.1;
const storeKey = (key: string) => `nabiman_zoom_${key}`;

export function zoomOf(key: string): number {
  try {
    const v = Number(localStorage.getItem(storeKey(key)));
    return v >= MIN && v <= MAX ? v : 1;
  } catch {
    return 1;
  }
}

/** The style a zoomable panel renders with. */
export const zoomStyle = (key: string): React.CSSProperties => {
  const z = zoomOf(key);
  return z === 1 ? {} : { zoom: z };
};

/** The zoom an element is drawn at (its own and its ancestors'), for mouse
 *  positions given to fixed menus inside it, which the zoom scales again. */
export function zoomAt(el: Element | null | undefined): number {
  let z = 1;
  for (let e: Element | null | undefined = el; e; e = e.parentElement) {
    const own = Number(getComputedStyle(e).getPropertyValue("zoom") || 1);
    if (own > 0 && Number.isFinite(own)) z *= own;
  }
  return z;
}

function onWheel(e: WheelEvent) {
  if (!e.ctrlKey) return;
  const target = e.target as Element | null;
  if (!target || target.closest('.xterm')) return;
  const panel = target.closest<HTMLElement>('[data-zoom-key]');
  if (!panel) return;
  e.preventDefault();
  const key = panel.dataset.zoomKey || '';
  const next = Math.round(Math.min(MAX, Math.max(MIN, zoomOf(key) + (e.deltaY < 0 ? STEP : -STEP))) * 10) / 10;
  try {
    if (next === 1) localStorage.removeItem(storeKey(key));
    else localStorage.setItem(storeKey(key), String(next));
  } catch { /* private mode: the zoom lasts until reload */ }
  // Every panel of the same kind on the page follows (a popped-out list, say).
  document.querySelectorAll<HTMLElement>(`[data-zoom-key="${CSS.escape(key)}"]`).forEach((p) => {
    if (next === 1) p.style.removeProperty("zoom"); else p.style.setProperty("zoom", String(next));
  });
  window.dispatchEvent(new Event('resize'));
}

let installed = false;
/** Listen on the whole page once; panels opt in by their attribute. */
export function installPanelZoom() {
  if (installed || typeof document === 'undefined') return;
  installed = true;
  document.addEventListener('wheel', onWheel, { passive: false, capture: true });
}
