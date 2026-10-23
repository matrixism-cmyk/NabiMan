import { useCallback, useEffect, useRef, useState } from 'react';
import { apiRequest } from '../../../shared/api/useApi';
import { isFolder, isLink, SftpEntry, SftpSource } from './sftpApi';

/** The columns a file list can show; the name is always there. */
export type ColumnKey = 'name' | 'size' | 'type' | 'mtime' | 'mode' | 'ownergroup';
export const OPTIONAL: Exclude<ColumnKey, 'name'>[] = ['size', 'type', 'mtime', 'mode', 'ownergroup'];

export interface Sort { key: ColumnKey; desc: boolean }

export interface ColumnState {
  /** The optional columns on show, in the order they appear. */
  shown: Exclude<ColumnKey, 'name'>[];
  /** Widths set by dragging or fitting (px); the name fills what is left until given one. */
  widths: Partial<Record<ColumnKey, number>>;
  sort: Sort;
}

const DEFAULT: ColumnState = { shown: ['size', 'type', 'mtime'], widths: {}, sort: { key: 'name', desc: false } };
/** Widths before any are set. */
export const NATURAL: Record<Exclude<ColumnKey, 'name'>, number> = { size: 64, type: 92, mtime: 104, mode: 86, ownergroup: 132 };
export const MIN_WIDTH = 36;
export const MAX_WIDTH = 720;

/** Reads any stored shape back into a valid state. */
function sanitize(raw: unknown): ColumnState {
  const r = (raw ?? {}) as Partial<ColumnState>;
  const shown = Array.isArray(r.shown)
    ? (r.shown.filter((k) => (OPTIONAL as string[]).includes(k as string)) as ColumnState['shown']).filter((k, i, a) => a.indexOf(k) === i)
    : DEFAULT.shown;
  const sortKey = (['name', ...OPTIONAL] as string[]).includes((r.sort?.key as string) ?? '') ? (r.sort!.key as ColumnKey) : 'name';
  return { shown, widths: typeof r.widths === 'object' && r.widths ? r.widths : {}, sort: { key: sortKey, desc: Boolean(r.sort?.desc) } };
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

/** The file extension, lower-cased, without the dot; '' for none or a dotfile. */
function extension(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
}

/** A short translation key, or a bare extension, for an entry's type. */
export function fileType(entry: SftpEntry): { label?: string; ext: string } {
  if (isFolder(entry)) return { label: 'sftp.type.folder', ext: '' };
  if (isLink(entry)) return { label: 'sftp.type.link', ext: '' };
  const ext = extension(entry.name);
  return ext ? { ext } : { label: 'sftp.type.file', ext: '' };
}

const owner = (e: SftpEntry) => e.owner ?? (e.uid !== undefined ? String(e.uid) : '');
const group = (e: SftpEntry) => e.group ?? (e.gid !== undefined ? String(e.gid) : '');
/** Owner and group as one value: "owner / group". */
export const ownerGroup = (e: SftpEntry): string => {
  const o = owner(e); const g = group(e);
  return o || g ? `${o} / ${g}` : '';
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
      case 'type': return extension(a.name).localeCompare(extension(b.name));
      case 'ownergroup': return ownerGroup(a).localeCompare(ownerGroup(b), undefined, { numeric: true });
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

// --- Where a panel's layout is kept -----------------------------------------

/** A signed-in account's layouts per scope, fetched once and shared by panels. */
let accountCache: Record<string, ColumnState> | null = null;
let accountLoad: Promise<Record<string, ColumnState>> | null = null;
function loadAccount(): Promise<Record<string, ColumnState>> {
  accountLoad ??= apiRequest<Record<string, unknown>>('/api/terminal/sftp/columns')
    .then((r) => { accountCache = {}; if (r.success && r.data) for (const [s, v] of Object.entries(r.data)) accountCache[s] = sanitize(v); return accountCache; })
    .catch(() => { accountCache = {}; return accountCache; });
  return accountLoad;
}
/** Dropped on sign-out so the next account does not see these. */
export function forgetAccountColumns(): void { accountCache = null; accountLoad = null; }
if (typeof window !== 'undefined') window.addEventListener('nabiman_auth_changed', forgetAccountColumns);

const localKey = (scope: string) => `nabiman_sftp_columns_${scope}`;
function loadLocal(scope: string): ColumnState {
  try { return sanitize(JSON.parse(localStorage.getItem(localKey(scope)) || 'null')); } catch { return DEFAULT; }
}

/**
 * The file list's columns — which, in what order, how wide, sorted by what.
 * A signed-in account keeps them per server on the server, so every device
 * matches; a share-link visitor keeps them in this browser.
 */
export function useColumns(source: SftpSource) {
  const scope = source.scope ?? source.key;
  const onAccount = !source.shareKey;
  const [state, setState] = useState<ColumnState>(() =>
    onAccount ? (accountCache?.[scope] ?? DEFAULT) : loadLocal(scope));
  const touched = useRef(false);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // Bring in the account's saved layout once, unless the user has already edited.
  useEffect(() => {
    if (!onAccount || accountCache) return;
    let live = true;
    loadAccount().then((all) => { if (live && !touched.current) setState(all[scope] ?? DEFAULT); });
    return () => { live = false; };
  }, [onAccount, scope]);

  // Persist changes: to the account (debounced) or this browser.
  const persist = useCallback((next: ColumnState) => {
    if (onAccount) {
      if (accountCache) accountCache[scope] = next;
      clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(() => {
        apiRequest('/api/terminal/sftp/columns', { method: 'PUT', body: JSON.stringify({ scope, layout: next }) }).catch(() => undefined);
      }, 500);
    } else {
      try { localStorage.setItem(localKey(scope), JSON.stringify(next)); } catch { /* private mode */ }
    }
  }, [onAccount, scope]);

  const edit = useCallback((change: (s: ColumnState) => ColumnState) => {
    touched.current = true;
    setState((s) => { const next = change(s); persist(next); return next; });
  }, [persist]);

  const sortBy = useCallback((key: ColumnKey) => edit((s) => ({
    ...s, sort: s.sort.key === key ? { key, desc: !s.sort.desc } : { key, desc: false },
  })), [edit]);
  const toggle = useCallback((key: Exclude<ColumnKey, 'name'>) => edit((s) => ({
    ...s, shown: s.shown.includes(key) ? s.shown.filter((k) => k !== key) : [...s.shown, key],
  })), [edit]);
  const move = useCallback((key: Exclude<ColumnKey, 'name'>, delta: -1 | 1) => edit((s) => {
    const i = s.shown.indexOf(key); const j = i + delta;
    if (i < 0 || j < 0 || j >= s.shown.length) return s;
    const shown = [...s.shown]; [shown[i], shown[j]] = [shown[j], shown[i]];
    return { ...s, shown };
  }), [edit]);
  const setWidth = useCallback((key: ColumnKey, px: number) => edit((s) => ({
    ...s, widths: { ...s.widths, [key]: Math.round(Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, px))) },
  })), [edit]);
  const reset = useCallback(() => edit((s) => ({ ...s, widths: {}, shown: DEFAULT.shown })), [edit]);

  return { state, sortBy, toggle, move, setWidth, reset };
}

export type Columns = ReturnType<typeof useColumns>;
