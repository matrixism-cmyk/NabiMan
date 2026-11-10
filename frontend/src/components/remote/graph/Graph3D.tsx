import React, { useEffect, useMemo, useRef } from 'react';
import ForceGraph3D from '3d-force-graph';
import { Graph, GraphNode, MapEvent, signatureOf } from './graphModel';
import type { GraphSettings } from './view/graphSettings';
import { paletteOf } from './view/graphTheme';
import { adjacency, Adjacency, degrees, forcedLabels, pickFocus } from './view/graphFocus';
import { chargeOf, chargeReach, centerPull, centerStrength, distanceOf, strengthOf } from './view/graphForces';
import { SceneFx } from './view/sceneFx';
import { richObject } from './view/richObjects';
import { simpleObject } from './view/simpleObjects';
import { arrowLength, LinkData, linkLook, linkShown, linkWidth, particleColor, particleCount, particleWidth, REPORT_COLOR, isOrder } from './view/linkStyle';
import { addLights, addStars, flyTo } from './view/sceneSetup';
import { watchContext } from './view/glContext';

interface Props {
  graph: Graph;
  selected: string | null;
  onSelect: (id: string | null) => void;
  onHoverEdge: (note: string | null, x: number, y: number) => void;
  settings: GraphSettings;
  /** The app is in a light theme: the simple style draws on a light stage. */
  light?: boolean;
  /** No turning, no breathing. */
  reducedMotion?: boolean;
  /** Nodes a search found: brought forward while nothing is hovered or picked. */
  highlight?: string[];
  /** Fly the camera to this node (a new `seq` flies again). */
  flyTo?: { id: string; seq: number } | null;
  /** Messages between sessions, oldest first: each new one flies along its path once. */
  events?: MapEvent[];
  /** Messages to play now, whatever their age (the time-lapse); a new `seq` plays the list. */
  bursts?: { seq: number; list: MapEvent[] } | null;
  /** A new value frames the whole graph once its layout settles (the local graph and back). */
  fitKey?: string;
}

type N = GraphNode & { x?: number; y?: number; z?: number; fx?: number; fy?: number; fz?: number };
type L = LinkData & { source: string | N; target: string | N; note: string };

/** Where the network's lowest tier stands above the lead, and the step between tiers. */
const BASE_Y = 70;
const TIER_GAP = 58;
/** The height the machines stand at when the network is drawn. */
const SERVER_Y = 14;
/** The height the sessions hang at under them. */
const SESSION_Y = -30;
const end = (v: string | N) => (typeof v === 'string' ? v : v.id);

