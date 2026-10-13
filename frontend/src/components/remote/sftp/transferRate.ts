/**
 * How fast a transfer is going and how long it has left.
 *
 * Progress events arrive in bursts (a piece finishes, the next one starts), so
 * the raw rate between two events jumps around. The rate here is smoothed
 * over the last few seconds, and the shown values change at most twice a
 * second, so the remaining time reads as a steady countdown.
 */

/** Seconds over which the rate is averaged. */
const SMOOTHING_S = 4;
/** How often the shown rate may change. */
const SHOW_EVERY_MS = 500;
/** Too little seen to guess from. */
const WARMUP_MS = 1000;

export interface RateState {
  startedAt: number;
  at: number;
  loaded: number;
  /** Bytes per second, smoothed. */
  rate: number;
  shownAt: number;
}

export interface RateView {
  /** Bytes per second, or null while there is not enough to tell. */
  rate: number | null;
  /** Seconds left, or null while unknown. */
  eta: number | null;
}

export function startRate(now: number): RateState {
  return { startedAt: now, at: now, loaded: 0, rate: 0, shownAt: 0 };
}

/**
 * Folds in a new progress reading. Returns the next state, and a view when it
 * is time to show one (null otherwise).
 */
export function updateRate(state: RateState, loaded: number, total: number, now: number):
  [RateState, RateView | null] {
  // A retried piece starts again from an earlier point: measure afresh.
  if (loaded < state.loaded) return [{ ...state, at: now, loaded }, null];
  const dt = (now - state.at) / 1000;
  if (dt <= 0) return [state, null];
  const instant = (loaded - state.loaded) / dt;
  const weight = 1 - Math.exp(-dt / SMOOTHING_S);
  const rate = state.rate === 0 ? instant : state.rate + weight * (instant - state.rate);
  const next = { ...state, at: now, loaded, rate };
  if (now - state.shownAt < SHOW_EVERY_MS) return [next, null];
  next.shownAt = now;
  if (now - state.startedAt < WARMUP_MS || rate <= 0) return [next, { rate: null, eta: null }];
  const left = Math.max(0, total - loaded);
  return [next, { rate, eta: total > 0 ? Math.ceil(left / rate) : null }];
}

/** "2시간 5분", "3분 20초", "45초" — the two largest units are enough. */
export function formatDuration(seconds: number, units: { h: string; m: string; s: string }): string {
  const s = Math.max(0, Math.round(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const rest = s % 60;
  if (h > 0) return m > 0 ? `${h}${units.h} ${m}${units.m}` : `${h}${units.h}`;
  if (m > 0) return rest > 0 ? `${m}${units.m} ${rest}${units.s}` : `${m}${units.m}`;
  return `${rest}${units.s}`;
}
