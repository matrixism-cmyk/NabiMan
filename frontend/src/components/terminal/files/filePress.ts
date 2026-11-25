/**
 * Press and hold on something in a terminal.
 *
 * A terminal acts on the press itself: xterm starts a selection, and with
 * mouse reporting on (tmux always has it on here; vim or Claude Code inside
 * may too) the press is sent to the program at once. So a press that lands on
 * a candidate is held back for a moment instead of being let through:
 *
 * - released early, or moved (a drag, a selection): the press is handed to the
 *   terminal as it was, then the release or the movement follows as usual —
 *   a click and a drag do what they always did, the press just arrives with
 *   the release or the first movement;
 * - held, and the candidate turns out to be nothing: the press is handed over
 *   then;
 * - held, and it is something: the menu opens, and the terminal — and the
 *   program in it — never hears of that press or its release.
 *
 * A press with a modifier key, another button or a double click is never
 * held back. A finger works the same way, except that nothing needs holding
 * back: a touch only becomes mouse events once it is lifted.
 */
export interface LongPressOptions<C, R> {
  el: HTMLElement;
  /** Cheap and synchronous: what is under this point, or null for nothing worth waiting on. */
  candidateAt: (clientX: number, clientY: number) => C | null;
  /** Asked once when the press starts: what to show for it, or null. */
  resolve: (candidate: C) => Promise<R | null>;
  open: (result: R, clientX: number, clientY: number) => void;
  holdMs?: number;
  /** How far the pointer may wander and still be holding. */
  slopPx?: number;
  /** How long past the hold an answer is waited for. */
  waitMs?: number;
}

export const HOLD_MS = 450;
const SLOP_MOUSE = 4;
const SLOP_TOUCH = 10;
const WAIT_MS = 4000;
/** After a finger opened the menu, the browser's own long-press menu is not ours to answer. */
const QUIET_MS = 1500;

interface Press<R> {
  touch: boolean;
  x: number;
  y: number;
  target: EventTarget | null;
  init: MouseEventInit;
  held: boolean;
  /** `undefined` until the answer is in. */
  result?: R | null;
  timers: number[];
}

