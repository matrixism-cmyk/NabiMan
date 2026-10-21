import { useCallback, useEffect, useState } from 'react';
import { isFolder, SftpEntry } from './sftpApi';

/** The columns a file list can show; the name is always there. */
export type ColumnKey = 'name' | 'size' | 'mtime' | 'mode' | 'owner' | 'group';
export const OPTIONAL: Exclude<ColumnKey, 'name'>[] = ['size', 'mtime', 'mode', 'owner', 'group'];

export interface Sort { key: ColumnKey; desc: boolean }

export interface ColumnState {
  /** The optional columns on show, in their order. */
  shown: Exclude<ColumnKey, 'name'>[];
  /** Widths set by dragging or fitting (px); the name fills what is left until given one. */
  widths: Partial<Record<ColumnKey, number>>;
  sort: Sort;
}

const KEY = 'nabiman_sftp_columns';
const DEFAULT: ColumnState = { shown: ['size', 'mtime'], widths: {}, sort: { key: 'name', desc: false } };
/** Widths before any are set. */
export const NATURAL: Record<Exclude<ColumnKey, 'name'>, number> = { size: 64, mtime: 104, mode: 86, owner: 72, group: 72 };
export const MIN_WIDTH = 36;
export const MAX_WIDTH = 720;

function load(): ColumnState {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) || 'null');
    if (!raw) return DEFAULT;
    const shown = Array.isArray(raw.shown) ? raw.shown.filter((k: string) => (OPTIONAL as string[]).includes(k)) : DEFAULT.shown;
    const sortKey = ['name', ...OPTIONAL].includes(raw.sort?.key) ? raw.sort.key : 'name';
    return { shown, widths: typeof raw.widths === 'object' && raw.widths ? raw.widths : {}, sort: { key: sortKey, desc: Boolean(raw.sort?.desc) } };
  } catch { return DEFAULT; }
}

/** `grid-template-columns` for the rows: the icon, the name, then the columns on show. */
export function template(state: ColumnState): string {
  const name = state.widths.name ? `${state.widths.name}px` : 'minmax(120px, 1fr)';
  return ['20px', name, ...state.shown.map((k) => `${state.widths[k] ?? NATURAL[k]}px`)].join(' ');
}

/** How wide the table must be at least, so fixed columns past the panel's edge scroll into view. */
export function minWidth(state: ColumnState): number {
  const columns = state.shown.reduce((sum, k) => sum + (state.widths[k] ?? NATURAL[k]), 0);
  const gaps = 8 * (state.shown.length + 1);
  return 20 + (state.widths.name ?? 120) + columns + gaps + 18;
}

const text = (e: SftpEntry, key: ColumnKey): string => {
  if (key === 'owner') return e.owner ?? (e.uid !== undefined ? String(e.uid) : '');
  if (key === 'group') return e.group ?? (e.gid !== undefined ? String(e.gid) : '');
  return e.name;
};

/**
 * Entries in the order chosen, folders first as file managers keep them.
 * Names compare as people read them ("file2" before "file10").
 */
export function sortEntries(entries: SftpEntry[], sort: Sort): SftpEntry[] {
  const byName = (a: SftpEntry, b: SftpEntry) => a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });
  const compare = (a: SftpEntry, b: SftpEntry): number => {
    switch (sort.key) {
      case 'size': return (a.size ?? 0) - (b.size ?? 0);
      case 'mtime': return (a.mtime ?? 0) - (b.mtime ?? 0);
      case 'mode': return (a.mode ?? -1) - (b.mode ?? -1);
      case 'owner': case 'group': return text(a, sort.key).localeCompare(text(b, sort.key), undefined, { numeric: true });
      default: return byName(a, b);
    }
  };
  return [...entries].sort((a, b) => {
    const folders = Number(isFolder(b)) - Number(isFolder(a));
    if (folders) return folders;
    const order = compare(a, b) || byName(a, b);
    return sort.desc ? -order : order;
  });
}

/** The file list's columns — which, how wide, sorted by what — remembered in this browser. */
export function useColumns() {
  const [state, setState] = useState<ColumnState>(load);
  useEffect(() => { try { localStorage.setItem(KEY, JSON.stringify(state)); } catch { /* private mode */ } }, [state]);

  const sortBy = useCallback((key: ColumnKey) => setState((s) => ({
    ...s, sort: s.sort.key === key ? { key, desc: !s.sort.desc } : { key, desc: false },
  })), []);
  const toggle = useCallback((key: Exclude<ColumnKey, 'name'>) => setState((s) => ({
    ...s,
    // A column comes back in its usual place.
    shown: s.shown.includes(key) ? s.shown.filter((k) => k !== key) : OPTIONAL.filter((k) => k === key || s.shown.includes(k)),
  })), []);
  const setWidth = useCallback((key: ColumnKey, px: number) => setState((s) => ({
    ...s, widths: { ...s.widths, [key]: Math.round(Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, px))) },
  })), []);
  const reset = useCallback(() => setState((s) => ({ ...s, widths: {}, shown: DEFAULT.shown })), []);
  return { state, sortBy, toggle, setWidth, reset };
}

export type Columns = ReturnType<typeof useColumns>;
