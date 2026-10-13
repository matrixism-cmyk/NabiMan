import { useCallback, useRef, useState } from 'react';
import { useT } from '../../../i18n';
import { Dropped } from './sftpDrop';
import { baseName, downloadFile, joinPath, makeDir, saveBlob, Transfer, uploadFile } from './sftpApi';

export interface TransferRow {
  id: number;
  name: string;
  direction: 'up' | 'down';
  loaded: number;
  total: number;
  status: 'waiting' | 'running' | 'done' | 'failed' | 'cancelled';
  message?: string;
}

/**
 * Uploads and downloads for one server's file panel, one at a time so a large
 * batch does not open dozens of connections, each with its own progress row.
 */
export function useTransfers(serverId: string, onUploaded: (dir: string) => void) {
  const { t } = useT();
  const [rows, setRows] = useState<TransferRow[]>([]);
  const nextId = useRef(1);
  const running = useRef<Map<number, Transfer>>(new Map());
  const cancelled = useRef<Set<number>>(new Set());
  const queue = useRef<Promise<void>>(Promise.resolve());

  const patch = useCallback((id: number, change: Partial<TransferRow>) => {
    setRows((list) => list.map((r) => (r.id === id ? { ...r, ...change } : r)));
  }, []);

  const add = useCallback((name: string, direction: TransferRow['direction'], total: number) => {
    const id = nextId.current++;
    setRows((list) => [...list, { id, name, direction, loaded: 0, total, status: 'waiting' }]);
    return id;
  }, []);

  /** Runs one transfer and records how it ended. */
  const run = useCallback(async (id: number, start: () => Transfer) => {
    if (cancelled.current.has(id)) return null;
    patch(id, { status: 'running' });
    const transfer = start();
    running.current.set(id, transfer);
    const result = await transfer.promise;
    running.current.delete(id);
    if (result.aborted) patch(id, { status: 'cancelled' });
    else if (!result.ok && !result.exists) patch(id, { status: 'failed', message: result.message });
    return result;
  }, [patch]);

  const enqueue = useCallback((job: () => Promise<void>) => {
    queue.current = queue.current.then(job, job);
  }, []);

  /** Puts dropped or picked files into `dir`, creating dropped folders first. */
  const upload = useCallback((dir: string, dropped: Dropped) => {
    const jobs = dropped.files.map((f) => ({ ...f, id: add(f.rel, 'up', f.file.size) }));
    enqueue(async () => {
      for (const folder of dropped.dirs) {
        // A folder that is already there is fine; anything else shows up as
        // failed uploads for the files that were meant to go in it.
        await makeDir(serverId, joinPath(dir, folder)).catch(() => undefined);
      }
      let changed = false;
      for (const job of jobs) {
        const parts = job.rel.split('/');
        const name = parts.pop() || job.file.name;
        const target = parts.length ? joinPath(dir, parts.join('/')) : dir;
        const progress = (loaded: number) => patch(job.id, { loaded });
        let result = await run(job.id, () => uploadFile(serverId, target, name, job.file, false, progress));
        if (result?.exists) {
          if (window.confirm(t('sftp.overwriteConfirm', { name: job.rel }))) {
            result = await run(job.id, () => uploadFile(serverId, target, name, job.file, true, progress));
          } else {
            patch(job.id, { status: 'cancelled', message: t('sftp.skipped') });
            continue;
          }
        }
        if (result?.ok) {
          patch(job.id, { status: 'done', loaded: job.file.size });
          changed = true;
        }
      }
      if (changed) onUploaded(dir);
    });
  }, [add, enqueue, onUploaded, patch, run, serverId, t]);

  const download = useCallback((paths: string[]) => {
    const jobs = paths.map((path) => ({ path, id: add(baseName(path), 'down', 0) }));
    enqueue(async () => {
      for (const job of jobs) {
        const result = await run(job.id, () => downloadFile(serverId, job.path,
          (loaded, total) => patch(job.id, { loaded, total })));
        if (result?.ok && result.blob) {
          saveBlob(baseName(job.path), result.blob);
          patch(job.id, { status: 'done', loaded: result.blob.size, total: result.blob.size });
        }
      }
    });
  }, [add, enqueue, patch, run, serverId]);

  const cancel = useCallback((id: number) => {
    cancelled.current.add(id);
    const transfer = running.current.get(id);
    if (transfer) transfer.abort();
    else patch(id, { status: 'cancelled' });
  }, [patch]);

  const clearFinished = useCallback(() => {
    setRows((list) => list.filter((r) => r.status === 'waiting' || r.status === 'running'));
  }, []);

  return { rows, upload, download, cancel, clearFinished };
}
