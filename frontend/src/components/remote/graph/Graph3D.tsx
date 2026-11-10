import React, { useEffect, useRef } from 'react';
import ForceGraph3D from '3d-force-graph';
import * as THREE from 'three';
import SpriteText from 'three-spritetext';
import { Graph, GraphNode, MapEvent, signatureOf } from './graphModel';

interface Props {
  graph: Graph;
  selected: string | null;
  onSelect: (id: string | null) => void;
  onHoverEdge: (note: string | null, x: number, y: number) => void;
  /** Turn slowly while nobody touches it. */
  autoRotate: boolean;
  /** Messages between sessions, oldest first: each new one flies along its path once. */
  events?: MapEvent[];
}

type N = GraphNode & { x?: number; y?: number; z?: number; fx?: number; fy?: number; fz?: number };
type L = {
  id: string; source: string | N; target: string | N; live: boolean; strength: number; color: string; note: string;
  kind: 'order' | 'host' | 'conn' | 'ghost' | 'report';
};

const radius = (n: GraphNode) => (n.kind === 'server' ? 11 : n.lead ? 15 : (n.claude ? 9 : 6.5) + Math.min(n.heat || 0, 12) * 0.25);
const REPORT_COLOR = '#7dd3fc';

/** A colour dimmed towards the background, as an arrow fades once its work is done. */
function dim(hex: string, strength: number) {
  const c = new THREE.Color(hex);
  return `#${c.lerp(new THREE.Color('#0b1220'), 1 - Math.max(0.25, strength)).getHexString()}`;
}

function label(text: string, size: number, color: string, y: number, boxed = false) {
  const t = new SpriteText(text, size, color);
  if (boxed) { t.backgroundColor = 'rgba(11,18,32,0.6)'; t.padding = 1.5; t.borderRadius = 2; t.fontWeight = '600'; }
  t.position.set(0, y, 0);
  return t;
}

/** A machine: a wire-frame crystal with its name and address, its sessions gathered round it. */
function serverObject(n: GraphNode) {
  const group = new THREE.Group();
  const r = radius(n);
  const color = new THREE.Color(n.alive ? n.color : '#64748b');
  const shell = new THREE.Mesh(new THREE.IcosahedronGeometry(r, 1), new THREE.MeshBasicMaterial({ color, wireframe: true, transparent: true, opacity: 0.75 }));
  shell.userData.turn = true;
  group.add(shell);
  group.add(new THREE.Mesh(new THREE.IcosahedronGeometry(r * 0.55, 0), new THREE.MeshStandardMaterial({ color, emissive: color, emissiveIntensity: 0.55, flatShading: true, transparent: true, opacity: n.alive ? 0.95 : 0.4 })));
  group.add(label(`▣ ${n.name}`, 6.2, '#e6edf3', -(r + 8), true));
  if (n.sub) group.add(label(n.sub, 4.2, '#9da7b3', -(r + 15.5)));
  if (n.status && n.status !== 'online') group.add(label(n.status, 3.8, '#f87171', -(r + 21)));
  return group;
}

/** A session: a glowing sphere with its name; `userData.pulse` marks one at work. */
function sessionObject(n: GraphNode) {
  const group = new THREE.Group();
  const r = radius(n);
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
  if (n.lead) {
    const ring = new THREE.Mesh(new THREE.TorusGeometry(r * 1.55, 0.5, 12, 64), new THREE.MeshBasicMaterial({ color: '#fde68a', transparent: true, opacity: 0.85 }));
    ring.rotation.x = Math.PI / 2.4;
    ring.userData.spin = true;
    group.add(ring);
  }
  group.add(label(`${n.lead ? '★ ' : ''}${n.name}`, n.lead ? 9 : 6.5, '#e6edf3', -(r + 8), true));
  // The AI in it: Claude Code, by its own mark.
  if (n.claude) group.add(label(`✳ ${n.claude}`, 4.6, '#f0a67a', -(r + 16)));
  // What it is working on, above it, while it works.
  if (n.task) group.add(label(n.task.length > 46 ? `${n.task.slice(0, 45)}…` : n.task, 4.4, '#bbf7d0', r + 9, true));
  group.userData = { pulse: n.busy, halo, core };
  return group;
}

