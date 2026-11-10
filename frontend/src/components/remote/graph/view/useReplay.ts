/**
 * Plays a replay plan: every 50 ms the events now due go to `onBurst`, and
 * the clock shown moves on (a few times a second, to spare React).
 */
import { useEffect, useRef, useState } from 'react';
import type { MapEvent } from '../graphModel';
import { dueSteps, planReplay, replayClock, ReplayStep } from './graphReplay';

export interface ReplayState { playing: boolean; clock: number; done: number; total: number }
const IDLE: ReplayState = { playing: false, clock: 0, done: 0, total: 0 };

export function useReplay(onBurst: (list: MapEvent[]) => void) {
  const [state, setState] = useState<ReplayState>(IDLE);
  const plan = useRef<ReplayStep[]>([]);
  const at = useRef({ index: 0, origin: 0, timer: 0 as unknown as ReturnType<typeof setInterval> | 0, shown: 0 });
  const burst = useRef(onBurst);
  burst.current = onBurst;

  const stop = () => {
    if (at.current.timer) clearInterval(at.current.timer);
    at.current.timer = 0;
    setState((s) => (s.playing ? { ...s, playing: false } : s));
  };

  const step = () => {
    const now = performance.now();
    const elapsed = now - at.current.origin;
    const due: MapEvent[] = [];
    at.current.index = dueSteps(plan.current, at.current.index, elapsed, due);
    if (due.length) burst.current(due);
    const finished = at.current.index >= plan.current.length;
    if (finished || now - at.current.shown > 200) {
      at.current.shown = now;
      setState({ playing: !finished, clock: replayClock(plan.current, elapsed), done: at.current.index, total: plan.current.length });
    }
    if (finished) stop();
  };

  /** Starts playing `events` (the last day of them); false when there is nothing to play. */
  const start = (events: MapEvent[], speed: number, nowSec = Date.now() / 1000) => {
    stop();
    plan.current = planReplay(events, speed, nowSec);
    if (!plan.current.length) return false;
    at.current.index = 0;
    at.current.origin = performance.now();
    at.current.shown = 0;
    setState({ playing: true, clock: plan.current[0].event.at, done: 0, total: plan.current.length });
    at.current.timer = setInterval(step, 50);
    return true;
  };

  /** A new speed while playing: the rest of the plan is laid out again from where it is. */
  const setSpeed = (speed: number) => {
    if (!at.current.timer || !plan.current.length) return;
    const events = plan.current.map((p) => p.event);
    const index = at.current.index;
    plan.current = planReplay(events, speed, Date.now() / 1000, Infinity);
    // Resume just after the last burst played.
    const last = index > 0 ? plan.current[index - 1] : undefined;
    at.current.origin = performance.now() - (last ? last.offset : 0);
  };

  useEffect(() => () => { if (at.current.timer) clearInterval(at.current.timer); }, []);
  return { state, start, stop, setSpeed };
}
