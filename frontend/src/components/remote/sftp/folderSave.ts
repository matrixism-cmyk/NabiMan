/**
 * Saving a server folder as a folder (not a ZIP) on this computer, through the
 * File System Access API: the user picks where, and each file streams straight
 * into the matching subfolder. Chrome and Edge have it; elsewhere the caller
 * falls back to the ZIP download.
 */
import { downloadLink, joinPath, listDir, SftpSource } from './sftpApi';

type DirHandle = FileSystemDirectoryHandle;
/** FileSystemFileHandle.createWritable's stream; not yet in TypeScript's DOM types. */
interface Writable { write(data: Uint8Array): Promise<void>; close(): Promise<void>; abort(): Promise<void> }

export const canSaveFolder = (): boolean =>
  typeof (window as unknown as { showDirectoryPicker?: unknown }).showDirectoryPicker === 'function';

/** Asks where to save; null if the user cancelled. Must run in the click. */
export async function pickSaveDir(): Promise<DirHandle | null> {
  try {
    const picker = (window as unknown as { showDirectoryPicker: (o: object) => Promise<DirHandle> }).showDirectoryPicker;
    // Start in Downloads: Chrome refuses the top-level folders themselves
    // (Downloads, Desktop, home, a drive root), but any folder inside them works.
    return await picker({ mode: 'readwrite', id: 'nabiman-sftp-save', startIn: 'downloads' });
  } catch {
    return null;
  }
}

export interface TreeFile { rel: string; path: string; size: number }
/** One server folder's contents, with paths relative to the folder being saved. */
export interface Level { rel: string; files: TreeFile[]; dirs: string[]; skipped: number; truncated: boolean }

/**
 * Folders a package manager or interpreter rebuilds (a Python venv alone can be
 * 30,000 files): saving them file by file takes hours and is of no use on the
 * other machine. They are left out; the ZIP download still has everything.
 */
export const REBUILT = new Set(['node_modules', '.venv', 'venv', '__pycache__', '.mypy_cache', '.pytest_cache', '.tox']);

/**
 * Walks a server folder breadth-first, handing over each folder as soon as it
 * is read, so saving starts at once instead of after the whole tree is listed
 * (a tree with thousands of folders takes minutes to list). Links to folders
 * are not followed (they could loop back on themselves), and REBUILT folders are skipped.
 */
export async function walkFolder(
  src: SftpSource, root: string, onLevel: (level: Level) => Promise<void>, stop: () => boolean,
): Promise<void> {
  const queue: { path: string; rel: string }[] = [{ path: root, rel: '' }];
  while (queue.length && !stop()) {
    const { path, rel } = queue.shift()!;
    const res = await listDir(src, path);
    if (!res.success || !res.data) throw new Error(res.message || path);
    const level: Level = { rel, files: [], dirs: [], skipped: 0, truncated: res.data.truncated };
    for (const e of res.data.entries) {
      const childRel = rel ? `${rel}/${e.name}` : e.name;
      const childPath = joinPath(path, e.name);
      if (e.kind === 'dir' && REBUILT.has(e.name)) {
        level.skipped += 1;
      } else if (e.kind === 'dir') {
        level.dirs.push(childRel);
        queue.push({ path: childPath, rel: childRel });
      } else if (e.kind === 'file' || (e.kind === 'link' && !e.to_dir)) {
        level.files.push({ rel: childRel, path: childPath, size: e.size });
      }
    }
    await onLevel(level);
  }
}

/** The handle for `rel` (a/b/c) under `base`, creating folders on the way. */
export async function dirAt(base: DirHandle, rel: string): Promise<DirHandle> {
  let dir = base;
  for (const part of rel.split('/').filter(Boolean)) dir = await dir.getDirectoryHandle(part, { create: true });
  return dir;
}

/** Streams one server file into `base` at `rel`, reporting bytes written. */
export async function saveInto(
  src: SftpSource, base: DirHandle, file: TreeFile, signal: AbortSignal, onBytes: (n: number) => void,
): Promise<void> {
  const url = await downloadLink(src, file.path);
  if (!url) throw new Error('link');
  const res = await fetch(url, { signal });
  if (!res.ok || !res.body) throw new Error(String(res.status));
  const parts = file.rel.split('/');
  const name = parts.pop() || file.rel;
  const handle = await (await dirAt(base, parts.join('/'))).getFileHandle(name, { create: true });
  const out = await (handle as unknown as { createWritable(): Promise<Writable> }).createWritable();
  const reader = res.body.getReader();
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      await out.write(value);
      onBytes(value.byteLength);
    }
    await out.close();
  } catch (err) {
    await out.abort().catch(() => undefined);
    throw err;
  }
}
