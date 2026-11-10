import type { Graph, GraphNode } from '../graphModel';
import { adjacency, degrees, filterGraph, focusOf, forcedLabels, localSubset, pickFocus, searchMatches } from './graphFocus';

const node = (id: string, extra: Partial<GraphNode> = {}): GraphNode => ({ id, name: id, claude: '', group: 'local', busy: false, lead: false, orchestrator: false, alive: true, color: '#fff', kind: 'session', ...extra });
const edge = (from: string, to: string, kind?: 'host' | 'net' | 'conn') => ({ id: `${from}>${to}`, from, to, live: false, strength: 0.5, color: '#fff', note: '', kind });

// lead → a → b, host:local holds lead and a, c is alone, d ended under host:local.
const graph: Graph = {
  nodes: [node('lead', { lead: true }), node('a', { claude: 'worker-1', name: 'Builder' }), node('b', { sub: '10.0.0.7' }), node('c'), node('d', { alive: false }),
    node('host:local', { kind: 'server', name: 'This machine' }), node('dev:fw', { kind: 'device', name: 'Firewall' })],
  edges: [edge('lead', 'a'), edge('a', 'b'), edge('host:local', 'lead', 'host'), edge('host:local', 'a', 'host'), edge('host:local', 'd', 'host'), edge('dev:fw', 'host:local', 'net')],
};
const ties = graph.edges.map((e) => ({ id: e.id, from: e.from, to: e.to }));

it('knows each node’s neighbours and how many links it has', () => {
  const adj = adjacency(ties);
  expect(Array.from(adj.of.get('a')!.nodes).sort()).toEqual(['b', 'host:local', 'lead']);
  expect(adj.ends.get('a>b')).toEqual(['a', 'b']);
  expect(adj.of.has('c')).toBe(false);
  const deg = degrees(ties);
  expect(deg.get('host:local')).toBe(4);
  expect(deg.get('b')).toBe(1);
});

it('focuses a node with its neighbours and their links, or only the links among several', () => {
  const adj = adjacency(ties);
  const f = focusOf(adj, ['b']);
  expect(Array.from(f.nodes).sort()).toEqual(['a', 'b']);
  expect(Array.from(f.links)).toEqual(['a>b']);
  const among = focusOf(adj, ['lead', 'a', 'b'], false);
  expect(Array.from(among.nodes).sort()).toEqual(['a', 'b', 'lead']);
  expect(Array.from(among.links).sort()).toEqual(['a>b', 'lead>a']);
});

it('lets hover win over the selection, and the selection over a search', () => {
  const adj = adjacency(ties);
  expect(pickFocus(adj, { hover: null, selected: null, matches: [] })).toBeNull();
  expect(Array.from(pickFocus(adj, { hover: 'b', selected: 'lead', matches: ['c'] })!.nodes).sort()).toEqual(['a', 'b']);
  expect(pickFocus(adj, { hover: null, selected: 'lead', matches: ['c'] })!.nodes.has('host:local')).toBe(true);
  expect(Array.from(pickFocus(adj, { hover: null, selected: null, matches: ['c'] })!.nodes)).toEqual(['c']);
  // A node with no links still focuses on itself.
  expect(Array.from(pickFocus(adj, { hover: 'c', selected: null, matches: [] })!.nodes)).toEqual(['c']);
});

it('always names the hovered node, its neighbours, the selected one and the matches', () => {
  const adj = adjacency(ties);
  expect(Array.from(forcedLabels(adj, { hover: 'b', selected: 'c', matches: ['d'] })).sort()).toEqual(['a', 'b', 'c', 'd']);
});

it('cuts the local graph to the nodes within the chosen depth', () => {
  const one = localSubset(graph, 'b', 1);
  expect(one.nodes.map((n) => n.id)).toEqual(['a', 'b']);
  expect(one.edges.map((e) => e.id)).toEqual(['a>b']);
  const two = localSubset(graph, 'b', 2);
  expect(two.nodes.map((n) => n.id).sort()).toEqual(['a', 'b', 'host:local', 'lead']);
  expect(two.edges.map((e) => e.id).sort()).toEqual(['a>b', 'host:local>a', 'host:local>lead', 'lead>a']);
  // A node that is no longer there leaves the graph whole.
  expect(localSubset(graph, 'gone', 1)).toBe(graph);
});

it('filters devices, servers, ended sessions and orphans', () => {
  const all = { showDevices: true, showEnded: true, showServers: true, showOrphans: true };
  expect(filterGraph(graph, all).nodes).toHaveLength(7);
  expect(filterGraph(graph, { ...all, showDevices: false }).nodes.some((n) => n.kind === 'device')).toBe(false);
  const noEnded = filterGraph(graph, { ...all, showEnded: false });
  expect(noEnded.nodes.map((n) => n.id)).not.toContain('d');
  expect(noEnded.edges.map((e) => e.id)).not.toContain('host:local>d');
  const noServers = filterGraph(graph, { ...all, showServers: false });
  expect(noServers.edges.every((e) => !e.from.startsWith('host:') && !e.to.startsWith('host:'))).toBe(true);
  // Without the servers, the ended session hangs on nothing: an orphan; the lead always stays.
  const lean = filterGraph(graph, { ...all, showServers: false, showOrphans: false });
  expect(lean.nodes.map((n) => n.id)).toEqual(['lead', 'a', 'b']);
});

it('finds nodes by name, Claude, address or server, case aside', () => {
  expect(searchMatches(graph.nodes, 'builder')).toEqual(['a']);
  expect(searchMatches(graph.nodes, 'WORKER')).toEqual(['a']);
  expect(searchMatches(graph.nodes, '10.0.0')).toEqual(['b']);
  expect(searchMatches(graph.nodes, '  ')).toEqual([]);
  expect(searchMatches(graph.nodes, 'fire')).toEqual(['dev:fw']);
});
