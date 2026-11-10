import { approach, fadeDistance, labelOpacity, labelTargets } from './graphLabels';

it('shows a name near, fades it with distance and hides it far off', () => {
  const far = fadeDistance(0.5);
  expect(labelOpacity(10, 0.5)).toBe(1);
  expect(labelOpacity(far * 0.6, 0.5)).toBe(1);
  expect(labelOpacity(far * 0.8, 0.5)).toBeCloseTo(0.5);
  expect(labelOpacity(far, 0.5)).toBe(0);
  expect(labelOpacity(far * 3, 0.5)).toBe(0);
});

it('shows names from further off as the threshold rises', () => {
  expect(fadeDistance(1)).toBeGreaterThan(fadeDistance(0.5));
  expect(labelOpacity(500, 0.2)).toBe(0);
  expect(labelOpacity(500, 1)).toBe(1);
  // Out-of-range thresholds are held to 0–1.
  expect(fadeDistance(7)).toBe(fadeDistance(1));
});

it('keeps forced names whatever the distance, and only the nearest of the rest under the cap', () => {
  const dist = [5000, 40, 10, 30, 20];
  const forced = [true, false, false, false, false];
  const out = labelTargets(dist, forced, 5, 0.5, 3);
  // The forced far one, then the two nearest of the others (10 and 20).
  expect(out).toEqual([1, 0, 1, 0, 1]);
  // Reuses what it is given.
  const reuse: number[] = [9, 9, 9, 9, 9, 9, 9];
  expect(labelTargets(dist, [false, false, false, false, false], 5, 0.5, 10, reuse)).toBe(reuse);
  expect(reuse).toEqual([0, 1, 1, 1, 1]);
});

it('eases towards a target and lands on it', () => {
  let v = 0;
  for (let i = 0; i < 60; i += 1) v = approach(v, 1, 1 / 60);
  expect(v).toBe(1);
  expect(approach(0, 1, 0.05)).toBeGreaterThan(0);
  expect(approach(0, 1, 0.05)).toBeLessThan(1);
});
