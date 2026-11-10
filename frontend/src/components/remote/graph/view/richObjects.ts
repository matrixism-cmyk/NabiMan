/**
 * The rich style ("화려하게"): glowing spheres, wire-frame crystals for the
 * machines, solids for the network devices, and every name and line shown.
 */
import * as THREE from 'three';
import SpriteText from 'three-spritetext';
import type { GraphNode } from '../graphModel';
import { AGENTS, shortModel } from '../AgentIcons';
import { collectParts, NodeParts, Three } from './nodeParts';

export const richRadius = (n: GraphNode) => (n.kind === 'device' ? (n.device === 'internet' ? 14 : n.device === 'firewall' ? 9 : 6) : n.kind === 'server' ? 11 : n.lead ? 15 : (n.claude ? 9 : 6.5) + Math.min(n.heat || 0, 12) * 0.25);

export const DEVICE_MARK: Record<string, string> = { firewall: '⛨', router: '⇄', gateway: '⇄', switch: '☰', node: '▦', vip: '◇', dongle: '⌁', internet: '◎', storage: '⛁', other: '◻' };
export const DEVICE_COLOR: Record<string, string> = { firewall: '#f87171', router: '#fbbf24', gateway: '#fbbf24', switch: '#38bdf8', node: '#a3e635', vip: '#c4b5fd', dongle: '#f472b6', internet: '#e2e8f0', storage: '#fb923c', other: '#94a3b8' };

export function label(text: string, size: number, color: string, y: number, boxed = false) {
  const t = new SpriteText(text, size, color);
  if (boxed) { t.backgroundColor = 'rgba(11,18,32,0.6)'; t.padding = 1.5; t.borderRadius = 2; t.fontWeight = '600'; }
  t.position.set(0, y, 0);
  return t;
}

/** A machine: a wire-frame crystal with its name and address, its sessions gathered round it. */
function serverObject(n: GraphNode): Three {
  const group = new THREE.Group();
  const r = richRadius(n);
  const color = new THREE.Color(n.alive ? n.color : '#64748b');
  const shell = new THREE.Mesh(new THREE.IcosahedronGeometry(r, 1), new THREE.MeshBasicMaterial({ color, wireframe: true, transparent: true, opacity: 0.75 }));
  group.add(shell);
  const core = new THREE.Mesh(new THREE.IcosahedronGeometry(r * 0.55, 0), new THREE.MeshStandardMaterial({ color, emissive: color, emissiveIntensity: 0.55, flatShading: true, transparent: true, opacity: n.alive ? 0.95 : 0.4 }));
  group.add(core);
  group.add(label(`▣ ${n.name}`, 6.2, '#e6edf3', -(r + 8), true));
  if (n.sub) group.add(label(n.sub, 4.2, '#9da7b3', -(r + 15.5)));
  let y = r + 21;
  if (n.status && n.status !== 'online') { group.add(label(n.status, 3.8, '#f87171', -y)); y += 5.5; }
  for (const line of n.info || []) { group.add(label(line.length > 44 ? `${line.slice(0, 43)}…` : line, 3.6, '#7f8ea3', -y)); y += 5.2; }
  group.userData.parts = collectParts(group, { core, turn: [shell] });
  return group;
}

