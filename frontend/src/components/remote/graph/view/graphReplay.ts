/**
 * The time-lapse: the last day's messages between sessions played back in
 * order, faster than they happened, with long quiet spells shortened so
 * the replay keeps moving.
 */
import type { MapEvent } from '../graphModel';

export const REPLAY_WINDOW = 24 * 60 * 60;
/** The longest wait between two bursts, however long the quiet was. */
export const MAX_GAP_MS = 1500;

export interface ReplayStep { event: MapEvent; /** ms from the start of the replay. */ offset: number }

/**
 * When each event fires: in the order they happened (ties by id), `speed`
 * seconds of the record per real second, gaps capped at `maxGap` ms. Events
 * older than `window` seconds before `now` are left out.
 */
export function planReplay(events: MapEvent[], speed: number, now: number, window = REPLAY_WINDOW, maxGap = MAX_GAP_MS): ReplayStep[] {
  const seen = new Set<number>();
  const list = events
    .filter((e) => e.at >= now - window && !seen.has(e.id) && seen.add(e.id))
    .sort((a, b) => a.at - b.at || a.id - b.id);
  const out: ReplayStep[] = [];
  let offset = 0;
  for (let i = 0; i < list.length; i += 1) {
    if (i > 0) offset += Math.min(maxGap, ((list[i].at - list[i - 1].at) * 1000) / Math.max(1, speed));
    out.push({ event: list[i], offset });
  }
  return out;
}

/** The steps due by `elapsed` ms that have not fired yet (from index `from`); returns the next index. */
export function dueSteps(plan: ReplayStep[], from: number, elapsed: number, out: MapEvent[]): number {
  let i = from;
  while (i < plan.length && plan[i].offset <= elapsed) { out.push(plan[i].event); i += 1; }
  return i;
}

/** The replay's clock at `elapsed` ms: the time of the record it has reached, between the step before and after. */
export function replayClock(plan: ReplayStep[], elapsed: number): number {
  if (!plan.length) return 0;
  let i = 0;
  while (i + 1 < plan.length && plan[i + 1].offset <= elapsed) i += 1;
  const a = plan[i];
  const b = plan[i + 1];
  if (!b || b.offset === a.offset) return a.event.at;
  const k = Math.min(1, Math.max(0, (elapsed - a.offset) / (b.offset - a.offset)));
  return a.event.at + (b.event.at - a.event.at) * k;
}