/** The sessions and their machines as lights in space; work flows along the arrows as particles. */
export default function Graph3D({ graph, selected, onSelect, onHoverEdge, autoRotate, events }: Props) {
  const box = useRef<HTMLDivElement | null>(null);
  const fg = useRef<any>(null);
  const pointer = useRef({ x: 0, y: 0 });
  const handlers = useRef({ onSelect, onHoverEdge });
  handlers.current = { onSelect, onHoverEdge };
  const lastEvent = useRef<number | null>(null);
  const shape = graph.nodes.map((n) => n.id).join(',') + '|' + graph.edges.map((e) => e.id).join(',');
  const signature = signatureOf(graph) + graph.nodes.map((n) => `${n.task || ''}|${n.heat || 0}|${n.status || ''}`).join(';');

  useEffect(() => {
    const el = box.current;
    if (!el) return undefined;
    const shown = (l: L) => l.kind === 'order';
    const g = new (ForceGraph3D as any)(el, { controlType: 'orbit' })
      .backgroundColor('rgba(0,0,0,0)')
      .showNavInfo(false)
      .nodeThreeObject((n: N) => (n.kind === 'server' ? serverObject(n) : sessionObject(n)))
      .nodeLabel(() => '')
      .linkVisibility((l: L) => l.kind !== 'ghost' && l.kind !== 'report')
      .linkColor((l: L) => (shown(l) ? dim(l.color, l.strength) : dim(l.color, l.kind === 'conn' ? 0.45 : 0.3)))
      .linkWidth((l: L) => (shown(l) ? (l.live ? 2 : 1) : l.kind === 'conn' ? 0.5 : 0.25))
      .linkOpacity(0.85)
      .linkCurvature((l: L) => (shown(l) ? 0.12 : 0))
      .linkDirectionalArrowLength((l: L) => (shown(l) ? 5.5 : 0))
      .linkDirectionalArrowRelPos(0.86)
      .linkDirectionalArrowColor((l: L) => dim(l.color, l.strength))
      .linkDirectionalParticles((l: L) => (shown(l) && l.live ? 5 : 0))
      .linkDirectionalParticleWidth((l: L) => (l.kind === 'report' ? 4.5 : 3.4))
      .linkDirectionalParticleSpeed(0.008)
      .linkDirectionalParticleColor((l: L) => (l.kind === 'report' ? REPORT_COLOR : l.color))
      .onNodeClick((n: N) => {
        handlers.current.onSelect(n.kind === 'server' ? null : n.id);
        const d = Math.hypot(n.x || 0, n.y || 0, n.z || 0) || 1;
        const k = 1 + 120 / d;
        g.cameraPosition({ x: (n.x || 0) * k, y: (n.y || 0) * k, z: (n.z || 0) * k + (d < 5 ? 150 : 0) }, n, 900);
      })
      .onBackgroundClick(() => handlers.current.onSelect(null))
      .onLinkHover((l: L | null) => handlers.current.onHoverEdge(l?.note || null, pointer.current.x, pointer.current.y));
    // Sessions stay by their machine; machines keep their distance from each other.
    g.d3Force('charge')?.strength((n: N) => (n.kind === 'server' ? -420 : -110)).distanceMax(320);
    g.d3Force('link')
      ?.distance((l: L) => (l.kind === 'host' ? 42 : l.kind === 'conn' ? 150 : l.kind === 'ghost' ? 150 : l.kind === 'report' ? 90 : 95))
      .strength((l: L) => (l.kind === 'host' ? 1 : l.kind === 'conn' ? 0.5 : l.kind === 'ghost' ? 0.25 : l.kind === 'report' ? 0 : 0.15));
    // Start close: the layout settles within a few hundred units of the centre.
    g.cameraPosition({ x: 0, y: 80, z: 420 });
    g.cooldownTicks(160);
    const scene = g.scene();
    scene.add(new THREE.AmbientLight('#8fb4ff', 1.1));
    const key = new THREE.PointLight('#ffffff', 900, 0, 1.6);
    key.position.set(120, 160, 220);
    scene.add(key);
    // A field of faint stars far behind, for depth as the view turns.
    const stars = new THREE.BufferGeometry();
    const pts = new Float32Array(900 * 3);
    for (let i = 0; i < pts.length; i += 1) pts[i] = (Math.random() - 0.5) * 2400;
    stars.setAttribute('position', new THREE.BufferAttribute(pts, 3));
    scene.add(new THREE.Points(stars, new THREE.PointsMaterial({ color: '#7c93b8', size: 1.6, transparent: true, opacity: 0.55, sizeAttenuation: true })));
    fg.current = g;

    const move = (e: MouseEvent) => { pointer.current = { x: e.clientX, y: e.clientY }; };
    el.addEventListener('mousemove', move);
    const size = () => { g.width(el.clientWidth).height(el.clientHeight); };
    size();
    const watch = new ResizeObserver(size);
    watch.observe(el);
    // Sessions at work breathe; the lead's ring and the machines' shells turn.
    let frame = 0;
    const tick = (ms: number) => {
      const beat = 0.5 + 0.5 * Math.sin(ms / 420);
      scene.traverse((o: any) => {
        if (o.userData?.pulse) {
          o.userData.halo.material.opacity = 0.1 + 0.22 * beat;
          o.userData.halo.scale.setScalar(1 + 0.18 * beat);
          o.userData.core.material.emissiveIntensity = 0.6 + 0.6 * beat;
        }
        if (o.userData?.spin) o.rotation.z += 0.012;
        if (o.userData?.turn) { o.rotation.y += 0.004; o.rotation.x += 0.0015; }
      });
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
      watch.disconnect();
      el.removeEventListener('mousemove', move);
      g._destructor?.();
      fg.current = null;
      el.innerHTML = '';
    };
  }, []);

  useEffect(() => {
    const g = fg.current;
    if (!g) return;
    // Keep where the nodes already are, so a refresh does not throw them about.
    const before = new Map<string, N>((g.graphData().nodes as N[]).map((n) => [n.id, n]));
    const nodes: N[] = graph.nodes.map((n) => {
      const old = before.get(n.id);
      const next: N = { ...n, x: old?.x, y: old?.y, z: old?.z };
      if (n.lead) { next.fx = 0; next.fy = 0; next.fz = 0; }
      return next;
    });
    const links: L[] = graph.edges.map((e) => ({ id: e.id, source: e.from, target: e.to, live: e.live, strength: e.strength, color: e.color, note: e.note, kind: e.kind || 'order' }));
    // A hand-off's answer travels back along an unseen twin of its arrow.
    for (const e of graph.edges) {
      if ((e.kind || 'order') === 'order') links.push({ id: `report>${e.id}`, source: e.to, target: e.from, live: false, strength: 0, color: REPORT_COLOR, note: '', kind: 'report' });
    }
    // Sessions nothing reaches are tied to the centre by an unseen thread, so they orbit instead of drifting off.
    const hub = nodes.find((n) => n.lead) || nodes[0];
    const tied = new Set(links.flatMap((l) => [l.source as string, l.target as string]));
    if (hub) for (const n of nodes) if (n !== hub && !tied.has(n.id)) links.push({ id: `ghost>${n.id}`, source: hub.id, target: n.id, live: false, strength: 0, color: '#000000', note: '', kind: 'ghost' });
    g.graphData({ nodes, links });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shape, signature]);

  // Each new message flies along its path once: an order along its arrow, a report back along the twin.
  useEffect(() => {
    const g = fg.current;
    if (!g || !events) return;
    const newest = events.length ? events[events.length - 1].id : 0;
    if (lastEvent.current === null) { lastEvent.current = newest; return; }
    const links = g.graphData().links as L[];
    const end = (v: string | N) => (typeof v === 'string' ? v : v.id);
    for (const e of events) {
      if (e.id <= lastEvent.current || !e.from || !e.to) continue;
      const path = links.find((l) => (l.kind === 'order' || l.kind === 'report') && end(l.source) === e.from && end(l.target) === e.to);
      if (path) for (let i = 0; i < 3; i += 1) setTimeout(() => fg.current?.emitParticle(path), i * 160);
    }
    lastEvent.current = newest;
  }, [events]);

  useEffect(() => {
    const g = fg.current;
    if (!g) return;
    const controls = g.controls();
    controls.autoRotate = autoRotate && !selected;
    controls.autoRotateSpeed = 0.6;
  }, [autoRotate, selected]);

  return <div className="session-graph-canvas is-3d" ref={box} data-testid="graph-3d" />;
}
