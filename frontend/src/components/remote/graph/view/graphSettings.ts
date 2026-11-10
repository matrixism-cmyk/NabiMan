/**
 * The work map's settings (the panel at the right, as in Obsidian's graph
 * view): what is shown, how it looks and how the layout pulls. Kept in this
 * browser; nothing here touches WebGL, so a 2D renderer can share it.
 */
export type GraphStyle = 'simple' | 'rich';
export type SectionKey = 'filters' | 'groups' | 'display' | 'forces';

export interface GraphSettings {
  /** The settings panel is open. */
  panel: boolean;
  /** Which of its sections are open. */
  sections: Record<SectionKey, boolean>;
  // Filters
  showDevices: boolean;
  showEnded: boolean;
  showServers: boolean;
  showOrphans: boolean;
  // Groups
  colorGroups: boolean;
  // Display
  style: GraphStyle;
  arrows: boolean;
  /** 0–1: how far away a name still shows; higher shows names from further off. */
  labelThreshold: number;
  /** Multiplies every node's size. */
  nodeSize: number;
  /** Multiplies every link's width. */
  linkThickness: number;
  autoRotate: boolean;
  // Forces
  /** 0–1: the pull towards the middle. */
  center: number;
  /** Multiplies the push between nodes. */
  repel: number;
  /** Multiplies how hard a link holds its ends together. */
  linkStrength: number;
  /** Multiplies how long a link wants to be. */
  linkDistance: number;
  /** Time-lapse: seconds of the record played per real second. */
  replaySpeed: number;
}

export const SETTINGS_KEY = 'nabiman_graph3d_settings';
/** The toggles that came before the panel; read once when nothing is saved yet. */
export const LEGACY_ROTATE_KEY = 'nabiman_graph3d_rotate';
export const LEGACY_NETWORK_KEY = 'nabiman_graph3d_network';

export const REPLAY_SPEEDS = [360, 1440, 4320];

export const FORCE_DEFAULTS = { center: 0.2, repel: 1, linkStrength: 1, linkDistance: 1 };

export const DEFAULT_SETTINGS: GraphSettings = {
  panel: false,
  sections: { filters: true, groups: true, display: true, forces: false },
  showDevices: true,
  showEnded: true,
  showServers: true,
  showOrphans: true,
  colorGroups: true,
  style: 'simple',
  arrows: true,
  labelThreshold: 0.5,
  nodeSize: 1,
  linkThickness: 1,
  autoRotate: true,
  ...FORCE_DEFAULTS,
  replaySpeed: 1440,
};

/** The bounds each slider moves within. */
export const RANGES = {
  labelThreshold: [0, 1],
  nodeSize: [0.4, 2.5],
  linkThickness: [0.3, 3],
  center: [0, 1],
  repel: [0.1, 3],
  linkStrength: [0.1, 2],
  linkDistance: [0.3, 3],
} as const;

type Store = Pick<Storage, 'getItem' | 'setItem'>;

const bool = (v: unknown, d: boolean) => (typeof v === 'boolean' ? v : d);
const num = (v: unknown, d: number, [lo, hi]: readonly [number, number]) => (typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : d);

/** Whatever was stored, made into whole and sensible settings: unknown or broken values fall back to the defaults. */
export function sanitize(raw: unknown, base: GraphSettings = DEFAULT_SETTINGS): GraphSettings {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Record<string, unknown>;
  const sec = (r.sections && typeof r.sections === 'object' ? r.sections : {}) as Record<string, unknown>;
  return {
    panel: bool(r.panel, base.panel),
    sections: {
      filters: bool(sec.filters, base.sections.filters),
      groups: bool(sec.groups, base.sections.groups),
      display: bool(sec.display, base.sections.display),
      forces: bool(sec.forces, base.sections.forces),
    },
    showDevices: bool(r.showDevices, base.showDevices),
    showEnded: bool(r.showEnded, base.showEnded),
    showServers: bool(r.showServers, base.showServers),
    showOrphans: bool(r.showOrphans, base.showOrphans),
    colorGroups: bool(r.colorGroups, base.colorGroups),
    style: r.style === 'rich' || r.style === 'simple' ? r.style : base.style,
    arrows: bool(r.arrows, base.arrows),
    labelThreshold: num(r.labelThreshold, base.labelThreshold, RANGES.labelThreshold),
    nodeSize: num(r.nodeSize, base.nodeSize, RANGES.nodeSize),
    linkThickness: num(r.linkThickness, base.linkThickness, RANGES.linkThickness),
    autoRotate: bool(r.autoRotate, base.autoRotate),
    center: num(r.center, base.center, RANGES.center),
    repel: num(r.repel, base.repel, RANGES.repel),
    linkStrength: num(r.linkStrength, base.linkStrength, RANGES.linkStrength),
    linkDistance: num(r.linkDistance, base.linkDistance, RANGES.linkDistance),
    replaySpeed: typeof r.replaySpeed === 'number' && REPLAY_SPEEDS.includes(r.replaySpeed) ? r.replaySpeed : base.replaySpeed,
  };
}

function storage(): Store | null {
  try { return typeof localStorage === 'undefined' ? null : localStorage; } catch { return null; }
}

/** The saved settings; with none saved, the defaults (taking the two old toggles over). */
export function loadSettings(store: Store | null = storage()): GraphSettings {
  if (!store) return sanitize(null);
  try {
    const text = store.getItem(SETTINGS_KEY);
    if (text) return sanitize(JSON.parse(text));
    const rotate = store.getItem(LEGACY_ROTATE_KEY);
    const network = store.getItem(LEGACY_NETWORK_KEY);
    return sanitize({
      ...(rotate !== null ? { autoRotate: rotate === '1' } : {}),
      ...(network !== null ? { showDevices: network === '1' } : {}),
    });
  } catch {
    // Bad JSON or storage refused: start from the defaults.
    return sanitize(null);
  }
}

export function saveSettings(s: GraphSettings, store: Store | null = storage()): void {
  if (!store) return;
  try { store.setItem(SETTINGS_KEY, JSON.stringify(s)); } catch { /* private mode or full: only this browser's memory is lost */ }
}
