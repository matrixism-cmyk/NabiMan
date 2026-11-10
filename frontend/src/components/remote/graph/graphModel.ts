import { SessionLink, LINK_COLORS, colorIndex } from '../SessionLinks';
import type { SessionInfo } from '../SessionsView';

/** How the sessions are shown: the cards, or a graph of who hands work to whom. */
export type ViewMode = 'tiles' | 'graph2d' | 'graph3d';
export const VIEW_MODES: ViewMode[] = ['tiles', 'graph2d', 'graph3d'];
const VIEW_KEY = 'nabiman_sessions_view';

export function readView(): ViewMode {
  try {
    const v = localStorage.getItem(VIEW_KEY) as ViewMode | null;
    return v && VIEW_MODES.includes(v) ? v : 'tiles';
  } catch {
    return 'tiles';
  }
}
export function saveView(v: ViewMode) {
  try { localStorage.setItem(VIEW_KEY, v); } catch { /* private mode */ }
}

export interface GraphNode {
  id: string;
  /** The name shown: the operator's title, else the label. */
  name: string;
  /** The Claude in it, if any. */
  claude: string;
  /** Where it runs: `local`, or the host of an SSH/telnet session. */
  group: string;
  busy: boolean;
  /** The session leading all Claude work (총괄). */
  lead: boolean;
  /** Hands work to others here and takes it from none. */
  orchestrator: boolean;
  alive: boolean;
  /** One colour per group, so sessions of one server read as a set. */
  color: string;
}

export interface GraphEdge {
  id: string;
  from: string;
  to: string;
  live: boolean;
  /** 0–1: full while the work is being done, fading after. */
  strength: number;
  /** The colour of the session handing out the work, as on the tiles. */
  color: string;
  note: string;
}

export interface Graph { nodes: GraphNode[]; edges: GraphEdge[] }

/** Colours for the groups (servers); the first is this machine's. */
export const GROUP_COLORS = ['#5eead4', '#a78bfa', '#fbbf24', '#60a5fa', '#f472b6', '#4ade80', '#fb923c', '#94a3b8'];

/** `local`, or the host out of `ssh:user@host:port` / `telnet:host:port`. */
export function groupOf(label: string): string {
  if (!label || label === 'local') return 'local';
  const rest = label.replace(/^[a-z]+:/, '');
  const host = rest.includes('@') ? rest.split('@')[1] : rest;
  return host.replace(/:\d+$/, '') || label;
}

/** The sessions and the hand-offs between them, as nodes and edges. */
export function buildGraph(sessions: SessionInfo[], links: SessionLink[]): Graph {
  const groups: string[] = [];
  const nodes = sessions.map((s): GraphNode => {
    const group = groupOf(s.label);
    if (!groups.includes(group)) groups.push(group);
    return {
      id: s.id,
      // An unnamed remote session goes by its host: the full `ssh:user@host:port` is in the panel.
      name: s.title || (group === 'local' ? s.label : group),
      claude: s.claude?.[0]?.name || '',
      group,
      busy: !!s.busy,
      lead: !!s.lead,
      orchestrator: !!s.orchestrator,
      alive: s.alive,
      color: '',
    };
  });
  // This machine first, then the others in the order met: stable colours while the list is.
  groups.sort((a, b) => Number(b === 'local') - Number(a === 'local'));
  for (const n of nodes) n.color = GROUP_COLORS[groups.indexOf(n.group) % GROUP_COLORS.length];
  const ids = new Set(nodes.map((n) => n.id));
  const edges = links.filter((l) => ids.has(l.from) && ids.has(l.to)).map((l): GraphEdge => ({
    id: `${l.from}>${l.to}`,
    from: l.from,
    to: l.to,
    live: l.live,
    strength: l.strength ?? (l.live ? 1 : 0.35),
    color: LINK_COLORS[colorIndex(l.from)],
    note: l.note || '',
  }));
  return { nodes, edges };
}

/** How far a session is from the lead (or from any orchestrator): 0 the lead, 1 those it hands work to… */
export function depths(g: Graph): Map<string, number> {
  const out = new Map<string, number>();
  const roots = g.nodes.filter((n) => n.lead).map((n) => n.id);
  const start = roots.length ? roots : g.nodes.filter((n) => n.orchestrator).map((n) => n.id);
  let frontier = start;
  start.forEach((id) => out.set(id, 0));
  for (let d = 1; frontier.length && d < 8; d += 1) {
    const next: string[] = [];
    for (const e of g.edges) {
      if (frontier.includes(e.from) && !out.has(e.to)) { out.set(e.to, d); next.push(e.to); }
    }
    frontier = next;
  }
  return out;
}

/** A signature of everything a drawing depends on, to redraw only on change. */
export const signatureOf = (g: Graph) => [
  ...g.nodes.map((n) => `${n.id}|${n.name}|${n.claude}|${n.group}|${n.busy}|${n.lead}|${n.alive}`),
  ...g.edges.map((e) => `${e.id}|${e.live}|${e.strength.toFixed(1)}`),
].join(';');
