/**
 * The map's WebGL context, should the browser take it back (too many contexts
 * open, a GPU reset, the tab long in the background): the event is cancelled,
 * which is what lets the browser give it back, and once it is back the scene is
 * drawn again. While it is gone the canvas shows nothing — the stage behind it.
 */
export interface ContextCounts { lost: number; restored: number }

interface Canvas {
  addEventListener(type: string, fn: (e: { preventDefault(): void }) => void): void;
  removeEventListener(type: string, fn: (e: { preventDefault(): void }) => void): void;
}

export function watchContext(canvas: Canvas, onRestored: () => void): (() => void) & { counts: ContextCounts } {
  const counts: ContextCounts = { lost: 0, restored: 0 };
  const lost = (e: { preventDefault(): void }) => { e.preventDefault(); counts.lost += 1; };
  const restored = () => { counts.restored += 1; onRestored(); };
  canvas.addEventListener('webglcontextlost', lost);
  canvas.addEventListener('webglcontextrestored', restored);
  const stop = () => {
    canvas.removeEventListener('webglcontextlost', lost);
    canvas.removeEventListener('webglcontextrestored', restored);
  };
  return Object.assign(stop, { counts });
}
