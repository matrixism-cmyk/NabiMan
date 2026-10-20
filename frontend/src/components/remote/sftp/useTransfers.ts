import { useCallback, useRef, useState } from 'react';
import { useT } from '../../../i18n';
import { Dropped } from './sftpDrop';
import { baseName, joinPath, makeDir, saveFile, SftpSource, Transfer, uploadFile } from './sftpApi';
import { RateState, startRate, updateRate } from './transferRate';
import { confirmDialog } from '../../common/ConfirmDialog';

export interface TransferRow {
  id: number;
  name: string;
  direction: 'up' | 'down';
  loaded: number;
  total: number;
  status: 'waiting' | 'running' | 'done' | 'failed' | 'cancelled';
  message?: string;
  /** Bytes per second, once there is enough to tell. */
  rate?: number | null;
  /** Seconds left, once there is enough to tell. */
  eta?: number | null;
}

/**
 * Uploads and downloads for one server's file panel, one at a time so a large
 * batch does not open dozens of connections, each with its own progress row.
 */
export function useTransfers(source: SftpSource, onUploaded: (dir: string) => void) {
  const { t } = useT();
  const [rows, setRows] = useState<TransferRow[]>([]);
  const nextId = useRef(1);
  const running = useRef<Map<number, Transfer>>(new Map());
  const cancelled = useRef<Set<number>>(new Set());
  const queue = useRef<Promise<void>>(Promise.resolve());
  const rates = useRef<Map<number, RateState>>(new Map());

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
        await makeDir(source, joinPath(dir, folder)).catch(() => undefined);
      }
      let changed = false;
      for (const job of jobs) {
        const parts = job.rel.split('/');
        const name = parts.pop() || job.file.name;
        const target = parts.length ? joinPath(dir, parts.join('/')) : dir;
        rates.current.set(job.id, startRate(performance.now()));
        const progress = (loaded: number) => {
          const before = rates.current.get(job.id) || startRate(performance.now());
          const [after, view] = updateRate(before, loaded, job.file.size, performance.now());
          rates.current.set(job.id, after);
          patch(job.id, view ? { loaded, ...view } : { loaded });
        };
        let result = await run(job.id, () => uploadFile(source, target, name, job.file, false, progress));
        if (result?.exists) {
          if ((await confirmDialog(t('sftp.overwriteConfirm', { name: job.rel })))) {
            result = await run(job.id, () => uploadFile(source, target, name, job.file, true, progress));
          } else {
            patch(job.id, { status: 'cancelled', message: t('sftp.skipped') });
            continue;
          }
        }
        if (result?.ok) {
          patch(job.id, { status: 'done', loaded: job.file.size, rate: null, eta: null });
          changed = true;
        }
      }
      if (changed) onUploaded(dir);
    });
  }, [add, enqueue, onUploaded, patch, run, source, t]);

  /** Downloads go to the browser's downloader; the row says it was handed over. */
  /** Folders arrive as a ZIP archive of everything in them. */
  const download = useCallback((items: { path: string; folder?: boolean }[]) => {
    const jobs = items.map(({ path, folder }) => {
      const name = folder ? `${baseName(path)}.zip` : baseName(path);
      return { path, name, id: add(name, 'down', 0) };
    });
    enqueue(async () => {
      for (const job of jobs) {
        if (cancelled.current.has(job.id)) continue;
        patch(job.id, { status: 'running' });
        const started = await saveFile(source, job.path, job.name).catch(() => false);
        patch(job.id, started
          ? { status: 'done', message: t('sftp.handedToBrowser') }
          : { status: 'failed', message: t('sftp.downloadFailed') });
      }
    });
  }, [add, enqueue, patch, source, t]);

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
