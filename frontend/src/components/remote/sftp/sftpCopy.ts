/**
 * Copies from one file panel's place to another's, relayed by NabiMan: the
 * browser only starts them and watches. A copy goes on if the page closes.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { apiPost, apiRequest } from '../../../shared/api/useApi';
import { useT } from '../../../shared/i18n';
import { RateState, RateView, startRate, updateRate } from './transferRate';
import { TransferRow } from './useTransfers';
import { DraggedRows } from './sftpDrop';
import { baseName } from './sftpApi';
import { confirmDialog } from '../../../shared/ui/ConfirmDialog';

export interface CopyProgress {
  state: 'copying' | 'done' | 'failed' | 'cancelled';
  bytes_total: number;
  bytes_done: number;
  files_total: number;
  files_done: number;
  current: string;
  message: string;
  skipped: string[];
  skipped_count: number;
  planned: boolean;
  finished_unix: number | null;
}

export interface Copy {
  id: string;
  from: { scope: string; name: string };
  to: { scope: string; name: string };
  dir: string;
  names: string[];
  started_unix: number;
  progress: CopyProgress;
}

export interface CopyRequest { from: string; paths: string[]; to: string; dir: string; overwrite?: boolean }

/** Starts a copy; a refusal over names already there lists them in `conflicts`. */
export async function startCopy(req: CopyRequest): Promise<{ copy?: Copy; conflicts?: string[]; message?: string }> {
  const res = await apiPost<Copy & { conflicts?: string[] }>('/api/terminal/copies', req);
  if (res.success && res.data) return { copy: res.data };
  return { conflicts: res.data?.conflicts, message: res.message };
}

export const cancelCopy = (id: string) => apiPost<Copy>(`/api/terminal/copies/${encodeURIComponent(id)}/cancel`, {});

const POLL_MS = 1000;

/**
 * The copies coming into one panel's place: those already under way when it
 * opens, and those it starts. It asks how they are going while any runs, and
 * says when one has finished in a folder.
 */
export function useCopies(scope: string | undefined, onFinished: (dir: string) => void) {
  const [copies, setCopies] = useState<Copy[]>([]);
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const running = useRef<Set<string>>(new Set());
  const timer = useRef<number | null>(null);
  const finished = useRef(onFinished);
  finished.current = onFinished;
  const rates = useRef<Map<string, [RateState, RateView | null]>>(new Map());
  const alive = useRef(true);
  const { t } = useT();
  useEffect(() => () => { alive.current = false; }, []);

  const poll = useCallback(async () => {
    if (!scope) return;
    const res = await apiRequest<Copy[]>('/api/terminal/copies').catch(() => null);
    if (!alive.current || !res?.success || !res.data) return;
    const mine = res.data.filter((c) => c.to.scope === scope);
    const now = performance.now();
    for (const c of mine) {
      if (!c.progress.finished_unix && c.progress.planned) {
        const [before] = rates.current.get(c.id) || [startRate(now), null];
        const [after, view] = updateRate(before, c.progress.bytes_done, c.progress.bytes_total, now);
        rates.current.set(c.id, [after, view || rates.current.get(c.id)?.[1] || null]);
      }
      const wasRunning = running.current.has(c.id);
      if (!c.progress.finished_unix) running.current.add(c.id);
      else if (wasRunning) {
        running.current.delete(c.id);
        finished.current(c.dir);
      }
    }
    setCopies(mine);
  }, [scope]);

  useEffect(() => {
    if (!scope) return undefined;
    let stopped = false;
    const tick = async () => {
      await poll();
      // Asks again only while something here is still copying.
      if (!stopped && running.current.size) timer.current = window.setTimeout(tick, POLL_MS);
    };
    tick();
    return () => { stopped = true; if (timer.current) window.clearTimeout(timer.current); };
  }, [scope, poll]);

  /** A copy just started here: shown at once, and watched until it ends. */
  const track = useCallback((copy: Copy) => {
    running.current.add(copy.id);
    setCopies((list) => [...list.filter((c) => c.id !== copy.id), copy]);
    if (timer.current) window.clearTimeout(timer.current);
    const tick = async () => {
      await poll();
      if (alive.current && running.current.size) timer.current = window.setTimeout(tick, POLL_MS);
    };
    timer.current = window.setTimeout(tick, POLL_MS / 2);
  }, [poll]);

  const cancel = useCallback(async (id: string) => {
    await cancelCopy(id).catch(() => undefined);
    poll();
  }, [poll]);

  const clearFinished = useCallback(() => {
    setHidden((prev) => new Set([...Array.from(prev), ...copies.filter((c) => c.progress.finished_unix).map((c) => c.id)]));
  }, [copies]);

  const rows: TransferRow[] = copies.filter((c) => !hidden.has(c.id)).map((c) => {
    const p = c.progress;
    const what = c.names.length > 1 ? t('sftp.copyMany', { name: c.names[0], count: c.names.length - 1 }) : c.names[0];
    const skipped = p.skipped_count ? ` · ${t('sftp.copySkipped', { count: p.skipped_count })}` : '';
    const view = rates.current.get(c.id)?.[1];
    return {
      id: c.id,
      name: `${what} · ${c.from.name} → ${c.to.name}`,
      direction: 'copy',
      loaded: p.bytes_done,
      total: p.planned ? p.bytes_total : 0,
      status: p.state === 'copying' ? 'running' : p.state,
      message: p.state === 'done' ? t('sftp.copyDone', { count: p.files_done }) + skipped
        : p.state === 'failed' ? (p.message || t('sftp.failed')) + skipped
        : p.state === 'cancelled' ? t('sftp.copyCancelled', { count: p.files_done }) : undefined,
      detail: [p.current, ...p.skipped].filter(Boolean).join('\n'),
      rate: p.state === 'copying' ? view?.rate ?? null : null,
      eta: p.state === 'copying' ? view?.eta ?? null : null,
    };
  });

  return { rows, track, cancel, clearFinished };
}

/**
 * Asks before copying rows dropped from another server's panel into `dir`
 * here, then again before replacing anything by the same name; starts it.
 */
export async function askAndCopy(
  rows: DraggedRows, to: { scope: string; name: string }, dir: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
): Promise<{ copy?: Copy; error?: string }> {
  if (!rows.scope || !rows.paths.length) return {};
  const items = rows.paths.map(baseName);
  const asked = await confirmDialog({
    message: t('sftp.copyConfirm', { from: rows.name || rows.scope, to: to.name, dir }),
    items, confirmLabel: t('sftp.copy'),
  });
  if (!asked) return {};
  const request: CopyRequest = { from: rows.scope, paths: rows.paths, to: to.scope, dir };
  let res = await startCopy(request);
  if (res.conflicts?.length) {
    const replace = await confirmDialog({
      message: t('sftp.copyOverwrite', { count: res.conflicts.length }),
      items: res.conflicts, danger: true, confirmLabel: t('sftp.overwrite'),
    });
    if (!replace) return {};
    res = await startCopy({ ...request, overwrite: true });
  }
  return res.copy ? { copy: res.copy } : { error: res.message || t('common.error') };
}
