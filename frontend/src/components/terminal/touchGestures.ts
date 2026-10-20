import type { Terminal } from 'xterm';

/**
 * Scrolls a terminal's history by a step: xterm's own scrollback for a plain
 * shell, and for tmux (the alternate screen) the same wheel event a mouse
 * would send over its text, so tmux scrolls its history.
 */
export function scrollTerminalHistory(term: Terminal, direction: number, lines = 3): void {
  if (term.buffer.active.type === 'normal') {
    term.scrollLines(direction * lines);
    return;
  }
  const screen = term.element?.querySelector('.xterm-screen');
  if (!screen) return;
  const rect = screen.getBoundingClientRect();
  screen.dispatchEvent(new WheelEvent('wheel', {
    bubbles: true, cancelable: true, deltaY: direction * 120,
    clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2,
  }));
}

/** How far a finger moves for one step of history. */
const STEP_PX = 36;
/** Movement under this is a tap (which focuses, and opens the keyboard). */
const SLOP_PX = 10;

/**
 * Touch on a terminal: one finger dragged up or down scrolls its history,
 * as on any phone, and two fingers pinch the text larger or smaller. A tap
 * is left alone so it still focuses the terminal.
 */
export function attachTouchGestures(
  el: HTMLElement,
  getTerm: () => Terminal | null,
  zoomTo: (size: number) => void,
): () => void {
  let mode: 'idle' | 'scroll' | 'pinch' = 'idle';
  let startX = 0, startY = 0, lastY = 0, carried = 0, startDistance = 0, startSize = 0;
  const distance = (t: TouchList) => Math.hypot(t[0].clientX - t[1].clientX, t[0].clientY - t[1].clientY);

  const start = (e: TouchEvent) => {
    if (e.touches.length === 2) {
      mode = 'pinch';
      startDistance = distance(e.touches) || 1;
      startSize = getTerm()?.options.fontSize || 14;
    } else if (e.touches.length === 1) {
      mode = 'idle';
      startX = e.touches[0].clientX;
      startY = lastY = e.touches[0].clientY;
      carried = 0;
    }
  };
  const move = (e: TouchEvent) => {
    const term = getTerm();
    if (!term) return;
    if (mode === 'pinch' && e.touches.length === 2) {
      e.preventDefault();
      e.stopPropagation();
      const size = Math.min(28, Math.max(8, Math.round(startSize * (distance(e.touches) / startDistance))));
      if (size !== term.options.fontSize) zoomTo(size);
      return;
    }
    if (e.touches.length !== 1) return;
    const { clientX: x, clientY: y } = e.touches[0];
    if (mode === 'idle' && Math.abs(y - startY) > SLOP_PX && Math.abs(y - startY) > Math.abs(x - startX)) mode = 'scroll';
    if (mode !== 'scroll') return;
    e.preventDefault();
    e.stopPropagation();
    // A finger moving down shows what came before, as on any phone.
    carried += lastY - y;
    lastY = y;
    while (Math.abs(carried) >= STEP_PX) {
      const direction = Math.sign(carried);
      scrollTerminalHistory(term, direction, 1);
      carried -= direction * STEP_PX;
    }
  };
  const end = (e: TouchEvent) => { if (e.touches.length === 0) mode = 'idle'; };

  el.addEventListener('touchstart', start, { passive: true, capture: true });
  el.addEventListener('touchmove', move, { passive: false, capture: true });
  el.addEventListener('touchend', end, { capture: true });
  el.addEventListener('touchcancel', end, { capture: true });
  return () => {
    el.removeEventListener('touchstart', start, { capture: true } as EventListenerOptions);
    el.removeEventListener('touchmove', move, { capture: true } as EventListenerOptions);
    el.removeEventListener('touchend', end, { capture: true } as EventListenerOptions);
    el.removeEventListener('touchcancel', end, { capture: true } as EventListenerOptions);
  };
}
