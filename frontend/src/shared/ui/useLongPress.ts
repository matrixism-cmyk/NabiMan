import { useCallback, useEffect, useRef } from 'react';

/** How long a finger rests on something before it counts as a long press. */
const HOLD_MS = 500;
const SLOP_PX = 10;

export interface Press { x: number; y: number; el: HTMLElement }

/**
 * Long press, for touch screens, where there is no right-click: holding a
 * finger still on an item calls `onLong` with where it was (as a right-click
 * menu would open). One hook serves a whole list: `bind(item)` gives each
 * row its handlers. `wasLong()` tells the click that follows a long press to
 * do nothing; `wasTouch()` tells a click it came from a finger.
 */
export function useLongPress<T>(onLong: (at: Press, item: T) => void) {
  const state = useRef<{ timer?: number; x: number; y: number; touch: boolean; long: boolean; at: number }>(
    { x: 0, y: 0, touch: false, long: false, at: 0 });
  const handler = useRef(onLong);
  handler.current = onLong;
  const cancel = useCallback(() => { window.clearTimeout(state.current.timer); state.current.timer = undefined; }, []);
  useEffect(() => cancel, [cancel]);

  const bind = useCallback((item: T) => ({
    onPointerDown: (e: React.PointerEvent<HTMLElement>) => {
      cancel();
      const s = state.current;
      s.touch = e.pointerType === 'touch' || e.pointerType === 'pen';
      s.long = false;
      if (!s.touch) return;
      s.x = e.clientX;
      s.y = e.clientY;
      const el = e.currentTarget;
      s.timer = window.setTimeout(() => {
        s.timer = undefined;
        s.long = true;
        s.at = Date.now();
        navigator.vibrate?.(12);
        handler.current({ x: s.x, y: s.y, el }, item);
      }, HOLD_MS);
    },
    onPointerMove: (e: React.PointerEvent<HTMLElement>) => {
      const s = state.current;
      if (s.timer && Math.hypot(e.clientX - s.x, e.clientY - s.y) > SLOP_PX) cancel();
    },
    onPointerUp: cancel,
    onPointerCancel: cancel,
  }), [cancel]);

  return {
    bind,
    wasLong: () => state.current.long,
    wasTouch: () => state.current.touch,
    /** A browser's own long-press menu event, right after ours, is the same press. */
    justFired: () => state.current.long && Date.now() - state.current.at < 1500,
  };
}