/** The sessions and their machines in space, drawn simple (as in Obsidian) or rich; work flows along the arrows as particles. */
export default function Graph3D({ graph, selected, onSelect, onHoverEdge, settings, light = false, reducedMotion = false, highlight = [], flyTo: target, events, bursts, fitKey = '' }: Props) {
  const box = useRef<HTMLDivElement | null>(null);
  const fg = useRef<any>(null);
  const fx = useRef<SceneFx | null>(null);
  const stars = useRef<any>(null);
  const pointer = useRef({ x: 0, y: 0 });
  const hover = useRef<string | null>(null);
  const adj = useRef<Adjacency>(adjacency([]));
  const view = useRef({ selected, highlight, rotate: false, reduced: reducedMotion });
  view.current.selected = selected;
  view.current.highlight = highlight;
  view.current.reduced = reducedMotion;
  const handlers = useRef({ onSelect, onHoverEdge });
  handlers.current = { onSelect, onHoverEdge };
  const lastEvent = useRef<number | null>(null);
  /** Frame the graph when the layout next comes to rest: at first, and after the local graph changes. */
  const fit = useRef(true);
  const shape = graph.nodes.map((n) => n.id).join(',') + '|' + graph.edges.map((e) => e.id).join(',');
  const signature = signatureOf(graph) + graph.nodes.map((n) => `${n.task || ''}|${n.heat || 0}|${n.status || ''}|${(n.info || []).join('/')}`).join(';');
  const deg = useMemo(() => degrees(graph.edges.map((e) => ({ id: e.id, from: e.from, to: e.to }))), [shape]); // eslint-disable-line react-hooks/exhaustive-deps
  const degRef = useRef(deg);
  degRef.current = deg;
  const tiered = graph.nodes.some((n) => n.kind === 'device');
  const palette = paletteOf(settings.style, light);
  const look = useRef({ settings, palette });
  look.current = { settings, palette };

  /** Who is in focus now: hover over selection over search. */
  const refocus = () => {
    const f = fx.current;
    if (!f) return;
    const input = { hover: hover.current, selected: view.current.selected, matches: view.current.highlight };
    f.setFocus(pickFocus(adj.current, input), forcedLabels(adj.current, input));
  };
  const rotate = () => {
    const g = fg.current;
    if (!g) return;
    const controls = g.controls();
    controls.autoRotate = view.current.rotate && !view.current.selected && !hover.current;
    controls.autoRotateSpeed = 0.6;
  };

  useEffect(() => {
    const el = box.current;
    if (!el) return undefined;
    const effects = new SceneFx();
    fx.current = effects;
    const g = new (ForceGraph3D as any)(el, { controlType: 'orbit' })
      .backgroundColor('rgba(0,0,0,0)')
      .showNavInfo(false)
      .nodeLabel(() => '')
      .linkVisibility((l: L) => linkShown(l))
      .linkMaterial((l: L) => effects.linkMaterial(l, linkLook(l, look.current.settings, look.current.palette)))
      .linkDirectionalArrowRelPos(0.86)
      .linkDirectionalParticleSpeed(0.008)
      .linkDirectionalParticleColor((l: L) => particleColor(l))
      .onNodeClick((n: N) => {
        // A machine opens its attack watch; a session its panel.
        handlers.current.onSelect(n.kind === 'device' ? null : n.id);
        flyTo(g, n, view.current.reduced ? 0 : 900);
      })
      .onNodeHover((n: N | null) => {
        hover.current = n?.id ?? null;
        el.style.cursor = n ? 'pointer' : '';
        refocus();
        rotate();
      })
      .onBackgroundClick(() => handlers.current.onSelect(null))
      .onLinkHover((l: L | null) => handlers.current.onHoverEdge(l?.note || null, pointer.current.x, pointer.current.y))
      .onEngineTick(() => effects.touch(true))
      .onEngineStop(() => { if (fit.current) { fit.current = false; g.zoomToFit(700, 50); } });
    g.d3Force('center', centerPull());
    // Start close: the layout settles within a few hundred units of the centre.
    g.cameraPosition({ x: 0, y: 150, z: 560 });
    g.cooldownTicks(160);
    const scene = g.scene();
    addLights(scene);
    stars.current = addStars(scene);
    fg.current = g;
    // Should the browser take the context back (too many open, a GPU reset), the canvas stays blank until it
    // returns: let it be given back, and redraw everything then.
    const unwatch = watchContext(g.renderer().domElement, () => effects.touch(true));
    // For looking at the layout from the browser's console (and the screenshot check).
    (el as any).__graph = g;
    (el as any).__fx = effects;
    (el as any).__gl = unwatch.counts;

    const move = (e: MouseEvent) => { pointer.current = { x: e.clientX, y: e.clientY }; };
    el.addEventListener('mousemove', move);
    const size = () => { g.width(el.clientWidth).height(el.clientHeight); };
    size();
    const watch = new ResizeObserver(size);
    watch.observe(el);
    let frame = 0;
    const tick = (ms: number) => {
      effects.frame(ms, g.camera());
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
      unwatch();
      watch.disconnect();
      el.removeEventListener('mousemove', move);
      g._destructor?.();
      fg.current = null;
      fx.current = null;
      el.innerHTML = '';
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // The look: rebuilt only when the style, the theme or a size changes.
  const { style, nodeSize, colorGroups, arrows, linkThickness } = settings;
  useEffect(() => {
    const g = fg.current;
    const effects = fx.current;
    if (!g || !effects) return;
    const s = look.current.settings;
    const p = look.current.palette;
    effects.fadeLabels = style === 'simple';
    effects.resetLinks();
    effects.dimmed = p.dimmed;
    effects.faded = p.fadedOpacity;
    if (stars.current) { stars.current.material.opacity = p.stars; stars.current.visible = p.stars > 0; }
    g.nodeThreeObject((n: N) => {
      const obj = style === 'rich' ? richObject(n) : simpleObject(n, { degree: degRef.current.get(n.id) || 0, palette: p, nodeSize, colorGroups });
      effects.addNode(n.id, obj);
      return obj;
    })
      .linkWidth((l: L) => linkWidth(l, s))
      .linkCurvature((l: L) => (isOrder(l) ? 0.12 : 0))
      .linkDirectionalArrowLength((l: L) => arrowLength(l, s))
      .linkDirectionalArrowColor((l: L) => (l.live ? l.color : p.link))
      .linkDirectionalParticles((l: L) => particleCount(l, s))
      .linkDirectionalParticleWidth((l: L) => particleWidth(l, s));
    const later = setTimeout(() => effects.touch(true), 30);
    return () => clearTimeout(later);
  }, [style, light, nodeSize, colorGroups, arrows, linkThickness]);

  useEffect(() => {
    const g = fg.current;
    if (!g) return;
    // Keep where the nodes already are, so a refresh does not throw them about.
    const before = new Map<string, N>((g.graphData().nodes as N[]).map((n) => [n.id, n]));
    const deepest = Math.max(0, ...graph.nodes.map((n) => n.layer ?? 0));
    const nodes: N[] = graph.nodes.map((n) => {
      const old = before.get(n.id);
      const next: N = { ...n, x: old?.x, y: old?.y, z: old?.z };
      if (n.lead) { next.fx = 0; next.fy = 0; next.fz = 0; }
      // The network stands in tiers above the sessions: the internet on top, each hop inwards a step down.
      if (n.kind === 'device') next.fy = BASE_Y + (deepest - (n.layer ?? deepest)) * TIER_GAP;
      // With the network drawn, the machines stand in a tier of their own just under it (level with the lead),
      // each one's sessions hanging close below: left free, the tiers above pressed the busiest machine far down.
      if (n.kind === 'server' && tiered) next.fy = SERVER_Y;
      // …and the sessions in a tier just under their machines, spread round them.
      if (n.kind === 'session' && !n.lead && tiered) next.fy = SESSION_Y;
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
    adj.current = adjacency(graph.edges.map((e) => ({ id: e.id, from: e.from, to: e.to })));
    fx.current?.keepOnly(new Set(nodes.map((n) => n.id)), new Set(links.map((l) => l.id)));
    g.graphData({ nodes, links });
    refocus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shape, signature]);

  // The forces, live: across only when the tiers hold the heights.
  const { center, repel, linkStrength, linkDistance } = settings;
  useEffect(() => {
    const g = fg.current;
    if (!g) return;
    const s = { center, repel, linkStrength, linkDistance };
    g.d3Force('charge')?.strength((n: N) => chargeOf(n.kind, s)).distanceMax(chargeReach(s));
    g.d3Force('link')?.distance((l: L) => distanceOf(l.kind, s)).strength((l: L) => strengthOf(l.kind, s));
    g.d3Force('center')?.strength?.(centerStrength(s)).vertical?.(!tiered);
    g.d3ReheatSimulation();
  }, [center, repel, linkStrength, linkDistance, tiered]);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { refocus(); }, [selected, highlight.join('\n')]);

  const lastFit = useRef(fitKey);
  useEffect(() => {
    if (fitKey === lastFit.current) return;
    lastFit.current = fitKey;
    fit.current = true;
    fg.current?.d3ReheatSimulation();
  }, [fitKey]);

  useEffect(() => {
    if (!fx.current) return;
    fx.current.threshold = settings.labelThreshold;
    fx.current.reduced = reducedMotion;
    fx.current.touch();
  }, [settings.labelThreshold, reducedMotion]);

  /** Each message flies along its path: an order along its arrow, a report back along the twin; where it lands jumps. */
  const play = (list: MapEvent[]) => {
    const g = fg.current;
    if (!g) return;
    const links = g.graphData().links as L[];
    const now = performance.now();
    list.forEach((e, k) => {
      if (!e.from || !e.to) return;
      const path = links.find((l) => (l.kind === 'order' || l.kind === 'report') && end(l.source) === e.from && end(l.target) === e.to);
      if (path) for (let i = 0; i < 3; i += 1) setTimeout(() => fgEmit(path), k * 40 + i * 160);
      fx.current?.ping(e.to, now + k * 40 + 450);
    });
  };
  const fgEmit = (path: L) => { fg.current?.emitParticle(path); };

  useEffect(() => {
    if (!fg.current || !events) return;
    const newest = events.length ? events[events.length - 1].id : 0;
    if (lastEvent.current === null) { lastEvent.current = newest; return; }
    play(events.filter((e) => e.id > (lastEvent.current as number)));
    lastEvent.current = newest;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [events]);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { if (bursts?.list.length) play(bursts.list); }, [bursts?.seq]);

  useEffect(() => {
    const g = fg.current;
    if (!g || !target) return;
    const n = (g.graphData().nodes as N[]).find((x) => x.id === target.id);
    if (n) flyTo(g, n, view.current.reduced ? 0 : 900);
  }, [target?.seq]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    view.current.rotate = settings.autoRotate && !reducedMotion;
    rotate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.autoRotate, reducedMotion, selected]);

  return <div className="session-graph-canvas is-3d" ref={box} data-testid="graph-3d" data-style={settings.style} />;
}
