import React, { useEffect, useRef } from 'react';
import ForceGraph3D from '3d-force-graph';
import * as THREE from 'three';
import SpriteText from 'three-spritetext';
import { Graph, GraphNode, signatureOf } from './graphModel';

interface Props {
  graph: Graph;
  selected: string | null;
  onSelect: (id: string | null) => void;
  onHoverEdge: (note: string | null, x: number, y: number) => void;
  /** Turn slowly while nobody touches it. */
  autoRotate: boolean;
}

type N = GraphNode & { x?: number; y?: number; z?: number; fx?: number; fy?: number; fz?: number };
type L = { id: string; source: string | N; target: string | N; live: boolean; strength: number; color: string; note: string; ghost?: boolean };

const radius = (n: GraphNode) => (n.lead ? 15 : n.claude ? 9 : 6.5);

/** A colour dimmed towards the background, as an arrow fades once its work is done. */
function dim(hex: string, strength: number) {
  const c = new THREE.Color(hex);
  return `#${c.lerp(new THREE.Color('#0b1220'), 1 - Math.max(0.25, strength)).getHexString()}`;
}

/** A glowing sphere with its name under it; `userData.pulse` marks one at work. */
function nodeObject(n: GraphNode) {
  const group = new THREE.Group();
  const r = radius(n);
  const color = new THREE.Color(n.lead ? '#fbbf24' : n.busy ? '#4ade80' : n.color);
  const core = new THREE.Mesh(
    new THREE.SphereGeometry(r, 28, 28),
    new THREE.MeshStandardMaterial({ color, emissive: color, emissiveIntensity: n.busy || n.lead ? 0.9 : 0.35, roughness: 0.35, metalness: 0.2, transparent: true, opacity: n.alive ? 1 : 0.35 }),
  );
  group.add(core);
  // A soft halo: bigger and brighter for the lead and for sessions at work.
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
  const name = new SpriteText(`${n.lead ? '★ ' : ''}${n.name}`, n.lead ? 9 : 6.5, '#e6edf3');
  name.fontWeight = '600';
  name.backgroundColor = 'rgba(11,18,32,0.55)';
  name.padding = 1.5;
  name.borderRadius = 2;
  name.position.set(0, -(r + 8), 0);
  group.add(name);
  if (n.claude) {
    const sub = new SpriteText(n.claude, 4.6, '#9da7b3');
    sub.position.set(0, -(r + 16), 0);
    group.add(sub);
  }
  group.userData = { pulse: n.busy, halo, core };
  return group;
}

/** The sessions as lights in space; work being done flows along the arrows as particles. */
export default function Graph3D({ graph, selected, onSelect, onHoverEdge, autoRotate }: Props) {
  const box = useRef<HTMLDivElement | null>(null);
  const fg = useRef<any>(null);
  const pointer = useRef({ x: 0, y: 0 });
  const handlers = useRef({ onSelect, onHoverEdge });
  handlers.current = { onSelect, onHoverEdge };
  const shape = graph.nodes.map((n) => n.id).join(',') + '|' + graph.edges.map((e) => e.id).join(',');
  const signature = signatureOf(graph);

  useEffect(() => {
    const el = box.current;
    if (!el) return undefined;
    const g = new (ForceGraph3D as any)(el, { controlType: 'orbit' })
      .backgroundColor('rgba(0,0,0,0)')
      .showNavInfo(false)
      .nodeThreeObject((n: N) => nodeObject(n))
      .nodeLabel(() => '')
      .linkColor((l: L) => dim(l.color, l.strength))
      .linkWidth((l: L) => (l.live ? 2 : 1))
      .linkOpacity(0.85)
      .linkVisibility((l: L) => !l.ghost)
      .linkCurvature(0.12)
      .linkDirectionalArrowLength(5.5)
      .linkDirectionalArrowRelPos(0.86)
      .linkDirectionalArrowColor((l: L) => dim(l.color, l.strength))
      .linkDirectionalParticles((l: L) => (l.live ? 5 : 0))
      .linkDirectionalParticleWidth(3.4)
      .linkDirectionalParticleSpeed(0.008)
      .linkDirectionalParticleColor((l: L) => l.color)
      .onNodeClick((n: N) => {
        handlers.current.onSelect(n.id);
        const d = Math.hypot(n.x || 0, n.y || 0, n.z || 0) || 1;
        const k = 1 + 120 / d;
        g.cameraPosition({ x: (n.x || 0) * k, y: (n.y || 0) * k, z: (n.z || 0) * k + (d < 5 ? 150 : 0) }, n, 900);
      })
      .onBackgroundClick(() => handlers.current.onSelect(null))
      .onLinkHover((l: L | null) => handlers.current.onHoverEdge(l?.note || null, pointer.current.x, pointer.current.y));
    // Sessions with no arrows stay near instead of drifting off into the dark.
    g.d3Force('charge')?.strength(-150).distanceMax(220);
    g.d3Force('link')?.distance((l: L) => (l.ghost ? 150 : 80)).strength((l: L) => (l.ghost ? 0.25 : 0.9));
    // Start close: the layout settles within a few hundred units of the centre.
    g.cameraPosition({ x: 0, y: 60, z: 330 });
    g.cooldownTicks(140);
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
    // Sessions at work breathe; the lead's ring turns.
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
    const links: L[] = graph.edges.map((e) => ({ id: e.id, source: e.from, target: e.to, live: e.live, strength: e.strength, color: e.color, note: e.note }));
    // Sessions no arrow reaches are tied to the centre by an unseen thread, so they orbit instead of drifting off.
    const hub = nodes.find((n) => n.lead) || nodes[0];
    const tied = new Set(links.flatMap((l) => [l.source as string, l.target as string]));
    if (hub) for (const n of nodes) if (n !== hub && !tied.has(n.id)) links.push({ id: `ghost>${n.id}`, source: hub.id, target: n.id, live: false, strength: 0, color: '#000000', note: '', ghost: true });
    g.graphData({ nodes, links });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shape, signature]);

  useEffect(() => {
    const g = fg.current;
    if (!g) return;
    const controls = g.controls();
    controls.autoRotate = autoRotate && !selected;
    controls.autoRotateSpeed = 0.6;
  }, [autoRotate, selected]);

  return <div className="session-graph-canvas is-3d" ref={box} data-testid="graph-3d" />;
}
