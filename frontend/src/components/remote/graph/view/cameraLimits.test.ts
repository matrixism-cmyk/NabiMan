import type { GraphNode } from '../graphModel';
import { biggestReach, flyDistance, minDistance, nodeReach } from './cameraLimits';

const node = (extra: Partial<GraphNode> = {}): GraphNode => ({
  id: 'a', name: 'a', claude: '', group: 'local', busy: false, lead: false, orchestrator: false, alive: true, color: '#fff', kind: 'session', ...extra,
});

test('a node reaches out to its glow or ring', () => {
  const lead = node({ lead: true });
  expect(nodeReach(lead, 'rich', 15)).toBeCloseTo(28.5);
  expect(nodeReach(node(), 'rich', 10)).toBeCloseTo(16);
  expect(nodeReach(lead, 'simple', 10)).toBeCloseTo(17);
  expect(nodeReach(node({ kind: 'server' }), 'rich', 11)).toBe(11);
  expect(nodeReach(node(), 'simple', 4)).toBe(4);
});

test('the orbit stops outside the biggest node, never closer than 12', () => {
  expect(minDistance(28.5)).toBeGreaterThan(28.5 * 2);
  expect(minDistance(0)).toBe(12);
  expect(minDistance(1)).toBe(12);
});

test('a flight stops 120 out, further for a big node, and always outside the orbit limit', () => {
  expect(flyDistance(5)).toBe(120);
  expect(flyDistance(40)).toBe(240);
  for (const r of [0, 5, 28.5, 40, 100]) expect(flyDistance(r)).toBeGreaterThan(minDistance(r));
});

test('the biggest reach is the largest of the nodes', () => {
  const nodes = [node({ id: 'a' }), node({ id: 'b', lead: true }), node({ id: 'c', kind: 'server' })];
  const size: Record<string, number> = { a: 6, b: 15, c: 11 };
  expect(biggestReach(nodes, (n) => nodeReach(n, 'rich', size[n.id]))).toBeCloseTo(28.5);
  expect(biggestReach([], () => 1)).toBe(0);
});
