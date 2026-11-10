import type { GraphNode } from '../graphModel';
import { Placed, reuseLinks, reuseNodes, visualKey } from './nodeReuse';

const node = (id: string, extra: Partial<GraphNode> = {}): GraphNode => ({
  id, name: id, claude: '', group: 'local', busy: false, lead: false, orchestrator: false, alive: true, color: '#5eead4', kind: 'session', ...extra,
});

describe('visualKey', () => {
  test('the simple style ignores heat but not the links a node has', () => {
    const n = node('a', { heat: 1 });
    expect(visualKey({ ...n, heat: 9 }, 2, 'simple')).toBe(visualKey(n, 2, 'simple'));
    expect(visualKey(n, 3, 'simple')).not.toBe(visualKey(n, 2, 'simple'));
  });
  test('the rich style is sized by heat, so heat counts there', () => {
    const n = node('a', { heat: 1 });
    expect(visualKey({ ...n, heat: 2 }, 0, 'rich')).not.toBe(visualKey(n, 0, 'rich'));
  });
  test('what is drawn on a node changes the key', () => {
    const n = node('a');
    for (const patch of [{ busy: true }, { alive: false }, { name: 'b' }, { task: 't' }, { status: 'offline' }, { color: '#000' }]) {
      expect(visualKey({ ...n, ...patch }, 0, 'simple')).not.toBe(visualKey(n, 0, 'simple'));
    }
  });
});

describe('reuseNodes', () => {
  const key = (n: GraphNode) => visualKey(n, 0, 'simple');
  const first = () => reuseNodes(new Map(), [node('a'), node('b')], key).nodes;

  test('an unchanged node keeps its object, its place and its drawn object', () => {
    const before = first();
    const a = before[0] as Placed & { __threeObj?: unknown; vx?: number };
    Object.assign(a, { x: 1, y: 2, z: 3, vx: 0.5, __threeObj: 'mesh' });
    const out = reuseNodes(new Map(before.map((n) => [n.id, n])), [node('a', { heat: 4 }), node('b')], key);
    expect(out.kept).toBe(2);
    expect(out.made).toBe(0);
    expect(out.nodes[0]).toBe(a);
    expect(a).toMatchObject({ x: 1, y: 2, z: 3, vx: 0.5, __threeObj: 'mesh', heat: 4 });
  });

  test('a changed node is a new object that starts where the old one stood', () => {
    const before = first();
    Object.assign(before[0], { x: 7, y: 8, z: 9 });
    const out = reuseNodes(new Map(before.map((n) => [n.id, n])), [node('a', { busy: true }), node('b'), node('c')], key);
    expect(out.kept).toBe(1);
    expect(out.made).toBe(2);
    expect(out.nodes[0]).not.toBe(before[0]);
    expect(out.nodes[0]).toMatchObject({ x: 7, y: 8, z: 9, busy: true });
    expect(out.nodes[2].x).toBeUndefined();
  });

  test('a kept node drops the fields the model no longer gives and its pins', () => {
    const before = reuseNodes(new Map(), [node('a', { heat: 3 })], key).nodes;
    before[0].fy = 14;
    const out = reuseNodes(new Map(before.map((n) => [n.id, n])), [node('a')], key);
    expect(out.nodes[0]).toBe(before[0]);
    expect('heat' in out.nodes[0]).toBe(false);
    expect(out.nodes[0].fy).toBeUndefined();
  });
});

describe('reuseLinks', () => {
  type L = { id: string; source: unknown; target: unknown; live: boolean };
  const key = (l: L) => `${l.live}`;
  test('an unchanged link keeps its object and gets its ends back as ids', () => {
    const one = reuseLinks<L>(new Map(), [{ id: 'a>b', source: 'a', target: 'b', live: false }], key).links;
    one[0].source = { id: 'a' };
    const out = reuseLinks<L>(new Map(one.map((l) => [l.id, l])), [{ id: 'a>b', source: 'a', target: 'b', live: false }], key);
    expect(out.links[0]).toBe(one[0]);
    expect(out.links[0].source).toBe('a');
    expect(out.kept).toBe(1);
  });
  test('a changed link is the new object', () => {
    const one = reuseLinks<L>(new Map(), [{ id: 'a>b', source: 'a', target: 'b', live: false }], key).links;
    const fresh = { id: 'a>b', source: 'a', target: 'b', live: true };
    const out = reuseLinks<L>(new Map(one.map((l) => [l.id, l])), [fresh], key);
    expect(out.links[0]).toBe(fresh);
    expect(out.made).toBe(1);
  });
});
