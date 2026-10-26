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
    return await picker({ mode: 'readwrite', id: 'nabiman-sftp-save' });
  } catch {
    return null;
  }
}

export interface TreeFile { rel: string; path: string; size: number }
export interface Tree { files: TreeFile[]; dirs: string[]; truncated: boolean }

/**
 * Everything under a server folder, with paths relative to it. Links to
 * folders are not followed (they could loop back on themselves).
 */
export async function listTree(src: SftpSource, root: string): Promise<Tree> {
  const tree: Tree = { files: [], dirs: [], truncated: false };
  const queue: { path: string; rel: string }[] = [{ path: root, rel: '' }];
  while (queue.length) {
    const { path, rel } = queue.shift()!;
    const res = await listDir(src, path);
    if (!res.success || !res.data) throw new Error(res.message || path);
    if (res.data.truncated) tree.truncated = true;
    for (const e of res.data.entries) {
      const childRel = rel ? `${rel}/${e.name}` : e.name;
      const childPath = joinPath(path, e.name);
      if (e.kind === 'dir') {
        tree.dirs.push(childRel);
        queue.push({ path: childPath, rel: childRel });
      } else if (e.kind === 'file' || (e.kind === 'link' && !e.to_dir)) {
        tree.files.push({ rel: childRel, path: childPath, size: e.size });
      }
    }
  }
  return tree;
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
