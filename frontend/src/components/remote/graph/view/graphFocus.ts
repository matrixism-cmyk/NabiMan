/**
 * What to show and what to bring forward, worked out on the plain graph so
 * any renderer can use it: filters, the local graph round one node, search,
 * and the focus (a node, its neighbours and their links) that hover and
 * selection give, as in Obsidian's graph view.
 */
import type { Graph, GraphNode } from '../graphModel';

/** A link as the focus needs it; the renderer's own links fit too. */
export interface Tie { id: string; from: string; to: string }

export interface Adjacency {
  /** Each node's neighbours and the links that reach them. */
  of: Map<string, { nodes: Set<string>; links: Set<string> }>;
  /** Each link's two ends. */
  ends: Map<string, [string, string]>;
}

export function adjacency(ties: Tie[]): Adjacency {
  const of: Adjacency['of'] = new Map();
  const ends: Adjacency['ends'] = new Map();
  const at = (id: string) => {
    let a = of.get(id);
    if (!a) { a = { nodes: new Set(), links: new Set() }; of.set(id, a); }
    return a;
  };
  for (const t of ties) {
    if (t.from === t.to) continue;
    const a = at(t.from);
    const b = at(t.to);
    a.nodes.add(t.to); a.links.add(t.id);
    b.nodes.add(t.from); b.links.add(t.id);
    ends.set(t.id, [t.from, t.to]);
  }
  return { of, ends };
}

/** How many links each node has. */
export function degrees(ties: Tie[]): Map<string, number> {
  const out = new Map<string, number>();
  adjacency(ties).of.forEach((a, id) => out.set(id, a.links.size));
  return out;
}

export interface Focus {
  /** The nodes drawn in full; the others dim. */
  nodes: Set<string>;
  /** The links drawn in colour; the others fade. */
  links: Set<string>;
}

/** The given nodes, with their neighbours and the links to them when `neighbours` is set (else only the links among them). */
export function focusOf(adj: Adjacency, ids: string[], neighbours = true): Focus {
  const nodes = new Set(ids);
  const links = new Set<string>();
  for (const id of ids) {
    const a = adj.of.get(id);
    if (!a) continue;
    a.links.forEach((l) => {
      const [x, y] = adj.ends.get(l) || ['', ''];
      if (neighbours || (nodes.has(x) && nodes.has(y))) links.add(l);
    });
    if (neighbours) a.nodes.forEach((n) => nodes.add(n));
  }
  return { nodes, links };
}

export interface FocusInput { hover: string | null; selected: string | null; matches: string[] }

/** Hover wins over the selection, the selection over a search; with none of them nothing is dimmed. */
export function pickFocus(adj: Adjacency, { hover, selected, matches }: FocusInput): Focus | null {
  if (hover && adj.of.has(hover)) return focusOf(adj, [hover]);
  if (hover) return { nodes: new Set([hover]), links: new Set() };
  if (selected) return adj.of.has(selected) ? focusOf(adj, [selected]) : { nodes: new Set([selected]), links: new Set() };
  if (matches.length) return focusOf(adj, matches, false);
  return null;
}

/** The names always shown, whatever the distance: the hovered node and its neighbours, the selected one, the search's matches. */
export function forcedLabels(adj: Adjacency, { hover, selected, matches }: FocusInput): Set<string> {
  const out = new Set<string>(matches);
  if (selected) out.add(selected);
  if (hover) {
    out.add(hover);
    adj.of.get(hover)?.nodes.forEach((n) => out.add(n));
  }
  return out;
}

const tiesOf = (g: Graph): Tie[] => g.edges.map((e) => ({ id: e.id, from: e.from, to: e.to }));

/** Only the nodes within `depth` links of `center` (and the links among them): the local graph. */
export function localSubset(g: Graph, center: string, depth: number): Graph {
  if (!g.nodes.some((n) => n.id === center)) return g;
  const adj = adjacency(tiesOf(g));
  const keep = new Set([center]);
  let front = [center];
  for (let d = 0; d < depth && front.length; d += 1) {
    const next: string[] = [];
    for (const id of front) {
      adj.of.get(id)?.nodes.forEach((n) => { if (!keep.has(n)) { keep.add(n); next.push(n); } });
    }
    front = next;
  }
  return { nodes: g.nodes.filter((n) => keep.has(n.id)), edges: g.edges.filter((e) => keep.has(e.from) && keep.has(e.to)) };
}

export interface Filters { showDevices: boolean; showEnded: boolean; showServers: boolean; showOrphans: boolean }

/** The graph less what the filters hide; an orphan is a node left with no link (the lead always stays). */
export function filterGraph(g: Graph, f: Filters): Graph {
  const kept = g.nodes.filter((n) => {
    if (!f.showDevices && n.kind === 'device') return false;
    if (!f.showServers && n.kind === 'server') return false;
    if (!f.showEnded && (n.kind || 'session') === 'session' && !n.alive) return false;
    return true;
  });
  const ids = new Set(kept.map((n) => n.id));
  const edges = g.edges.filter((e) => ids.has(e.from) && ids.has(e.to));
  if (f.showOrphans) return { nodes: kept, edges };
  const tied = new Set(edges.flatMap((e) => [e.from, e.to]));
  return { nodes: kept.filter((n) => n.lead || tied.has(n.id)), edges };
}

/** The nodes a search finds: by name, Claude, address, server or model, case aside. */
export function searchMatches(nodes: GraphNode[], query: string): string[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  return nodes
    .filter((n) => [n.name, n.claude, n.sub, n.group, n.model].some((v) => !!v && v.toLowerCase().includes(q)))
    .map((n) => n.id);
}