export function attachLongPress<C, R>(options: LongPressOptions<C, R>): () => void {
  const { el, candidateAt, resolve, open } = options;
  const holdMs = options.holdMs ?? HOLD_MS;
  const waitMs = options.waitMs ?? WAIT_MS;
  let press: Press<R> | null = null;
  let replaying = false;
  /** The button (or finger) that opened the menu is still down. */
  let swallowRelease: 'mouse' | 'touch' | null = null;
  let quietUntil = 0;

  const clear = () => {
    press?.timers.forEach((timer) => window.clearTimeout(timer));
    press = null;
  };
  /** Gives the terminal the press that was held back. */
  const handOver = () => {
    const was = press;
    clear();
    if (!was || was.touch || !was.target) return;
    replaying = true;
    try {
      was.target.dispatchEvent(new MouseEvent('mousedown', was.init));
    } finally {
      replaying = false;
    }
  };
  const settle = () => {
    const now = press;
    if (!now || now.result === undefined) return;
    if (now.result === null) { handOver(); return; }
    if (!now.held) return;
    const { result, x, y, touch } = now;
    clear();
    swallowRelease = touch ? 'touch' : 'mouse';
    if (touch) quietUntil = performance.now() + QUIET_MS;
    open(result, x, y);
  };
  const begin = (touch: boolean, x: number, y: number, candidate: C, target: EventTarget | null, init: MouseEventInit) => {
    clear();
    const mine: Press<R> = { touch, x, y, target, init, held: false, timers: [] };
    press = mine;
    mine.timers.push(window.setTimeout(() => { mine.held = true; if (press === mine) settle(); }, holdMs));
    mine.timers.push(window.setTimeout(() => { if (press === mine) handOver(); }, holdMs + waitMs));
    resolve(candidate).catch(() => null).then((result) => {
      mine.result = result;
      if (press === mine) settle();
    });
  };
  const far = (x: number, y: number, slop: number) => Boolean(press) && Math.hypot(x - press!.x, y - press!.y) > slop;

  // A release that never came (the button let go outside the window) must not cost the next one.
  const onAnyPress = () => { if (!replaying) swallowRelease = null; };
  const onMouseDown = (e: MouseEvent) => {
    if (replaying) return;
    if (press) handOver();
    if (e.button !== 0 || e.ctrlKey || e.shiftKey || e.altKey || e.metaKey || e.detail > 1) return;
    const candidate = candidateAt(e.clientX, e.clientY);
    if (!candidate) return;
    // Held back: see above. xterm would have stopped the browser's own handling too.
    e.preventDefault();
    e.stopPropagation();
    begin(false, e.clientX, e.clientY, candidate, e.target, {
      bubbles: true, cancelable: true, composed: true, view: window, detail: 1, button: 0, buttons: 1,
      clientX: e.clientX, clientY: e.clientY, screenX: e.screenX, screenY: e.screenY,
    });
  };
  const onMouseMove = (e: MouseEvent) => {
    if (press && !press.touch && far(e.clientX, e.clientY, options.slopPx ?? SLOP_MOUSE)) handOver();
  };
  const onMouseUp = (e: MouseEvent) => {
    if (press && !press.touch) { handOver(); return; }
    if (swallowRelease !== 'mouse') return;
    swallowRelease = null;
    e.stopPropagation();
    e.preventDefault();
  };
  // The wheel, a key or another window in between: the press is dropped, not replayed late.
  const onLeave = () => clear();

  const onTouchStart = (e: TouchEvent) => {
    if (press) clear();
    if (e.touches.length !== 1) return;
    const { clientX, clientY } = e.touches[0];
    const candidate = candidateAt(clientX, clientY);
    if (candidate) begin(true, clientX, clientY, candidate, null, {});
  };
  const onTouchMove = (e: TouchEvent) => {
    if (!press?.touch) return;
    if (e.touches.length !== 1 || far(e.touches[0].clientX, e.touches[0].clientY, options.slopPx ?? SLOP_TOUCH)) clear();
  };
  const onTouchEnd = (e: TouchEvent) => {
    if (press?.touch) clear();
    if (swallowRelease !== 'touch') return;
    swallowRelease = null;
    // The lifted finger would otherwise become a click on whatever the menu put under it.
    if (e.cancelable) e.preventDefault();
  };
  // A long press is also how a phone asks for its own menu, which the terminal
  // answers by pasting: whichever comes first wins, and the other is dropped.
  const onContextMenu = (e: MouseEvent) => {
    if (press?.touch) { clear(); return; }
    if (performance.now() >= quietUntil) return;
    e.preventDefault();
    e.stopImmediatePropagation();
  };

  window.addEventListener('mousedown', onAnyPress, true);
  window.addEventListener('touchstart', onAnyPress, { capture: true, passive: true });
  el.addEventListener('mousedown', onMouseDown, true);
  window.addEventListener('mousemove', onMouseMove, true);
  window.addEventListener('mouseup', onMouseUp, true);
  window.addEventListener('blur', onLeave);
  el.addEventListener('wheel', onLeave, { capture: true, passive: true });
  el.addEventListener('keydown', onLeave, true);
  el.addEventListener('touchstart', onTouchStart, { capture: true, passive: true });
  el.addEventListener('touchmove', onTouchMove, { capture: true, passive: true });
  el.addEventListener('touchend', onTouchEnd, { capture: true, passive: false });
  el.addEventListener('touchcancel', onTouchEnd, { capture: true, passive: false });
  el.addEventListener('contextmenu', onContextMenu, true);
  return () => {
    clear();
    window.removeEventListener('mousedown', onAnyPress, true);
    window.removeEventListener('touchstart', onAnyPress, true);
    el.removeEventListener('mousedown', onMouseDown, true);
    window.removeEventListener('mousemove', onMouseMove, true);
    window.removeEventListener('mouseup', onMouseUp, true);
    window.removeEventListener('blur', onLeave);
    el.removeEventListener('wheel', onLeave, true);
    el.removeEventListener('keydown', onLeave, true);
    el.removeEventListener('touchstart', onTouchStart, true);
    el.removeEventListener('touchmove', onTouchMove, true);
    el.removeEventListener('touchend', onTouchEnd, true);
    el.removeEventListener('touchcancel', onTouchEnd, true);
    el.removeEventListener('contextmenu', onContextMenu, true);
  };
}
