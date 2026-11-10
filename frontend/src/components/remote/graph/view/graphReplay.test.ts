import type { MapEvent } from '../graphModel';
import { dueSteps, MAX_GAP_MS, planReplay, replayClock } from './graphReplay';

const ev = (id: number, at: number): MapEvent => ({ id, at, from: 'a', to: 'b', from_name: 'A', to_name: 'B', text: `m${id}` });
const NOW = 1_000_000;

it('plays the last day in the order it happened, faster, with long quiet spells cut short', () => {
  const events = [ev(3, NOW - 100), ev(1, NOW - 4000), ev(2, NOW - 3990), ev(9, NOW - 90_000), ev(2, NOW - 3990)];
  const plan = planReplay(events, 10, NOW);
  // Older than a day is left out, the duplicate once.
  expect(plan.map((p) => p.event.id)).toEqual([1, 2, 3]);
  expect(plan[0].offset).toBe(0);
  // 10 s of record at ×10: 1 s.
  expect(plan[1].offset).toBe(1000);
  // 3890 s of quiet: capped.
  expect(plan[2].offset).toBe(1000 + MAX_GAP_MS);
});

it('scales with the speed and keeps ties in id order', () => {
  const events = [ev(5, NOW - 50), ev(4, NOW - 50), ev(6, NOW - 49)];
  expect(planReplay(events, 1, NOW).map((p) => [p.event.id, p.offset])).toEqual([[4, 0], [5, 0], [6, 1000]]);
  expect(planReplay(events, 4, NOW)[2].offset).toBe(250);
  expect(planReplay([], 10, NOW)).toEqual([]);
});

it('hands out the steps as they fall due, each once', () => {
  const plan = planReplay([ev(1, NOW - 30), ev(2, NOW - 20), ev(3, NOW - 10)], 10, NOW);
  const out: MapEvent[] = [];
  let i = dueSteps(plan, 0, 0, out);
  expect(out.map((e) => e.id)).toEqual([1]);
  i = dueSteps(plan, i, 999, out);
  expect(i).toBe(1);
  i = dueSteps(plan, i, 5000, out);
  expect(out.map((e) => e.id)).toEqual([1, 2, 3]);
  expect(i).toBe(3);
});

it('tells the time of the record reached, between two steps', () => {
  const plan = planReplay([ev(1, NOW - 30), ev(2, NOW - 20)], 10, NOW);
  expect(replayClock(plan, 0)).toBe(NOW - 30);
  expect(replayClock(plan, 500)).toBe(NOW - 25);
  expect(replayClock(plan, 9999)).toBe(NOW - 20);
  expect(replayClock([], 10)).toBe(0);
});
