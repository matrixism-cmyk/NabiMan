/** The closest a pinch goes: one VM pixel to this many screen pixels. */
const MAX_SCALE = 3;
/** On a touchpad a two-finger swipe scrolls; it is a pinch once the fingers'
 *  distance has changed by this much. */
const PINCH_START = 0.12;
/** Below this the screen snaps back to fitting whole. */
const SNAP = 1.05;
/** How close the touchpad's pointer may come to the edge before the view follows it. */
const FOLLOW_MARGIN = 40;

/**
 * Zooming a VM's screen on a phone, where the whole desktop fitted on a small
 * screen is hard to read and to hit. Two fingers pinch to zoom in or out
 * around where they are and, zoomed in, drag to move about; the screen is
 * drawn bigger (Guacamole's own scale) inside `stage`, which scrolls. The
 * pointer math already counts the stage's scroll, so taps land where they
 * look. With "touchpad" touch, a two-finger swipe stays the VM's scroll wheel
 * until it turns into a pinch, and the view follows the pointer instead.
 */
export function rdpZoom({ stage, display, pinchOnly, onZoom, onRescale }: {
  stage: HTMLElement;
  display: any;
  pinchOnly: boolean;
  /** 1 is the whole screen fitted in. */
  onZoom: (zoom: number) => void;
  /** The scale was multiplied by `ratio` (a touchpad keeps its pointer where it was). */
  onRescale?: (ratio: number) => void;
}) {
  let zoom = 1;
  const el: HTMLElement = display.getElement();
  const fitScale = () => {
    const w = display.getWidth(), h = display.getHeight();
    return w && h ? Math.min(stage.clientWidth / w, stage.clientHeight / h, 1) : 0;
  };

  /** Draws at `zoom`. With an anchor, the VM point under (x, y) — in the
   *  stage — ends up under (tx, ty). */
  const apply = (anchor?: { x: number; y: number; tx: number; ty: number }) => {
    const base = fitScale();
    if (!base) return;
    const old = display.getScale() || base;
    const next = base * zoom;
    const px = anchor ? (stage.scrollLeft + anchor.x - el.offsetLeft) / old : 0;
    const py = anchor ? (stage.scrollTop + anchor.y - el.offsetTop) / old : 0;
    display.scale(next);
    if (anchor) {
      stage.scrollLeft = px * next + el.offsetLeft - anchor.tx;
      stage.scrollTop = py * next + el.offsetTop - anchor.ty;
    }
    if (old !== next) onRescale?.(next / old);
  };

  let pinch: { d0: number; z0: number; x: number; y: number; ours: boolean } | null = null;
  const fingers = (e: TouchEvent) => {
    const [a, b] = [e.touches[0], e.touches[1]];
    const r = stage.getBoundingClientRect();
    return {
      d: Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY) || 1,
      x: (a.clientX + b.clientX) / 2 - r.left,
      y: (a.clientY + b.clientY) / 2 - r.top,
    };
  };
  const start = (e: TouchEvent) => {
    if (e.touches.length !== 2) { pinch = null; return; }
    const g = fingers(e);
    pinch = { d0: g.d, z0: zoom, x: g.x, y: g.y, ours: !pinchOnly };
    e.preventDefault();
  };
  const move = (e: TouchEvent) => {
    if (!pinch || e.touches.length !== 2) return;
    e.preventDefault();
    const g = fingers(e);
    if (!pinch.ours) {
      if (Math.abs(g.d / pinch.d0 - 1) < PINCH_START) return;
      pinch.ours = true;
    }
    // Ours from here: the touchpad must not scroll the VM with it as well.
    e.stopPropagation();
    const most = Math.max(1, MAX_SCALE / (fitScale() || 1));
    zoom = Math.min(most, Math.max(1, pinch.z0 * (g.d / pinch.d0)));
    apply({ x: pinch.x, y: pinch.y, tx: g.x, ty: g.y });
    pinch.x = g.x; pinch.y = g.y;
  };
  const end = (e: TouchEvent) => {
    if (!pinch || e.touches.length >= 2) return;
    pinch = null;
    if (zoom < SNAP && zoom !== 1) { zoom = 1; apply(); }
    onZoom(zoom);
  };

  // Capture on the stage runs before Guacamole's own handlers on the screen inside it.
  const opts = { capture: true, passive: false } as const;
  stage.addEventListener('touchstart', start, opts);
  stage.addEventListener('touchmove', move, opts);
  stage.addEventListener('touchend', end, opts);
  stage.addEventListener('touchcancel', end, opts);

  return {
    /** Fits again after the window or the VM's resolution changed, keeping the zoom. */
    fit: () => apply(),
    reset: () => { zoom = 1; stage.scrollLeft = 0; stage.scrollTop = 0; apply(); onZoom(1); },
    /** Keeps a pointer at (x, y) — on the scaled screen — in view. */
    follow: (x: number, y: number) => {
      if (zoom <= 1) return;
      const cx = el.offsetLeft + x, cy = el.offsetTop + y, m = FOLLOW_MARGIN;
      if (cx < stage.scrollLeft + m) stage.scrollLeft = cx - m;
      else if (cx > stage.scrollLeft + stage.clientWidth - m) stage.scrollLeft = cx - stage.clientWidth + m;
      if (cy < stage.scrollTop + m) stage.scrollTop = cy - m;
      else if (cy > stage.scrollTop + stage.clientHeight - m) stage.scrollTop = cy - stage.clientHeight + m;
    },
    detach: () => {
      stage.removeEventListener('touchstart', start, opts);
      stage.removeEventListener('touchmove', move, opts);
      stage.removeEventListener('touchend', end, opts);
      stage.removeEventListener('touchcancel', end, opts);
    },
  };
}
