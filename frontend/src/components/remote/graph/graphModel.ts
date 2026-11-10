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
  /** The AI agents running in it, by kind. */
  agents?: string[];
  /** The model its Claude last answered with. */
  model?: string;
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
  /** What it is on the work map: a session (default) or the machine sessions run on. */
  kind?: 'session' | 'server' | 'device';
  /** A network device's kind: firewall, router, switch, node, vip, gateway, dongle, internet, storage, other. */
  device?: string;
  /** A device's tier in the network, counted from the internet (0) inwards; drawn top to bottom. */
  layer?: number;
  /** A second line: a server's address, a session's Claude. */
  sub?: string;
  /** A server's state as last checked. */
  status?: string;
  /** What a busy session is working on: the first line of the last request it got. */
  task?: string;
  /** Messages to or from it in the window: busier ones are drawn larger. */
  heat?: number;
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
  /** `order` (default): a hand-off. `host`: a server and a session on it. `conn`: NabiMan's hold on a server. */
  kind?: 'order' | 'host' | 'conn' | 'net';
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
      agents: s.agents || [],
      model: s.claude?.[0]?.model,
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
  ...g.nodes.map((n) => `${n.id}|${n.name}|${n.claude}|${(n.agents || []).join('+')}|${n.model || ''}|${n.group}|${n.busy}|${n.lead}|${n.alive}`),
  ...g.edges.map((e) => `${e.id}|${e.live}|${e.strength.toFixed(1)}`),
].join(';');

/** A saved server, as the work map needs it. */
export interface MapServer { id: string; name: string; host: string; port?: number; status?: string }
/** A message between two Claude sessions, from the record. */
export interface MapEvent { id: number; at: number; from: string | null; to: string | null; from_name: string; to_name: string; text: string }

/**
 * The work map: the sessions and their hand-offs, plus the machines they run
 * on (this one and each server a session is connected to), tied together so
 * each machine's sessions gather round it.
 */
/** The network as an administrator entered it: devices, links, and which device each server sits behind. */
export interface Topology {
  devices: { key: string; name: string; kind: string; address: string; note: string }[];
  links: { from: string; to: string; label: string }[];
  hosts: { device: string; server_host: string }[];
}

export function buildWorkMap(sessions: SessionInfo[], links: SessionLink[], servers: MapServer[], events: MapEvent[], here: string, topology?: Topology | null): Graph {
  const g = buildGraph(sessions, links);
  const heat = new Map<string, number>();
  const task = new Map<string, string>();
  for (const e of events) {
    for (const id of [e.from, e.to]) if (id) heat.set(id, (heat.get(id) || 0) + 1);
    if (e.to && e.text) task.set(e.to, e.text);
  }
  const hub = g.nodes.find((n) => n.lead)?.id;
  const made = new Map<string, GraphNode>();
  for (const n of [...g.nodes]) {
    n.kind = 'session';
    n.heat = heat.get(n.id) || 0;
    if (n.busy) n.task = task.get(n.id);
    const server = n.group === 'local' ? undefined : servers.find((x) => x.host === n.group);
    const id = n.group === 'local' ? 'host:local' : server ? `srv:${server.id}` : `host:${n.group}`;
    if (!made.has(id)) {
      const node: GraphNode = {
        id, kind: 'server', group: n.group, color: n.color, claude: '', busy: false, lead: false, orchestrator: false,
        alive: !server || server.status !== 'offline',
        name: n.group === 'local' ? here : server?.name || n.group,
        sub: n.group === 'local' ? '' : `${server?.host || n.group}${server?.port && server.port !== 22 ? `:${server.port}` : ''}`,
        status: server?.status,
      };
      made.set(id, node);
      g.nodes.push(node);
      // NabiMan (the lead's machine) holds a connection to every other machine.
      if (id !== 'host:local') g.edges.push({ id: `conn>${id}`, from: 'host:local', to: id, live: false, strength: 0.5, color: n.color, note: '', kind: 'conn' });
    }
    g.edges.push({ id: `host>${n.id}`, from: id, to: n.id, live: false, strength: 0.4, color: n.color, note: '', kind: 'host' });
  }
  if (!made.has('host:local') && made.size) {
    g.nodes.push({ id: 'host:local', kind: 'server', group: 'local', color: GROUP_COLORS[0], claude: '', busy: false, lead: false, orchestrator: false, alive: true, name: here, sub: '' });
  }
  void hub;
  // The network between the machines: each device, its links, and the machines behind it.
  if (topology?.devices.length) {
    const tie = new Map(topology.hosts.map((h) => [h.server_host, h.device]));
    for (const d of topology.devices) {
      g.nodes.push({ id: `dev:${d.key}`, kind: 'device', device: d.kind, name: d.name, sub: d.address, group: 'network', color: '#94a3b8', claude: '', busy: false, lead: false, orchestrator: false, alive: true, task: d.note });
    }
    // Tiers: how many links a device is from the internet (or, with none entered, from the firewall).
    const tier = new Map<string, number>();
    const roots = topology.devices.filter((d) => d.kind === 'internet');
    let front = (roots.length ? roots : topology.devices.filter((d) => d.kind === 'firewall')).map((d) => d.key);
    front.forEach((k) => tier.set(k, 0));
    for (let depth = 1; front.length && depth < 12; depth += 1) {
      const next: string[] = [];
      for (const l of topology.links) {
        for (const [a, b] of [[l.from, l.to], [l.to, l.from]]) {
          if (front.includes(a) && !tier.has(b)) { tier.set(b, depth); next.push(b); }
        }
      }
      front = next;
    }
    const deepest = Math.max(0, ...Array.from(tier.values()));
    for (const n of g.nodes) if (n.kind === 'device') n.layer = tier.get(n.id.slice(4)) ?? deepest + 1;
    for (const l of topology.links) g.edges.push({ id: `net>${l.from}>${l.to}`, from: `dev:${l.from}`, to: `dev:${l.to}`, live: false, strength: 0.5, color: '#64748b', note: l.label, kind: 'net' });
    for (const [id, node] of Array.from(made)) {
      const host = id === 'host:local' ? 'localhost' : servers.find((x) => `srv:${x.id}` === id)?.host || node.group;
      const device = tie.get(host);
      if (!device) continue;
      g.edges.push({ id: `net>${device}>${id}`, from: `dev:${device}`, to: id, live: false, strength: 0.5, color: node.color, note: '', kind: 'net' });
      // The network path stands in for the plain thread from NabiMan.
      g.edges = g.edges.filter((e) => e.id !== `conn>${id}`);
    }
  }
  return g;
}
