/**
 * Keeps a refresh from rebuilding the whole map. The graph library ties each
 * drawn object to its data object (by identity), so handing it fresh copies
 * on every poll threw away and remade every sphere, ring and name texture,
 * every link, arrow and particle, several times a minute. Here a node or link
 * whose look has not changed keeps its data object — and so its 3D object;
 * only what changed is made again (the library disposes what it replaces).
 */
import type { GraphNode } from '../graphModel';
import type { GraphStyle } from './graphSettings';

/** Everything a node's drawn object depends on, for its style (the rich one also sizes by heat and lists the AIs). */
export function visualKey(n: GraphNode, degree: number, style: GraphStyle): string {
  const common = [n.kind || 'session', n.device || '', n.name, n.alive, n.lead, n.busy, n.color, n.sub || '', n.status || '', n.task || '', n.claude];
  const own = style === 'rich' ? [n.model || '', (n.agents || []).join('+'), (n.info || []).join('/'), n.heat || 0] : [degree];
  return JSON.stringify([...common, ...own]);
}

/** What the layout keeps on a node object: where it is, how it moves, and its pins. */
const LAYOUT = new Set(['x', 'y', 'z', 'vx', 'vy', 'vz', 'fx', 'fy', 'fz', 'index']);

/** A placed node: its position and pins are the layout's, the rest is the model's. */
export type Placed = GraphNode & { x?: number; y?: number; z?: number; fx?: number; fy?: number; fz?: number; __key?: string };

/**
 * The nodes for the next drawing: an unchanged one is the same object as
 * before with the model's fields copied on; a changed or new one a new object
 * that starts where the old one stood. `kept`/`made` count each, for checks.
 */
export function reuseNodes(before: Map<string, Placed>, next: GraphNode[], keyOf: (n: GraphNode) => string): { nodes: Placed[]; kept: number; made: number } {
  let kept = 0;
  let made = 0;
  const nodes = next.map((n) => {
    const key = keyOf(n);
    const old = before.get(n.id);
    if (old && old.__key === key) {
      kept += 1;
      // A field the model no longer gives goes; the pins are set again by whoever places the tiers.
      const rec = old as unknown as Record<string, unknown>;
      for (const k of Object.keys(rec)) if (!(k in n) && !LAYOUT.has(k) && !k.startsWith('__')) delete rec[k];
      old.fx = undefined; old.fy = undefined; old.fz = undefined;
      return Object.assign(old, n);
    }
    made += 1;
    const fresh: Placed = { ...n, x: old?.x, y: old?.y, z: old?.z, __key: key };
    return fresh;
  });
  return { nodes, kept, made };
}

/** A link as drawn: the layout turns its ends into the node objects, so they are set back to ids each time. */
export interface LinkLike { id: string; source: unknown; target: unknown }

/** The links for the next drawing, keeping the object of each one whose look (`keyOf`) is unchanged. */
export function reuseLinks<L extends LinkLike>(before: Map<string, L & { __key?: string }>, next: L[], keyOf: (l: L) => string): { links: L[]; kept: number; made: number } {
  let kept = 0;
  let made = 0;
  const links = next.map((l) => {
    const key = keyOf(l);
    const old = before.get(l.id);
    if (old && old.__key === key) {
      kept += 1;
      return Object.assign(old, l);
    }
    made += 1;
    return Object.assign(l, { __key: key });
  });
  return { links, kept, made };
}

/**
 * The links with both ends set to the node objects themselves, never ids. The
 * running layout reads `link.source.vx` on every tick, and the library only
 * turns ids into nodes on its next update, a frame later: a kept link whose
 * ends had just been set back to ids broke the tick in between ("Cannot create
 * property 'vx' on string"). A link to a node that is not drawn is left out.
 */
export function tieEnds<L extends LinkLike, N extends { id: string }>(links: L[], nodes: N[]): L[] {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const idOf = (end: unknown) => (typeof end === 'object' && end !== null ? (end as { id: string }).id : String(end));
  const out: L[] = [];
  for (const l of links) {
    const a = byId.get(idOf(l.source));
    const b = byId.get(idOf(l.target));
    if (!a || !b) continue;
    l.source = a;
    l.target = b;
    out.push(l);
  }
  return out;
}
