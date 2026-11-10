import React, { useEffect, useRef, useState } from 'react';
import cytoscape, { Core, ElementDefinition } from 'cytoscape';
import { depths, Graph, signatureOf } from './graphModel';

export type Layout2D = 'concentric' | 'tree' | 'free';
const POS_KEY = 'nabiman_graph2d_positions';

interface Props {
  graph: Graph;
  layout: Layout2D;
  selected: string | null;
  onSelect: (id: string | null) => void;
  /** Shown beside the pointer while it is over an arrow. */
  onHoverEdge: (note: string | null, x: number, y: number) => void;
}

const readPositions = (): Record<string, { x: number; y: number }> => {
  try { return JSON.parse(localStorage.getItem(POS_KEY) || '{}') || {}; } catch { return {}; }
};

function elements(g: Graph): ElementDefinition[] {
  return [
    ...g.nodes.map((n) => ({
      data: {
        id: n.id,
        label: `${n.lead ? '★ ' : ''}${n.name}${n.claude ? `\n${n.claude}` : ''}`,
        color: n.color,
        group: n.group,
      },
      classes: [n.busy ? 'busy' : '', n.lead ? 'lead' : '', n.alive ? '' : 'ended', n.claude ? 'has-claude' : ''].join(' '),
    })),
    ...g.edges.map((e) => ({
      data: { id: e.id, source: e.from, target: e.to, color: e.color, strength: e.strength, note: e.note },
      classes: e.live ? 'live' : '',
    })),
  ];
}

function layoutOf(kind: Layout2D, g: Graph, saved: Record<string, { x: number; y: number }>): cytoscape.LayoutOptions {
  const depth = depths(g);
  const deepest = Math.max(1, ...Array.from(depth.values()));
  if (kind === 'free') {
    const known = g.nodes.every((n) => saved[n.id]);
    if (known) return { name: 'preset', positions: (n: any) => saved[n.id()], fit: true, padding: 50 } as cytoscape.LayoutOptions;
    return { name: 'cose', animate: false, padding: 50, nodeRepulsion: () => 90000, idealEdgeLength: () => 170 } as cytoscape.LayoutOptions;
  }
  if (kind === 'tree') {
    const roots = g.nodes.filter((n) => depth.get(n.id) === 0).map((n) => n.id);
    return { name: 'breadthfirst', directed: true, roots: roots.length ? roots : undefined, padding: 50, spacingFactor: 1.05, animate: true, animationDuration: 400 } as cytoscape.LayoutOptions;
  }
  // The lead in the middle, those it hands work to around it, the rest outside.
  return {
    name: 'concentric', padding: 36, minNodeSpacing: 26, spacingFactor: 0.9, animate: true, animationDuration: 400,
    concentric: (n: any) => deepest + 1 - (depth.get(n.id()) ?? deepest + 1),
    levelWidth: () => 1,
  } as cytoscape.LayoutOptions;
}

const STYLE: cytoscape.StylesheetJson = [
  { selector: 'node', style: {
    shape: 'round-rectangle', width: 176, height: 62, 'background-color': '#16222f', 'background-opacity': 0.96,
    'border-width': 2, 'border-color': 'data(color)', label: 'data(label)', color: '#e6edf3', 'font-size': 14, 'font-weight': 600,
    'text-wrap': 'wrap', 'text-max-width': '160px', 'text-overflow-wrap': 'anywhere', 'text-valign': 'center', 'text-halign': 'center', 'line-height': 1.35,
    'overlay-opacity': 0, 'transition-property': 'border-width, border-color, background-color', 'transition-duration': 300,
  } },
  { selector: 'node.has-claude', style: { 'background-color': '#18283a' } },
  { selector: 'node.busy', style: { 'border-color': '#4ade80', 'border-width': 3, 'shadow-blur': 22, 'shadow-color': '#4ade80', 'shadow-opacity': 0.75 } as any },
  { selector: 'node.lead', style: { width: 220, height: 80, 'font-size': 17, 'border-color': '#fbbf24', 'border-width': 3.5, 'background-color': '#2a2413', 'shadow-blur': 28, 'shadow-color': '#fbbf24', 'shadow-opacity': 0.55 } as any },
  { selector: 'node.ended', style: { opacity: 0.4, 'border-style': 'dashed' } },
  { selector: 'node:selected', style: { 'border-color': '#ffffff', 'border-width': 4 } },
  { selector: 'edge', style: {
    width: 2.6, 'line-color': 'data(color)', 'target-arrow-color': 'data(color)', 'target-arrow-shape': 'triangle', 'arrow-scale': 1.2,
    'curve-style': 'bezier', opacity: 'data(strength)' as any, 'line-style': 'dashed', 'line-dash-pattern': [8, 6], 'overlay-opacity': 0,
  } },
  { selector: 'edge.live', style: { width: 3.2 } },
  { selector: 'edge.hover', style: { width: 4.5, opacity: 1 } },
  { selector: '.dim', style: { opacity: 0.12 } },
];

