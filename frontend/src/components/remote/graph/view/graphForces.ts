/**
 * The layout's forces, as the settings scale them. The per-kind values are
 * the ones the map was tuned with: sessions stay by their machine, machines
 * keep apart, devices push only gently (a strong push from a fixed tier
 * pressed everything under it down).
 */
type Kind = string | undefined;
export interface ForceSettings { center: number; repel: number; linkStrength: number; linkDistance: number }

const CHARGE: Record<string, number> = { server: -260, device: -80, session: -55 };
const DISTANCE: Record<string, number> = { net: 60, host: 66, conn: 150, ghost: 150, report: 90, order: 95 };
const STRENGTH: Record<string, number> = { net: 0.7, host: 1, conn: 0.5, ghost: 0.25, report: 0, order: 0.15 };

export const chargeOf = (kind: Kind, s: ForceSettings) => (CHARGE[kind || 'session'] ?? CHARGE.session) * s.repel;
export const distanceOf = (kind: Kind, s: ForceSettings) => (DISTANCE[kind || 'order'] ?? DISTANCE.order) * s.linkDistance;
export const strengthOf = (kind: Kind, s: ForceSettings) => Math.min(1, (STRENGTH[kind || 'order'] ?? STRENGTH.order) * s.linkStrength);
/** How far the push reaches: further when the nodes push harder, so a wider spread is possible. */
export const chargeReach = (s: ForceSettings) => 130 * Math.max(1, Math.sqrt(s.repel));
/** The pull to the middle per tick, for the 0–1 setting. */
export const centerStrength = (s: ForceSettings) => s.center * 0.03;

interface Body { x?: number; y?: number; z?: number; vx?: number; vy?: number; vz?: number }

/**
 * A pull of every node towards the middle, as d3-force takes it. With
 * `vertical` off it pulls only across (x, z): the tiers keep their heights.
 */
export function centerPull() {
  let nodes: Body[] = [];
  let k = 0;
  let vertical = true;
  const force = (alpha: number) => {
    const a = k * alpha;
    if (!a) return;
    for (const n of nodes) {
      n.vx = (n.vx || 0) - (n.x || 0) * a;
      n.vz = (n.vz || 0) - (n.z || 0) * a;
      if (vertical) n.vy = (n.vy || 0) - (n.y || 0) * a;
    }
  };
  force.initialize = (list: Body[]) => { nodes = list; };
  force.strength = (v: number) => { k = v; return force; };
  force.vertical = (v: boolean) => { vertical = v; return force; };
  return force;
}
