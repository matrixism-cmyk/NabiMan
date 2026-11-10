/**
 * What the per-frame effects need of a node's 3D object: its materials (to
 * dim it), its name (to fade with distance), and what breathes or turns.
 * Gathered once when the object is made, so a frame never walks the scene.
 */
/** A three.js object or material: three ships no types this TypeScript can read (see three.d.ts). */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type Three = any;

export interface NodeParts {
  /** Every material in the object, with the opacity it has at full view. */
  mats: { m: Three; base: number }[];
  /** The name: fades in with distance (simple style); always shown when absent. */
  label?: Three;
  /** A second line (address, task) shown only while the node is in focus. */
  detail?: Three;
  /** The main body: breathes while at work, jumps when a message reaches it. */
  core?: Three;
  /** The rich style's glow round a working session. */
  halo?: Three;
  busy: boolean;
  spin: Three[];
  turn: Three[];
  // Current values, eased towards their targets frame by frame.
  alpha: number;
  labelAlpha: number;
  detailAlpha: number;
  /** When a message last reached it (ms, the frame clock), for a short jump. */
  ping: number;
}

/** Gathers the parts of a node object; `skip` are materials the effects set by themselves (the halo). */
export function collectParts(root: Three, extra: Partial<NodeParts> = {}): NodeParts {
  const mats: NodeParts['mats'] = [];
  const own = new Set<Three>([extra.label?.material, extra.detail?.material, extra.halo?.material].filter(Boolean));
  root.traverse((o: Three) => {
    const list = Array.isArray(o.material) ? o.material : o.material ? [o.material] : [];
    for (const m of list) {
      if (own.has(m)) continue;
      m.transparent = true;
      mats.push({ m, base: typeof m.opacity === 'number' ? m.opacity : 1 });
    }
  });
  for (const s of [extra.label, extra.detail]) {
    if (!s) continue;
    s.material.transparent = true;
    s.material.depthWrite = false;
    s.material.opacity = 0;
    // A faded name is no hover target: pointing at empty space must not light a node up.
    s.raycast = () => undefined;
  }
  return { mats, busy: false, spin: [], turn: [], alpha: 1, labelAlpha: 0, detailAlpha: 0, ping: -1e9, ...extra };
}
