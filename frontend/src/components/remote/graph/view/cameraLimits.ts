/**
 * How near the camera may come. The library lets the wheel dolly in to 0.1
 * units of the point looked at — which, after a click, is a node's centre —
 * so the camera ended up inside the node, its ring or its glow, with nothing
 * (or one face) to draw. The nearest it may come now is a few times the
 * biggest node's reach (ring and glow included), and a flight to a node stops
 * well outside the node it goes to.
 */
import type { GraphNode } from '../graphModel';
import type { GraphStyle } from './graphSettings';

/** How far a node of body `radius` reaches from its centre: a rich session's glow (1.6 r, the lead's 1.9 r), the simple lead's ring (1.7 r). */
export function nodeReach(n: GraphNode, style: GraphStyle, radius: number): number {
  if (style === 'rich') return radius * (n.kind === 'session' || !n.kind ? (n.lead ? 1.9 : 1.6) : 1);
  return radius * (n.lead ? 1.7 : 1);
}

/** The nearest the orbit may come to the point it turns round: outside any node standing there, with room to see it. */
export function minDistance(reach: number): number {
  return Math.max(12, reach * 2.5);
}

/** How far from a node a flight to it stops: the usual 120, more for a node big enough to fill that. */
export function flyDistance(reach: number): number {
  return Math.max(120, reach * 6);
}

/** The biggest reach among the nodes drawn, `reachOf` giving each one's. */
export function biggestReach(nodes: GraphNode[], reachOf: (n: GraphNode) => number): number {
  let most = 0;
  for (const n of nodes) most = Math.max(most, reachOf(n));
  return most;
}
