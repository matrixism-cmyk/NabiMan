/**
 * What was dropped on the file panel: files from the desktop (folders
 * included, walked into), or entries dragged within the panel itself.
 */

/**
 * Set on rows dragged from a panel: where they are (the panel's route and,
 * for an account's panel, its server) and their full paths.
 */
export const SFTP_MIME = 'application/x-nabiman-sftp';

export interface DraggedRows {
  /** The panel's route prefix, telling one place from another. */
  base?: string;
  /** A saved server id or `local`; absent for a share link's panel. */
  scope?: string;
  /** The panel's name for its place, for questions and progress. */
  name?: string;
  paths: string[];
}

/**
 * The rows being dragged right now in this page. A drop target can only read
 * the data on drop; while hovering it looks here to say copy or move.
 */
let dragging: Omit<DraggedRows, 'paths'> | null = null;
export const draggingFrom = () => dragging;
export function startDragging(dt: DataTransfer, rows: DraggedRows) {
  dt.setData(SFTP_MIME, JSON.stringify(rows));
  dragging = { base: rows.base, scope: rows.scope, name: rows.name };
}
export function stopDragging() {
  dragging = null;
}

export interface DroppedFile {
  /** Path relative to the drop target, using `/`, ending in the file name. */
  rel: string;
  file: File;
}

export interface Dropped {
  files: DroppedFile[];
  /** Folders to create first, parents before children. */
  dirs: string[];
}

export const draggingOwnRows = (dt: DataTransfer | null) => Boolean(dt?.types.includes(SFTP_MIME));
export const draggingDesktopFiles = (dt: DataTransfer | null) =>
  Boolean(dt?.types.includes('Files')) && !draggingOwnRows(dt);

/** The rows dropped, and where they came from; no paths if it was not rows. */
export function droppedRows(dt: DataTransfer): DraggedRows {
  try {
    const value = JSON.parse(dt.getData(SFTP_MIME));
    const list: unknown = Array.isArray(value) ? value : value?.paths;
    const text = (v: unknown) => (typeof v === 'string' ? v : undefined);
    return {
      base: text(value?.base), scope: text(value?.scope), name: text(value?.name),
      paths: Array.isArray(list) ? list.filter((p): p is string => typeof p === 'string') : [],
    };
  } catch {
    return { paths: [] };
  }
}

type Entry = FileSystemEntry;

/**
 * Must run inside the drop handler itself: the browser empties the
 * DataTransfer as soon as the event is over, so the entries are taken now and
 * walked afterwards.
 */
export function takeDrop(dt: DataTransfer): () => Promise<Dropped> {
  const items = Array.from(dt.items || []).filter((i) => i.kind === 'file');
  const entries = items
    .map((i) => (typeof i.webkitGetAsEntry === 'function' ? i.webkitGetAsEntry() : null))
    .filter((e): e is Entry => Boolean(e));
  const plain = Array.from(dt.files || []);
  if (!entries.length) {
    return async () => ({ files: plain.map((file) => ({ rel: file.name, file })), dirs: [] });
  }
  return () => walk(entries);
}

async function walk(roots: Entry[]): Promise<Dropped> {
  const out: Dropped = { files: [], dirs: [] };
  const visit = async (entry: Entry, prefix: string): Promise<void> => {
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isFile) {
      const file = await new Promise<File | null>((resolve) =>
        (entry as FileSystemFileEntry).file(resolve, () => resolve(null)));
      if (file) out.files.push({ rel, file });
    } else if (entry.isDirectory) {
      out.dirs.push(rel);
      for (const child of await readAll(entry as FileSystemDirectoryEntry)) await visit(child, rel);
    }
  };
  for (const root of roots) await visit(root, '');
  return out;
}

/** A directory reader hands entries out in batches until it returns none. */
async function readAll(dir: FileSystemDirectoryEntry): Promise<Entry[]> {
  const reader = dir.createReader();
  const all: Entry[] = [];
  for (;;) {
    const batch = await new Promise<Entry[]>((resolve) => reader.readEntries(resolve, () => resolve([])));
    if (!batch.length) return all;
    all.push(...batch);
  }
}
