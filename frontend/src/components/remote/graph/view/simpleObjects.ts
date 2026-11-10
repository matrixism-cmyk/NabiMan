/**
 * The simple style ("심플"), the default: small flat-shaded spheres sized by
 * how many links they have, one name each that fades in as the camera comes
 * near, and a second line (address, task) only while a node is in focus.
 */
import * as THREE from 'three';
import SpriteText from 'three-spritetext';
import type { GraphNode } from '../graphModel';
import { DEVICE_COLOR, DEVICE_MARK } from './richObjects';
import { collectParts, Three } from './nodeParts';
import type { Palette } from './graphTheme';

export interface SimpleOptions { degree: number; palette: Palette; nodeSize: number; colorGroups: boolean }

const BASE: Record<string, number> = { session: 4.2, server: 6, device: 3.6 };

/** A node's radius: by kind, growing with the log of its links; the lead a little larger. */
export function simpleRadius(n: GraphNode, degree: number, nodeSize = 1): number {
  const base = BASE[n.kind || 'session'] ?? BASE.session;
  return base * (1 + 0.45 * Math.log(1 + Math.max(0, degree))) * (n.lead ? 1.35 : 1) * nodeSize;
}

function colorOf(n: GraphNode, p: Palette, groups: boolean): string {
  if (n.kind === 'device') return groups ? DEVICE_COLOR[n.device || 'other'] || DEVICE_COLOR.other : p.node;
  if (!n.alive) return p.ended;
  if (n.lead) return p.lead;
  if (n.busy) return p.busy;
  return groups ? n.color : p.node;
}

function shapeOf(n: GraphNode, r: number) {
  if (n.kind === 'server') return new THREE.IcosahedronGeometry(r, 0);
  if (n.kind !== 'device') return new THREE.SphereGeometry(r, 12, 8);
  switch (n.device) {
    case 'firewall': return new THREE.BoxGeometry(r * 2.2, r * 1.4, r * 0.4);
    case 'node': return new THREE.BoxGeometry(r * 1.8, r * 0.8, r * 1.4);
    case 'switch': return new THREE.CylinderGeometry(r, r, r * 0.35, 6);
    case 'storage': return new THREE.CylinderGeometry(r * 0.8, r * 0.8, r * 1.1, 8);
    case 'internet': return new THREE.SphereGeometry(r * 1.4, 10, 6);
    default: return new THREE.OctahedronGeometry(r, 0);
  }
}

function text(value: string, size: number, color: string, y: number) {
  const t = new SpriteText(value, size, color);
  t.fontFace = 'system-ui, -apple-system, "Segoe UI", sans-serif';
  t.position.set(0, y, 0);
  return t;
}

const clip = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);

/** The second line shown in focus: a machine's address and state, a session's Claude or its task. */
function detailOf(n: GraphNode): string {
  if (n.kind === 'server') return [n.sub, n.status && n.status !== 'online' ? n.status : ''].filter(Boolean).join(' · ');
  if (n.kind === 'device') return n.sub || '';
  if (n.busy && n.task) return clip(n.task, 46);
  return n.claude || '';
}

export function simpleObject(n: GraphNode, o: SimpleOptions): Three {
  const group = new THREE.Group();
  const r = simpleRadius(n, o.degree, o.nodeSize);
  const color = new THREE.Color(colorOf(n, o.palette, o.colorGroups));
  const core = new THREE.Mesh(shapeOf(n, r), new THREE.MeshLambertMaterial({ color, flatShading: true, transparent: true, opacity: n.alive ? 1 : 0.5 }));
  group.add(core);
  if (n.lead) {
    // A soft ring round the lead, lying almost flat.
    const ring = new THREE.Mesh(new THREE.TorusGeometry(r * 1.7, Math.max(0.18, 0.22 * o.nodeSize), 6, 48), new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.35, depthWrite: false }));
    ring.rotation.x = Math.PI / 2.2;
    group.add(ring);
  }
  const mark = n.kind === 'server' ? '▣ ' : n.kind === 'device' ? `${DEVICE_MARK[n.device || 'other'] || ''} ` : n.lead ? '★ ' : '';
  const size = (n.lead ? 6.6 : n.kind === 'server' ? 6 : 5.2) * Math.sqrt(o.nodeSize);
  const name = text(`${mark}${clip(n.name, 40)}`, size, o.palette.label, -(r + size * 0.9 + 1));
  group.add(name);
  const line = detailOf(n);
  const detail = line ? text(line, size * 0.72, n.busy && n.task ? o.palette.busy : o.palette.sub, -(r + size * 2.1 + 1.5)) : undefined;
  if (detail) group.add(detail);
  group.userData.parts = collectParts(group, { core, label: name, detail, busy: n.busy && n.alive });
  return group;
}
