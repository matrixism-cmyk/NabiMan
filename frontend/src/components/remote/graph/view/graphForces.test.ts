import { centerPull, centerStrength, chargeOf, distanceOf, strengthOf } from './graphForces';

const s = { center: 0.5, repel: 2, linkStrength: 3, linkDistance: 0.5 };

it('scales the tuned per-kind forces by the settings', () => {
  expect(chargeOf('server', s)).toBe(-520);
  expect(chargeOf(undefined, s)).toBe(-110);
  expect(distanceOf('host', s)).toBe(33);
  expect(distanceOf(undefined, s)).toBe(47.5);
  // A link never holds harder than 1.
  expect(strengthOf('host', s)).toBe(1);
  expect(strengthOf('order', s)).toBeCloseTo(0.45);
  expect(strengthOf('report', s)).toBe(0);
});

it('pulls to the middle, only across when the tiers hold the heights', () => {
  const nodes = [{ x: 100, y: 50, z: -20, vx: 0, vy: 0, vz: 0 }];
  const f = centerPull().strength(centerStrength(s));
  f.initialize(nodes);
  f(1);
  expect(nodes[0].vx).toBeLessThan(0);
  expect(nodes[0].vy).toBeLessThan(0);
  expect(nodes[0].vz).toBeGreaterThan(0);
  const flat = [{ x: 100, y: 50, z: 0, vx: 0, vy: 0, vz: 0 }];
  const g = centerPull().strength(0.1).vertical(false);
  g.initialize(flat);
  g(1);
  expect(flat[0].vy).toBe(0);
  expect(flat[0].vx).toBe(-10);
});