/** A network device: a small solid by its kind (a wall for the firewall, a rack for a node…), with name and address. */
function deviceObject(n: GraphNode): Three {
  const group = new THREE.Group();
  const r = richRadius(n);
  const kind = n.device || 'other';
  const color = new THREE.Color(DEVICE_COLOR[kind] || DEVICE_COLOR.other);
  const solid = new THREE.MeshStandardMaterial({ color, emissive: color, emissiveIntensity: 0.45, roughness: 0.5, metalness: 0.3, flatShading: true });
  const shape = kind === 'firewall' ? new THREE.BoxGeometry(r * 2.4, r * 1.5, r * 0.5)
    : kind === 'node' ? new THREE.BoxGeometry(r * 2, r * 0.9, r * 1.6)
    : kind === 'switch' ? new THREE.CylinderGeometry(r, r, r * 0.45, 6)
    : kind === 'storage' ? new THREE.CylinderGeometry(r * 0.8, r * 0.8, r * 1.3, 16)
    : kind === 'vip' ? new THREE.OctahedronGeometry(r * 0.8, 0)
    : kind === 'internet' ? new THREE.SphereGeometry(r, 14, 10)
    : new THREE.OctahedronGeometry(r, 0);
  const mesh = new THREE.Mesh(shape, kind === 'internet' ? new THREE.MeshBasicMaterial({ color, wireframe: true, transparent: true, opacity: 0.6 }) : solid);
  group.add(mesh);
  group.add(label(`${DEVICE_MARK[kind] || ''} ${n.name}`, kind === 'firewall' || kind === 'internet' ? 5.6 : 4.4, '#cbd5e1', -(r + 6.5), true));
  if (n.sub) group.add(label(n.sub, 3.5, '#8391a5', -(r + 12)));
  group.userData.parts = collectParts(group, { core: mesh, turn: kind === 'internet' || kind === 'vip' ? [mesh] : [] });
  return group;
}

/** A session: a glowing sphere with its name; one at work breathes. */
function sessionObject(n: GraphNode): Three {
  const group = new THREE.Group();
  const r = richRadius(n);
  const color = new THREE.Color(n.lead ? '#fbbf24' : n.busy ? '#4ade80' : n.color);
  const core = new THREE.Mesh(
    new THREE.SphereGeometry(r, 28, 28),
    new THREE.MeshStandardMaterial({ color, emissive: color, emissiveIntensity: n.busy || n.lead ? 0.9 : 0.35, roughness: 0.35, metalness: 0.2, transparent: true, opacity: n.alive ? 1 : 0.35 }),
  );
  group.add(core);
  const halo = new THREE.Mesh(
    new THREE.SphereGeometry(r * (n.lead ? 1.9 : 1.6), 24, 24),
    new THREE.MeshBasicMaterial({ color, transparent: true, opacity: n.lead ? 0.16 : n.busy ? 0.2 : 0.06, depthWrite: false }),
  );
  group.add(halo);
  const spin: Three[] = [];
  if (n.lead) {
    const ring = new THREE.Mesh(new THREE.TorusGeometry(r * 1.55, 0.5, 12, 64), new THREE.MeshBasicMaterial({ color: '#fde68a', transparent: true, opacity: 0.85 }));
    ring.rotation.x = Math.PI / 2.4;
    spin.push(ring);
    group.add(ring);
  }
  group.add(label(`${n.lead ? '★ ' : ''}${n.name}`, n.lead ? 9 : 6.5, '#e6edf3', -(r + 8), true));
  // The AIs in it, each by its own mark: Claude Code with its name, the others by kind.
  let below = r + 16;
  if (n.claude) { group.add(label(`✳ ${n.claude}${n.model ? ` · ${shortModel(n.model)}` : ''}`, 4.6, AGENTS.claude.color, -below)); below += 6.5; }
  for (const k of (n.agents || []).filter((x) => AGENTS[x] && !(x === 'claude' && n.claude))) {
    group.add(label(`${AGENTS[k].mark} ${AGENTS[k].name}`, 4.6, AGENTS[k].color, -below));
    below += 6.5;
  }
  // What it is working on, above it, while it works.
  if (n.task) group.add(label(n.task.length > 46 ? `${n.task.slice(0, 45)}…` : n.task, 4.4, '#bbf7d0', r + 9, true));
  group.userData.parts = collectParts(group, { core, halo: n.busy ? halo : undefined, busy: n.busy, spin });
  return group;
}

export function richObject(n: GraphNode): Three {
  return n.kind === 'device' ? deviceObject(n) : n.kind === 'server' ? serverObject(n) : sessionObject(n);
}

export type { NodeParts };