/** The sessions as a network: the lead in the middle, an arrow for each hand-off. */
export default function Graph2D({ graph, layout, selected, onSelect, onHoverEdge }: Props) {
  const box = useRef<HTMLDivElement | null>(null);
  const cy = useRef<Core | null>(null);
  const [ready, setReady] = useState(false);
  const shape = graph.nodes.map((n) => n.id).join(',') + '|' + graph.edges.map((e) => e.id).join(',');
  const signature = signatureOf(graph);
  const handlers = useRef({ onSelect, onHoverEdge });
  handlers.current = { onSelect, onHoverEdge };

  useEffect(() => {
    if (!box.current) return undefined;
    const c = cytoscape({ container: box.current, style: STYLE, minZoom: 0.3, maxZoom: 2.5, wheelSensitivity: 0.25, boxSelectionEnabled: false });
    cy.current = c;
    c.on('tap', 'node', (e) => handlers.current.onSelect(e.target.id()));
    c.on('tap', (e) => { if (e.target === c) handlers.current.onSelect(null); });
    c.on('mouseover', 'edge', (e) => {
      e.target.addClass('hover');
      c.elements().not(e.target).not(e.target.connectedNodes()).addClass('dim');
    });
    c.on('mousemove', 'edge', (e) => handlers.current.onHoverEdge(e.target.data('note') || null, e.originalEvent.clientX, e.originalEvent.clientY));
    c.on('mouseout', 'edge', (e) => { e.target.removeClass('hover'); c.elements().removeClass('dim'); handlers.current.onHoverEdge(null, 0, 0); });
    c.on('dragfree', 'node', () => {
      const all = readPositions();
      c.nodes().forEach((n) => { all[n.id()] = n.position(); });
      try { localStorage.setItem(POS_KEY, JSON.stringify(all)); } catch { /* private mode */ }
    });
    // The dashes of work being done now run towards where it went.
    let offset = 0;
    let frame = 0;
    const flow = () => {
      offset = (offset - 0.6) % 14;
      c.edges('.live').style('line-dash-offset', offset);
      frame = requestAnimationFrame(flow);
    };
    frame = requestAnimationFrame(flow);
    const watch = new ResizeObserver(() => { c.resize(); });
    watch.observe(box.current);
    setReady(true);
    return () => { cancelAnimationFrame(frame); watch.disconnect(); c.destroy(); cy.current = null; };
  }, []);

  // New or gone sessions and arrows: lay out again. Otherwise only restyle in place.
  useEffect(() => {
    const c = cy.current;
    if (!c || !ready) return;
    c.elements().remove();
    c.add(elements(graph));
    c.layout(layoutOf(layout, graph, readPositions())).run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shape, layout, ready]);

  useEffect(() => {
    const c = cy.current;
    if (!c || !ready) return;
    for (const el of elements(graph)) {
      const found = c.getElementById(el.data.id as string);
      if (found.empty()) continue;
      found.data(el.data);
      found.classes(el.classes as string);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [signature, ready]);

  useEffect(() => {
    const c = cy.current;
    if (!c || !ready) return;
    c.nodes().unselect();
    if (selected) c.getElementById(selected).select();
  }, [selected, ready]);

  return <div className="session-graph-canvas" ref={box} data-testid="graph-2d" />;
}
